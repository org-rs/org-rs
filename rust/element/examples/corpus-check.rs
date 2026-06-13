use goof::Error;
use lexpr::Value;
use org_element::data::{NodeArena, NodeId, ScriptKind, Syntax, SyntaxT};
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::io::{IsTerminal, Write};
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Error)]
enum CorpusError {
    #[error("failed to read {path:?}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("emacs oracle could not be launched for {path:?}: {source}")]
    OracleExec {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("oracle output for {path:?} is not valid UTF-8: {source}")]
    OracleUtf8 {
        path: PathBuf,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("oracle S-expression for {path:?} did not parse: {source}")]
    OracleSexp {
        path: PathBuf,
        #[source]
        source: lexpr::parse::Error,
    },
    #[error("oracle output for {path:?} did not yield a node")]
    EmptyOracle { path: PathBuf },
    #[error("emacs is not in PATH or oracle.el is missing")]
    EmacsUnavailable,
}

#[derive(Default, Clone, Copy)]
struct FileTimings {
    emacs: Duration,
    sexp: Duration,
    oracle_conv: Duration,
    rust: Duration,
    compare: Duration,
    total: Duration,
}

struct TypeInterner {
    strings: Vec<Box<str>>,
    index: HashMap<Box<str>, u32>,
}

impl TypeInterner {
    fn new() -> Self {
        TypeInterner {
            strings: Vec::new(),
            index: HashMap::new(),
        }
    }

    fn intern(&mut self, raw: &str) -> TypeKey {
        let normalized = normalize_type(raw);
        if let Some(&k) = self.index.get(normalized.as_str()) {
            return TypeKey(k);
        }
        let k = self.strings.len() as u32;
        let boxed: Box<str> = normalized.into();
        self.index.insert(boxed.clone(), k);
        self.strings.push(boxed);
        TypeKey(k)
    }

    fn lookup(&self, key: TypeKey) -> &str {
        &self.strings[key.0 as usize]
    }
}

thread_local! {
    static INTERNER: RefCell<TypeInterner> = RefCell::new(TypeInterner::new());
}

fn intern_type(raw: &str) -> TypeKey {
    INTERNER.with(|i| i.borrow_mut().intern(raw))
}

/// Map Emacs org-element type names to a canonical form before normalisation.
/// Where Emacs uses finer-grained names than Rust (e.g. subscript/superscript),
/// the Rust side is responsible for emitting the Emacs-compatible name — so
/// Emacs names pass through unchanged here.
fn canonical_type(s: &str) -> &str {
    s
}

fn normalize_type(s: &str) -> String {
    let s = canonical_type(s);
    let without_hyphens = s.replace('-', "_");
    let mut result = String::new();
    let mut after_sep = false;
    for (i, c) in without_hyphens.char_indices() {
        if c == '_' {
            if !result.is_empty() && !after_sep {
                result.push('_');
            }
            after_sep = true;
            continue;
        }
        if c.is_uppercase() && i > 0 && !after_sep {
            result.push('_');
        }
        result.extend(c.to_lowercase());
        after_sep = false;
    }
    result
}

fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct TypeKey(u32);

impl fmt::Display for TypeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        INTERNER.with(|i| write!(f, "{}", i.borrow().lookup(*self)))
    }
}

impl fmt::Debug for TypeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// 1-based Emacs character position as emitted by the oracle (`:begin` / `:end`).
/// This is a *character* count, not a byte offset — they differ for non-ASCII text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct EmacsCharPos(NonZeroU64);

impl EmacsCharPos {
    /// 0-based character index.
    #[inline]
    fn char_index(self) -> usize {
        (self.0.get() - 1) as usize
    }
}

/// 0-based byte offset into the file content, as used by the Rust parser.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ByteOffset(usize);

impl ByteOffset {
    #[inline]
    fn as_usize(self) -> usize {
        self.0
    }
}

/// Precomputed map from character-index → byte-offset for O(1) lookups.
struct CharByteTable {
    char_to_byte: Vec<usize>,
}

impl CharByteTable {
    fn new(s: &str) -> Self {
        let mut char_to_byte = Vec::with_capacity(s.len());
        for (b, _) in s.char_indices() {
            char_to_byte.push(b);
        }
        // sentinel: one past the last char maps to string length
        char_to_byte.push(s.len());
        CharByteTable { char_to_byte }
    }

    fn to_byte(&self, pos: EmacsCharPos) -> ByteOffset {
        let idx = pos.char_index();
        ByteOffset(
            self.char_to_byte
                .get(idx)
                .copied()
                .unwrap_or(self.char_to_byte[self.char_to_byte.len() - 1]),
        )
    }
}

struct Snippet(String);

impl Snippet {
    fn new(input: &str, begin: usize, end: usize) -> Self {
        let lo_raw = begin.saturating_sub(20);
        let hi_raw = (end + 20).min(input.len());
        // Snap to valid char boundaries so we don't panic on multi-byte chars.
        let lo = (0..=lo_raw)
            .rev()
            .find(|&i| input.is_char_boundary(i))
            .unwrap_or(0);
        let hi = (hi_raw..=input.len())
            .find(|&i| input.is_char_boundary(i))
            .unwrap_or(input.len());
        Snippet(input[lo..hi].replace('\n', "\\n"))
    }
}

impl fmt::Display for Snippet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Snippet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

#[derive(Debug)]
struct OracleNode {
    type_key: TypeKey,
    begin: Option<ByteOffset>,
    end: Option<ByteOffset>,
    props_display: String,
    children: Vec<OracleNode>,
}

impl OracleNode {
    fn type_key(&self) -> TypeKey {
        self.type_key
    }

    fn span(&self) -> (usize, usize) {
        (
            self.begin.map_or(0, ByteOffset::as_usize),
            self.end.map_or(0, ByteOffset::as_usize),
        )
    }
}

impl fmt::Display for OracleNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}{})", self.type_key, self.props_display)
    }
}

fn from_value(v: &Value, table: &CharByteTable) -> Option<OracleNode> {
    let mut iter = v.list_iter()?;

    let type_name = iter.next()?.as_symbol()?;
    let type_key = intern_type(type_name);
    let plist_val = iter.next()?;

    let mut props_display = String::new();
    let mut begin = None;
    let mut end = None;

    if let Some(mut pl) = plist_val.list_iter() {
        while let Some(k) = pl.next() {
            let key = k.as_symbol()?;
            let val = pl.next()?;
            match key {
                ":begin" => {
                    begin = val
                        .as_u64()
                        .and_then(NonZeroU64::new)
                        .map(|n| table.to_byte(EmacsCharPos(n)))
                }
                ":end" => {
                    end = val
                        .as_u64()
                        .and_then(NonZeroU64::new)
                        .map(|n| table.to_byte(EmacsCharPos(n)))
                }
                _ => {
                    use std::fmt::Write;
                    write!(props_display, " {key} {val:?}").ok();
                }
            }
        }
    }

    let children = iter
        .filter(|v| !v.is_string() && !v.is_symbol() && !v.is_null())
        .filter_map(|v| from_value(v, table))
        .collect();

    Some(OracleNode {
        type_key,
        begin,
        end,
        props_display,
        children,
    })
}

#[derive(Debug, Error)]
enum DiscrepancyKind {
    #[error("{0}")]
    TypeMismatch(goof::Mismatch<TypeKey>),
    #[error("missing in Rust at {0}")]
    MissingInRust(usize),
    #[error("extra in Rust at {0}")]
    ExtraInRust(usize),
}

#[derive(Debug)]
struct Discrepancy {
    tree_path: String,
    oracle_key: TypeKey,
    kind: DiscrepancyKind,
    snippet: Snippet,
    oracle_props: String,
    type_name: String,
}

impl DiscrepancyKind {
    fn kind_name(&self) -> &'static str {
        match self {
            DiscrepancyKind::TypeMismatch(_) => "type-mismatch",
            DiscrepancyKind::MissingInRust(_) => "missing-in-rust",
            DiscrepancyKind::ExtraInRust(_) => "extra-in-rust",
        }
    }
}

impl Discrepancy {
    fn byte_pos(&self) -> usize {
        match &self.kind {
            DiscrepancyKind::TypeMismatch(_) => 0,
            DiscrepancyKind::MissingInRust(at) => *at,
            DiscrepancyKind::ExtraInRust(at) => *at,
        }
    }

    fn print_context(&self, input: &str, show_hex: bool) {
        let pos = self.byte_pos();
        let syntax_t = snake_to_pascal(&format!("{}", self.oracle_key));
        let snippet = &self.snippet;

        println!("  {} at byte {} (tree: {})", self.kind, pos, self.tree_path,);
        println!("    Emacs type: {}  {}", syntax_t, self.oracle_props);
        println!("    Context: ...{}...", snippet);
        if show_hex {
            let lo = pos.saturating_sub(8);
            let hi = (pos + 8).min(input.len());
            print!("    Hex: ");
            for b in &input.as_bytes()[lo..hi] {
                print!("{b:02x} ");
            }
            println!();
            print!("    Asc: ");
            for b in &input.as_bytes()[lo..hi] {
                if b.is_ascii_graphic() || *b == b' ' {
                    print!(" {} ", *b as char);
                } else {
                    print!(" . ");
                }
            }
            println!();
        }
        println!();
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum DisplayMode {
    #[default]
    Text,
    Compact,
    Json,
    Csv,
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        for c in s.chars() {
            if c == '"' {
                out.push_str("\"\"");
            } else {
                out.push(c);
            }
        }
        out.push('"');
        out
    } else {
        s.to_string()
    }
}

#[derive(Default, Clone)]
struct DiscrepancyFilters {
    element_types: Option<Vec<String>>,
    kinds: Option<Vec<String>>,
    max_per_file: Option<usize>,
    summary_only: bool,
    mode: DisplayMode,
    show_hex: bool,
}

impl DiscrepancyFilters {
    fn apply<'a>(&self, ds: &'a [Discrepancy]) -> Vec<&'a Discrepancy> {
        ds.iter()
            .filter(|d| {
                if let Some(ref types) = self.element_types {
                    if !types.contains(&d.type_name) {
                        return false;
                    }
                }
                if let Some(ref kinds) = self.kinds {
                    if !kinds.iter().any(|k| k == d.kind.kind_name()) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }
}

fn rust_type_key(syntax: &Syntax) -> TypeKey {
    // For types where Emacs uses a finer-grained name than Rust's single
    // variant, emit the Emacs-compatible name so both sides share the same key.
    match syntax {
        Syntax::Script(flags) => intern_type(match flags.kind() {
            ScriptKind::Sub => "subscript",
            ScriptKind::Sup => "superscript",
        }),
        // Emacs names this `inlinetask` (one word); Rust's `InlineTask`
        // would normalise to `inline_task` and never match.
        Syntax::InlineTask(_) => intern_type("inlinetask"),
        _ => intern_type(&format!("{:?}", SyntaxT::from(syntax))),
    }
}

fn compare(
    oracle: &OracleNode,
    arena: &NodeArena,
    rust_id: NodeId,
    input: &str,
    file: &Path,
    path: &str,
    out: &mut Vec<Discrepancy>,
) {
    let emacs_key = oracle.type_key();
    let rust_key = rust_type_key(&arena[rust_id].data);

    if let Err(mismatch) = goof::assert_eq(&rust_key, &emacs_key) {
        let (begin, end) = oracle.span();
        out.push(Discrepancy {
            tree_path: path.to_owned(),
            oracle_key: emacs_key,
            kind: DiscrepancyKind::TypeMismatch(mismatch),
            snippet: Snippet::new(input, begin, end),
            oracle_props: oracle.to_string(),
            type_name: format!("{}", emacs_key),
        });
        return;
    }

    compare_children(
        &oracle.children,
        arena,
        &arena[rust_id].children,
        input,
        file,
        path,
        out,
    );
}

fn compare_children(
    oracle_kids: &[OracleNode],
    arena: &NodeArena,
    rust_kids: &[NodeId],
    input: &str,
    file: &Path,
    path: &str,
    out: &mut Vec<Discrepancy>,
) {
    type Key = (TypeKey, usize);

    let oracle_map: HashMap<Key, &OracleNode> = oracle_kids
        .iter()
        .map(|n| ((n.type_key(), n.span().0), n))
        .collect();

    let rust_map: HashMap<Key, NodeId> = rust_kids
        .iter()
        .map(|&id| {
            (
                (rust_type_key(&arena[id].data), arena[id].location.start),
                id,
            )
        })
        .collect();

    for (&key, oracle_node) in &oracle_map {
        if !rust_map.contains_key(&key) {
            let (begin, end) = oracle_node.span();
            out.push(Discrepancy {
                tree_path: format!("{}/{}", path, key.0),
                oracle_key: key.0,
                kind: DiscrepancyKind::MissingInRust(begin),
                snippet: Snippet::new(input, begin, end),
                oracle_props: oracle_node.to_string(),
                type_name: format!("{}", key.0),
            });
        }
    }

    for (&key, &rust_id) in &rust_map {
        if !oracle_map.contains_key(&key) {
            let node = &arena[rust_id];
            out.push(Discrepancy {
                tree_path: format!("{}/{}", path, key.0),
                oracle_key: key.0,
                kind: DiscrepancyKind::ExtraInRust(node.location.start),
                snippet: Snippet::new(input, node.location.start, node.location.end),
                oracle_props: String::new(),
                type_name: format!("{}", key.0),
            });
        }
    }

    for (&key, oracle_node) in &oracle_map {
        if let Some(&rust_id) = rust_map.get(&key) {
            compare(
                oracle_node,
                arena,
                rust_id,
                input,
                file,
                &format!("{}/{}", path, key.0),
                out,
            );
        }
    }
}

fn run_oracle(path: &Path) -> Result<String, CorpusError> {
    let oracle_el = Path::new(env!("CARGO_MANIFEST_DIR")).join("oracle.el");
    let out = std::process::Command::new("emacs")
        .args([
            "--batch",
            "-Q",
            "--load",
            oracle_el.to_str().ok_or(CorpusError::EmacsUnavailable)?,
            path.to_str().ok_or(CorpusError::EmacsUnavailable)?,
        ])
        .output()
        .map_err(|source| CorpusError::OracleExec {
            path: path.to_owned(),
            source,
        })?;
    String::from_utf8(out.stdout).map_err(|source| CorpusError::OracleUtf8 {
        path: path.to_owned(),
        source,
    })
}

fn timed_stage<T>(
    label: &str,
    display: &str,
    index: usize,
    total: usize,
    stderr_lock: &Arc<Mutex<()>>,
    work: impl FnOnce() -> T,
) -> (T, Duration) {
    // Only show in-place progress on a TTY.  When stderr is piped or
    // captured (CI, tools), `\r` does not overwrite — every poll would
    // concatenate into one long line.
    if !std::io::stderr().is_terminal() {
        let start = Instant::now();
        let result = work();
        return (result, start.elapsed());
    }

    let done = Arc::new(AtomicBool::new(false));
    let d = done.clone();
    let l = Arc::clone(stderr_lock);
    let dn = display.to_string();
    let lb = label.to_string();

    let timer = thread::spawn(move || {
        let start = Instant::now();
        // Wait before the first print so fast stages produce no output.
        let first_print_delay = Duration::from_millis(300);
        let poll_interval = Duration::from_millis(250);
        loop {
            if d.load(Ordering::Relaxed) {
                return;
            }
            if start.elapsed() >= first_print_delay {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        while !d.load(Ordering::Relaxed) {
            let elapsed = start.elapsed();
            let _g = l.lock().unwrap();
            eprint!(
                "\r[{}/{}] {}  {} {:.3}s  ",
                index,
                total,
                dn,
                lb,
                elapsed.as_secs_f64()
            );
            std::io::stderr().flush().ok();
            drop(_g);
            thread::sleep(poll_interval);
        }
    });

    let start = Instant::now();
    let result = work();
    let elapsed = start.elapsed();

    done.store(true, Ordering::Relaxed);
    timer.join().unwrap();
    (result, elapsed)
}

fn process_file(
    path: &Path,
    display: &str,
    index: usize,
    total: usize,
    stderr_lock: &Arc<Mutex<()>>,
) -> Result<(String, Vec<Discrepancy>, FileTimings), CorpusError> {
    let file_start = Instant::now();

    let raw = std::fs::read_to_string(path).map_err(|source| CorpusError::ReadFile {
        path: path.to_owned(),
        source,
    })?;
    // Emacs normalises CRLF line endings to LF when visiting a file.
    // Strip all CR bytes so our byte offsets match the oracle's character positions.
    let input = if raw.contains('\r') {
        raw.replace('\r', "")
    } else {
        raw
    };

    let (orc_result, emacs) = timed_stage("emacs", display, index, total, stderr_lock, || {
        run_oracle(path)
    });
    let sexp = orc_result?;

    let (root, sexp_t) = timed_stage("sexp", display, index, total, stderr_lock, || {
        lexpr::from_str(&sexp).map_err(|source| CorpusError::OracleSexp {
            path: path.to_owned(),
            source,
        })
    });
    let root = root?;

    let (oracle, oracle_conv) = timed_stage("oracle", display, index, total, stderr_lock, || {
        let table = CharByteTable::new(&input);
        from_value(&root, &table).ok_or_else(|| CorpusError::EmptyOracle {
            path: path.to_owned(),
        })
    });
    let oracle = oracle?;

    let bump = bumpalo::Bump::new();
    let rust_start = Instant::now();
    let mut parser = Parser::new(&input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root_id) = parser.parse_buffer();
    let rust = rust_start.elapsed();

    let (discrepancies, compare) = timed_stage("cmp", display, index, total, stderr_lock, || {
        let mut ds = Vec::new();
        compare(&oracle, &arena, root_id, &input, path, "root", &mut ds);
        ds
    });

    let total = file_start.elapsed();

    let input_for_return = input.clone();

    Ok((
        input_for_return,
        discrepancies,
        FileTimings {
            emacs,
            sexp: sexp_t,
            oracle_conv,
            rust,
            compare,
            total,
        },
    ))
}

fn print_usage() {
    let bin = std::env::args()
        .next()
        .unwrap_or_else(|| "corpus-check".into());
    eprintln!(
        "\
Usage: {bin} [OPTIONS]

Compares org-rs parser output against Emacs org-element for a
corpus of .org files, reporting any discrepancies found.

Options:
  -h, --help            Print this help message
  -c, --corpus <DIR>    Corpus directory (default: <repo_root>/corpus/)
  -f, --filter <PAT>    Only process files whose name contains <PAT>
                        (case-insensitive substring match)
  -t, --type <TYPES>    Only show discrepancies for given Emacs element types
                        (comma-separated, e.g. headline,paragraph,keyword)
  -k, --kind <KINDS>    Only show discrepancies of given kinds
                        (comma-separated: missing-in-rust,extra-in-rust,type-mismatch)
  -m, --max <N>         Max discrepancies to print per file
  -s, --summary         Only print file-level summary counts, not individual details
  --compact             Single-line per discrepancy (no context snippet)
  --json                JSONL output (one JSON object per discrepancy per line)
  --csv                 CSV output (header + one row per discrepancy)
  --hex                 Show hex dump around discrepancy position (text/compact only)

The tool requires:
  - `emacs` on PATH with org-element (built-in since Org 9.0)
  - `oracle.el` next to the binary or in CARGO_MANIFEST_DIR

Exit codes:
  0   All files matched the oracle (or no files to check)
  1   One or more discrepancies were found
"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut corpus_path = None;
    let mut filter: Option<String> = None;
    let mut filters = DiscrepancyFilters::default();

    {
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-h" | "--help" => {
                    print_usage();
                    std::process::exit(0);
                }
                "-c" | "--corpus" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("error: --corpus requires a path argument");
                        std::process::exit(1);
                    }
                    corpus_path = Some(PathBuf::from(&args[i]));
                }
                "-f" | "--filter" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("error: --filter requires a pattern argument");
                        std::process::exit(1);
                    }
                    filter = Some(args[i].to_lowercase());
                }
                "-t" | "--type" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("error: --type requires a comma-separated list");
                        std::process::exit(1);
                    }
                    filters.element_types = Some(
                        args[i]
                            .split(',')
                            .map(|s| normalize_type(s.trim()))
                            .collect(),
                    );
                }
                "-k" | "--kind" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("error: --kind requires a comma-separated list");
                        std::process::exit(1);
                    }
                    let valid = ["missing-in-rust", "extra-in-rust", "type-mismatch"];
                    let kinds: Vec<String> =
                        args[i].split(',').map(|s| s.trim().to_string()).collect();
                    for k in &kinds {
                        if !valid.contains(&k.as_str()) {
                            eprintln!("error: unknown kind '{k}' (valid: {})", valid.join(", "));
                            std::process::exit(1);
                        }
                    }
                    filters.kinds = Some(kinds);
                }
                "-m" | "--max" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("error: --max requires a number");
                        std::process::exit(1);
                    }
                    filters.max_per_file = Some(
                        args[i]
                            .parse()
                            .expect("error: --max requires a positive integer"),
                    );
                }
                "-s" | "--summary" => {
                    filters.summary_only = true;
                }
                "--compact" => {
                    filters.mode = DisplayMode::Compact;
                }
                "--json" => {
                    filters.mode = DisplayMode::Json;
                }
                "--csv" => {
                    filters.mode = DisplayMode::Csv;
                }
                "--hex" => {
                    filters.show_hex = true;
                }
                _ => {
                    eprintln!("error: unknown option '{}'", args[i]);
                    eprint!("usage: ");
                    print_usage();
                    std::process::exit(1);
                }
            }
            i += 1;
        }
    }

    let corpus = corpus_path.unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("corpus")
    });

    if !corpus.exists() {
        eprintln!("corpus/ not found at {}", corpus.display());
        std::process::exit(1);
    }

    let org_files: Vec<_> = std::fs::read_dir(&corpus)
        .expect("cannot read corpus/")
        .filter_map(|e| {
            let p = e.ok()?.path();
            if p.extension().and_then(|e| e.to_str()) != Some("org") {
                return None;
            }
            if let Some(ref pat) = filter {
                let name = p.file_name()?.to_str()?.to_lowercase();
                if !name.contains(pat.as_str()) {
                    return None;
                }
            }
            Some(p)
        })
        .collect();
    let n = org_files.len();
    let total_discrepancies = Arc::new(AtomicUsize::new(0));
    let stderr_lock = Arc::new(Mutex::new(()));
    let stdout_lock = Arc::new(Mutex::new(()));

    let filters_ref = &filters;
    let csv_header_printed = Arc::new(AtomicBool::new(false));
    std::thread::scope(|s| {
        #[allow(clippy::needless_range_loop)]
        for i in 0..org_files.len() {
            let path = &org_files[i];
            let dn = path.file_name().unwrap().to_str().unwrap().to_string();
            let g = stderr_lock.lock().unwrap();
            eprintln!("[{}/{}] {}", i + 1, n, dn);
            drop(g);

            let sl = Arc::clone(&stderr_lock);
            let sol = Arc::clone(&stdout_lock);
            let td = Arc::clone(&total_discrepancies);
            let chp = Arc::clone(&csv_header_printed);
            s.spawn(move || {
                let result = process_file(path, &dn, i + 1, n, &sl);

                let g = sl.lock().unwrap();
                match result {
                    Err(e) => eprintln!("  SKIP: {e}"),
                    Ok((input, ds, t)) => {
                        let emit_start = Instant::now();

                        let sg = sol.lock().unwrap();
                        if ds.is_empty() {
                            println!("OK  {}", path.display());
                        } else {
                            let filtered = filters_ref.apply(&ds);
                            let shown = filtered.len();
                            let count = if filters_ref.summary_only
                                || matches!(filters_ref.mode, DisplayMode::Json | DisplayMode::Csv)
                            {
                                filtered.len()
                            } else if let Some(max) = filters_ref.max_per_file {
                                shown.min(max)
                            } else {
                                shown
                            };

                            match filters_ref.mode {
                                DisplayMode::Text => {
                                    if filters_ref.summary_only {
                                        use std::collections::HashMap;
                                        let mut by_type: HashMap<&str, usize> = HashMap::new();
                                        let mut by_kind: HashMap<&str, usize> = HashMap::new();
                                        for d in &filtered {
                                            *by_type.entry(&d.type_name).or_default() += 1;
                                            *by_kind.entry(d.kind.kind_name()).or_default() += 1;
                                        }
                                        let mut type_parts: Vec<String> = by_type
                                            .into_iter()
                                            .map(|(t, c)| format!("{t}: {c}"))
                                            .collect();
                                        type_parts.sort();
                                        let mut kind_parts: Vec<String> = by_kind
                                            .into_iter()
                                            .map(|(k, c)| format!("{k}: {c}"))
                                            .collect();
                                        kind_parts.sort();
                                        println!(
                                            "FAIL {} ({} raw, {} filtered)",
                                            path.display(),
                                            ds.len(),
                                            filtered.len()
                                        );
                                        println!("  by type: {}", type_parts.join(", "));
                                        println!("  by kind: {}", kind_parts.join(", "));
                                    } else {
                                        println!("FAIL {} ({} discrepancies, {} filtered)",
                                            path.display(), ds.len(), filtered.len());
                                        for d in filtered.iter().take(count) {
                                            d.print_context(&input, filters_ref.show_hex);
                                        }
                                        if count < filtered.len() {
                                            println!("  ... and {} more (use --max to show more)", filtered.len() - count);
                                        }
                                    }
                                }
                                DisplayMode::Compact => {
                                    println!("FAIL {} ({} discrepancies, {} filtered)",
                                        path.display(), ds.len(), filtered.len());
                                    for d in filtered.iter().take(count) {
                                        let pos = d.byte_pos();
                                        println!("  {} {} @{} [{}]",
                                            d.kind.kind_name(), d.type_name, pos, d.tree_path);
                                        if filters_ref.show_hex {
                                            let lo = pos.saturating_sub(8);
                                            let hi = (pos + 8).min(input.len());
                                            print!("    Hex: ");
                                            for b in &input.as_bytes()[lo..hi] {
                                                print!("{b:02x} ");
                                            }
                                            println!();
                                            print!("    Asc: ");
                                            for b in &input.as_bytes()[lo..hi] {
                                                if b.is_ascii_graphic() || *b == b' ' {
                                                    print!(" {} ", *b as char);
                                                } else {
                                                    print!(" . ");
                                                }
                                            }
                                            println!();
                                        }
                                    }
                                    if count < filtered.len() {
                                        println!("  ... and {} more", filtered.len() - count);
                                    }
                                }
                                DisplayMode::Json => {
                                    for d in &filtered {
                                        let pos = d.byte_pos();
                                        println!(r#"{{"file":"{}","type":"{}","kind":"{}","byte_offset":{},"tree_path":"{}","context":"{}","props":"{}"}}"#,
                                            json_escape(&path.display().to_string()),
                                            json_escape(&d.type_name),
                                            json_escape(d.kind.kind_name()),
                                            pos,
                                            json_escape(&d.tree_path),
                                            json_escape(&d.snippet.to_string()),
                                            json_escape(&d.oracle_props));
                                    }
                                }
                                DisplayMode::Csv => {
                                    if !chp.swap(true, Ordering::Relaxed) {
                                        println!("file,type,kind,byte_offset,tree_path,context,props");
                                    }
                                    for d in &filtered {
                                        let pos = d.byte_pos();
                                        println!("{},{},{},{},{},{},{}",
                                            csv_field(&path.display().to_string()),
                                            csv_field(&d.type_name),
                                            csv_field(d.kind.kind_name()),
                                            pos,
                                            csv_field(&d.tree_path),
                                            csv_field(&d.snippet.to_string()),
                                            csv_field(&d.oracle_props));
                                    }
                                }
                            }
                            td.fetch_add(ds.len(), Ordering::Relaxed);
                        }
                        drop(sg);

                        let emit = emit_start.elapsed();
                        eprintln!(
                            "\n----\n\
                             parse: emacs: {:.3}s, org-rs: {:.3}s\n\
                             sexp: {:.3}s, oracle: {:.3}s\n\
                             emit: {:.3}s\n\
                             total: {:.3}s\n\
                             ----",
                            t.emacs.as_secs_f64(),
                            t.rust.as_secs_f64(),
                            t.sexp.as_secs_f64(),
                            t.oracle_conv.as_secs_f64(),
                            (t.compare + emit).as_secs_f64(),
                            (t.total + emit).as_secs_f64(),
                        );
                    }
                }
                drop(g);
            });
        }
    });

    if total_discrepancies.load(Ordering::Relaxed) > 0 {
        std::process::exit(1);
    }
}
