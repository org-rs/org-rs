use goof::Error;
use lexpr::Value;
use org_element::data::{NodeArena, NodeId, ScriptKind, Syntax, SyntaxT};
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

#[derive(Debug, Error)]
enum CorpusError {
    #[error("failed to read {path:?}: {source}")]
    ReadFile { path: PathBuf, #[source] source: std::io::Error },
    #[error("emacs oracle could not be launched for {path:?}: {source}")]
    OracleExec { path: PathBuf, #[source] source: std::io::Error },
    #[error("oracle output for {path:?} is not valid UTF-8: {source}")]
    OracleUtf8 { path: PathBuf, #[source] source: std::string::FromUtf8Error },
    #[error("oracle S-expression for {path:?} did not parse: {source}")]
    OracleSexp { path: PathBuf, #[source] source: lexpr::parse::Error },
    #[error("oracle output for {path:?} did not yield a node")]
    EmptyOracle { path: PathBuf },
    #[error("emacs is not in PATH or oracle.el is missing")]
    EmacsUnavailable,
}

struct TypeInterner {
    strings: Vec<Box<str>>,
    index: HashMap<Box<str>, u32>,
}

impl TypeInterner {
    fn new() -> Self {
        TypeInterner { strings: Vec::new(), index: HashMap::new() }
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
                None    => String::new(),
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
    fn char_index(self) -> usize { (self.0.get() - 1) as usize }
}

/// 0-based byte offset into the file content, as used by the Rust parser.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ByteOffset(usize);

impl ByteOffset {
    #[inline]
    fn as_usize(self) -> usize { self.0 }
}

/// Converts Emacs character positions to byte offsets by walking the source
/// string on each call.  No allocation — O(file_size) per lookup.
struct CharByteTable<'a>(&'a str);

impl<'a> CharByteTable<'a> {
    fn new(s: &'a str) -> Self { CharByteTable(s) }

    fn to_byte(&self, pos: EmacsCharPos) -> ByteOffset {
        let idx = pos.char_index();
        ByteOffset(
            self.0.char_indices().nth(idx)
                .map(|(b, _)| b)
                .unwrap_or(self.0.len()),
        )
    }
}

struct Snippet(String);

impl Snippet {
    fn new(input: &str, begin: usize, end: usize) -> Self {
        let lo_raw = begin.saturating_sub(20);
        let hi_raw = (end + 20).min(input.len());
        // Snap to valid char boundaries so we don't panic on multi-byte chars.
        let lo = (0..=lo_raw).rev().find(|&i| input.is_char_boundary(i)).unwrap_or(0);
        let hi = (hi_raw..=input.len()).find(|&i| input.is_char_boundary(i)).unwrap_or(input.len());
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
    node_type: String,
    begin: Option<ByteOffset>,
    end: Option<ByteOffset>,
    props: HashMap<String, Value>,
    children: Vec<OracleNode>,
}

impl OracleNode {
    fn type_key(&self) -> TypeKey {
        intern_type(&self.node_type)
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
        write!(f, "({}", self.node_type)?;
        let mut keys: Vec<_> = self.props.keys().collect();
        keys.sort();
        for k in keys {
            write!(f, " {k} {:?}", self.props[k])?;
        }
        write!(f, ")")
    }
}

fn from_value(v: &Value, table: &CharByteTable<'_>) -> Option<OracleNode> {
    let mut iter = v.list_iter()?;

    let node_type = iter.next()?.as_symbol()?.to_string();
    let plist_val = iter.next()?;

    let mut props = HashMap::new();
    let mut begin = None;
    let mut end   = None;

    if let Some(mut pl) = plist_val.list_iter() {
        while let Some(k) = pl.next() {
            let key = k.as_symbol()?.to_string();
            let val = pl.next()?.clone();
            match key.as_str() {
                ":begin" => begin = val.as_u64().and_then(NonZeroU64::new)
                                       .map(|n| table.to_byte(EmacsCharPos(n))),
                ":end"   => end   = val.as_u64().and_then(NonZeroU64::new)
                                       .map(|n| table.to_byte(EmacsCharPos(n))),
                _        => { props.insert(key, val); }
            }
        }
    }

    let children = iter
        .filter(|v| !v.is_string() && !v.is_symbol() && !v.is_null())
        .filter_map(|v| from_value(v, table))
        .collect();

    Some(OracleNode { node_type, begin, end, props, children })
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
    file: PathBuf,
    tree_path: String,
    oracle_key: TypeKey,
    kind: DiscrepancyKind,
    snippet: Snippet,
    oracle_props: String,
}

impl Discrepancy {
    fn print_test(&self) {
        let stem = self.file
            .file_stem().unwrap()
            .to_string_lossy()
            .replace(['-', ' ', '.'], "_");

        let pos = match &self.kind {
            DiscrepancyKind::TypeMismatch(_) => 0usize,
            DiscrepancyKind::MissingInRust(at) => *at,
            DiscrepancyKind::ExtraInRust(at)   => *at,
        };

        let fn_name  = format!("corpus_{}_{pos}", stem);
        let syntax_t = snake_to_pascal(&format!("{}", self.oracle_key));
        let snippet  = &self.snippet;

        println!(
            r#"
    #[test]
    fn {fn_name}() {{
        // Source: {}  path: {}
        // {}  {}
        let input = {snippet:?};
        let count = get_type_count(input, SyntaxT::{syntax_t}, ParseGranularity::Element);
        assert!(count > 0, "expected at least one {syntax_t} node");
    }}"#,
            self.file.display(),
            self.tree_path,
            self.kind,
            self.oracle_props,
        );
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
    let rust_key  = rust_type_key(&arena[rust_id].data);

    if let Err(mismatch) = goof::assert_eq(&rust_key, &emacs_key) {
        let (begin, end) = oracle.span();
        out.push(Discrepancy {
            file: file.to_owned(),
            tree_path: path.to_owned(),
            oracle_key: emacs_key,
            kind: DiscrepancyKind::TypeMismatch(mismatch),
            snippet: Snippet::new(input, begin, end),
            oracle_props: oracle.to_string(),
        });
        return;
    }

    compare_children(&oracle.children, arena, &arena[rust_id].children, input, file, path, out);
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
        .map(|&id| ((rust_type_key(&arena[id].data), arena[id].location.start), id))
        .collect();

    for (&key, oracle_node) in &oracle_map {
        if !rust_map.contains_key(&key) {
            let (begin, end) = oracle_node.span();
            out.push(Discrepancy {
                file: file.to_owned(),
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
                file: file.to_owned(),
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
            compare(oracle_node, arena, rust_id, input, file, &format!("{}/{}", path, key.0), out);
        }
    }
}

fn run_oracle(path: &Path) -> Result<String, CorpusError> {
    let oracle_el = Path::new(env!("CARGO_MANIFEST_DIR")).join("oracle.el");
    let out = std::process::Command::new("emacs")
        .args([
            "--batch", "-Q",
            "--load", oracle_el.to_str().ok_or(CorpusError::EmacsUnavailable)?,
            path.to_str().ok_or(CorpusError::EmacsUnavailable)?,
        ])
        .output()
        .map_err(|source| CorpusError::OracleExec { path: path.to_owned(), source })?;
    String::from_utf8(out.stdout)
        .map_err(|source| CorpusError::OracleUtf8 { path: path.to_owned(), source })
}

fn process_file(path: &Path) -> Result<Vec<Discrepancy>, CorpusError> {
    let input  = std::fs::read_to_string(path)
        .map_err(|source| CorpusError::ReadFile { path: path.to_owned(), source })?;
    let sexp   = run_oracle(path)?;
    let root   = lexpr::from_str(&sexp)
        .map_err(|source| CorpusError::OracleSexp { path: path.to_owned(), source })?;
    let table  = CharByteTable::new(&input);
    let oracle = from_value(&root, &table)
        .ok_or_else(|| CorpusError::EmptyOracle { path: path.to_owned() })?;

    let mut parser = Parser::new(&input, ParseGranularity::Object, DefaultEnvironment);
    let (arena, root_id) = parser.parse_buffer();

    let mut discrepancies = Vec::new();
    compare(&oracle, &arena, root_id, &input, path, "root", &mut discrepancies);
    Ok(discrepancies)
}

fn main() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("corpus");

    if !corpus.exists() {
        eprintln!("corpus/ not found at {}", corpus.display());
        std::process::exit(1);
    }

    let mut total = 0usize;

    for entry in std::fs::read_dir(&corpus).expect("cannot read corpus/") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("org") {
            continue;
        }

        match process_file(&path) {
            Err(e) => eprintln!("SKIP {}: {e}", path.display()),
            Ok(ds) if ds.is_empty() => println!("OK  {}", path.display()),
            Ok(ds) => {
                println!("FAIL {} ({} discrepancies)", path.display(), ds.len());
                for d in &ds {
                    eprintln!("  {:?}", d.kind);
                    d.print_test();
                }
                total += ds.len();
            }
        }
    }

    if total > 0 {
        std::process::exit(1);
    }
}
