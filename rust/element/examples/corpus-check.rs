use goof::Error;
use lexpr::Value;
use org_element::data::{NodeArena, NodeId, ScriptKind, Syntax, SyntaxT};
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::io::Write;
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
    emit: Duration,
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

    let type_name = iter.next()?.as_symbol()?.as_ref();
    let type_key = intern_type(type_name);
    let plist_val = iter.next()?;

    let mut props_display = String::new();
    let mut begin = None;
    let mut end = None;

    if let Some(mut pl) = plist_val.list_iter() {
        while let Some(k) = pl.next() {
            let key = k.as_symbol()?;
            let val = pl.next()?;
            match key.as_ref() {
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
}

impl Discrepancy {
    fn print_context(&self) {
        let pos = match &self.kind {
            DiscrepancyKind::TypeMismatch(_) => 0usize,
            DiscrepancyKind::MissingInRust(at) => *at,
            DiscrepancyKind::ExtraInRust(at) => *at,
        };

        let syntax_t = snake_to_pascal(&format!("{}", self.oracle_key));
        let snippet = &self.snippet;

        println!("  {} at byte {} (tree: {})", self.kind, pos, self.tree_path,);
        println!("    Emacs type: {}  {}", syntax_t, self.oracle_props);
        println!("    Context: ...{}...", snippet);
        println!();
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
    let done = Arc::new(AtomicBool::new(false));
    let d = done.clone();
    let l = Arc::clone(stderr_lock);
    let dn = display.to_string();
    let lb = label.to_string();

    let timer = thread::spawn(move || {
        let start = Instant::now();
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
            thread::sleep(Duration::from_millis(50));
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
) -> Result<(Vec<Discrepancy>, FileTimings), CorpusError> {
    let file_start = Instant::now();

    let input = std::fs::read_to_string(path).map_err(|source| CorpusError::ReadFile {
        path: path.to_owned(),
        source,
    })?;

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

    Ok((
        discrepancies,
        FileTimings {
            emacs,
            sexp: sexp_t,
            oracle_conv,
            rust,
            compare,
            emit: Duration::ZERO,
            total,
        },
    ))
}

fn main() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("corpus");

    if !corpus.exists() {
        eprintln!("corpus/ not found at {}", corpus.display());
        std::process::exit(1);
    }

    let org_files: Vec<_> = std::fs::read_dir(&corpus)
        .expect("cannot read corpus/")
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension().and_then(|e| e.to_str()) == Some("org")).then_some(p)
        })
        .collect();
    let n = org_files.len();
    let total_discrepancies = Arc::new(AtomicUsize::new(0));
    let stderr_lock = Arc::new(Mutex::new(()));
    let stdout_lock = Arc::new(Mutex::new(()));

    std::thread::scope(|s| {
        for i in 0..org_files.len() {
            let path = &org_files[i];
            let dn = path.file_name().unwrap().to_str().unwrap().to_string();
            let g = stderr_lock.lock().unwrap();
            eprintln!("[{}/{}] {}", i + 1, n, dn);
            drop(g);

            let sl = Arc::clone(&stderr_lock);
            let sol = Arc::clone(&stdout_lock);
            let td = Arc::clone(&total_discrepancies);
            s.spawn(move || {
                let result = process_file(path, &dn, i + 1, n, &sl);

                let g = sl.lock().unwrap();
                match result {
                    Err(e) => eprintln!("  SKIP: {e}"),
                    Ok((ds, t)) => {
                        let emit_start = Instant::now();

                        let sg = sol.lock().unwrap();
                        if ds.is_empty() {
                            println!("OK  {}", path.display());
                        } else {
                            println!("FAIL {} ({} discrepancies)", path.display(), ds.len());
                            for d in &ds {
                                d.print_context();
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
