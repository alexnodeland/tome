//! Symbol extraction (S2-6, spec P2-015).
//!
//! # Where symbols actually are, measured
//!
//! P2-015's technical note extracts declarations from **code blocks**, with a
//! per-language table of `fn\s+(\w+)`-style patterns. Run over the 339-page
//! relevance corpus, that finds 2 821 declarations whose most common names are:
//!
//! ```text
//! main, buf, server, Foo, foo, __init__, char, import, myURL, req, options
//! ```
//!
//! Those are the *examples'* scaffolding, not the API being documented. The
//! symbols users actually search for score close to zero as declarations:
//! `Vec` is declared once and mentioned 321 times, `with_capacity` is declared
//! **never** and mentioned 30 times, `os.cpus` never at all. A `symbols` field
//! filled from code blocks would be almost pure noise, and P2-015's own
//! "no false positives for prose" criterion would fail on its own technical
//! note.
//!
//! The patterns are right; the *place* is wrong. Documentation generators put
//! signatures in **headings**:
//!
//! ```text
//! path : std/vec/struct.Vec.html
//! title: Struct Vec
//! h1: Struct Vec
//! h3: impl<T> Vec<T>
//! h4: pub const fn new() -> Vec<T>
//! h4: pub fn with_capacity(capacity: usize) -> Vec<T>
//! ```
//!
//! So extraction reads three things, in descending order of trust:
//!
//! 1. **The page path.** rustdoc encodes the kind in the filename —
//!    `struct.Vec.html`, `fn.read_to_string.html`. Unambiguous and free.
//! 2. **The title.** `Struct Vec`, `Function read_to_string`, `Module os`.
//! 3. **Headings**, for the declarations a reference page lists — methods,
//!    associated functions, `def`/`class` on Sphinx pages, and bare
//!    symbol-shaped headings like `os.cpus()` on Node's.
//!
//! This is also why S2-4 measured the `code` field as barely load-bearing:
//! `headers` was already carrying the method names, because `extract`'s
//! `inline_text` folds a heading's `<code>` children into the heading value.

use crate::model::Node;

/// What a symbol *is*, for P2-015's "symbol type in results".
///
/// Deliberately coarse. The distinctions that survive across Rust, Python,
/// JavaScript, Go and C are function/type/trait/module/constant/macro; finer
/// ones (`struct` vs `enum` vs `union`) are per-language and a result list has
/// no room to show them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Type,
    Trait,
    Module,
    Constant,
    Macro,
}

impl SymbolKind {
    /// Stable short form, for the stored field. Short because it is written
    /// once per page and SPIKE-003 finding 2 makes index size a running cost.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Type => "type",
            Self::Trait => "trait",
            Self::Module => "module",
            Self::Constant => "constant",
            Self::Macro => "macro",
        }
    }

    /// Parse the stored form back. `None` for anything unrecognised, which is
    /// what an index written by a newer build looks like.
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "function" => Self::Function,
            "type" => Self::Type,
            "trait" => Self::Trait,
            "module" => Self::Module,
            "constant" => Self::Constant,
            "macro" => Self::Macro,
            _ => return None,
        })
    }

    /// The keyword that introduces this kind of declaration, across the
    /// languages in scope. P2-015's "recognize patterns: fn, struct, class,
    /// def, function" lives here.
    fn from_keyword(word: &str) -> Option<Self> {
        Some(match word {
            // Functions and methods.
            "fn" | "def" | "function" | "func" | "method" | "async" => Self::Function,
            // Types.
            "struct" | "class" | "enum" | "union" | "type" | "interface" | "record" => Self::Type,
            "trait" | "protocol" => Self::Trait,
            "mod" | "module" | "package" | "namespace" => Self::Module,
            "const" | "static" | "constant" | "var" | "let" => Self::Constant,
            "macro" | "macro_rules" => Self::Macro,
            _ => return None,
        })
    }
}

/// One symbol declared by a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
}

/// Everything [`extract`] found.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Symbols {
    /// The symbol this page *is about*, if it is a reference page for one.
    ///
    /// Separate from [`all`](Self::all) because it is the one a result list
    /// should label, and because it is the only one worth storing: one short
    /// string per page rather than a list that grows with the page.
    pub primary: Option<Symbol>,
    /// Every symbol the page declares, primary included. Indexed for matching.
    pub all: Vec<Symbol>,
}

impl Symbols {
    /// The primary symbol's name, or empty if the page is not a reference
    /// page for one.
    pub fn primary_name(&self) -> &str {
        self.primary
            .as_ref()
            .map_or("", |symbol| symbol.name.as_str())
    }

    /// Every name, space-separated, as the `declarations` field stores them.
    pub fn names(&self) -> String {
        let mut out = String::new();
        for symbol in &self.all {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&symbol.name);
        }
        out
    }
}

/// Longest heading that can still be a declaration, in characters.
///
/// A signature is a line; a paragraph that happens to start with the word
/// "Function" is prose. Rust signatures with several generic parameters are
/// the long end of what is real, and they fit comfortably.
const MAX_SIGNATURE_CHARS: usize = 200;

/// Extract a page's symbols from its path, title and headings.
///
/// `content` is the stored AST. Code blocks are **not** read — see the module
/// docs for the measurement that decided that.
pub fn extract(path: &str, title: &str, content: &Node) -> Symbols {
    let mut out = Symbols::default();
    let mut seen = std::collections::BTreeSet::new();

    let mut add = |symbol: Option<Symbol>, out: &mut Symbols| {
        let Some(symbol) = symbol else { return };
        if symbol.name.is_empty() || !seen.insert(symbol.name.to_lowercase()) {
            return;
        }
        out.all.push(symbol);
    };

    // 1. The path. rustdoc puts the kind in the filename, which makes this the
    //    only unambiguous source of the three.
    let from_path = from_path(path);
    // 2. The title. `Struct Vec`, `Function read_to_string`.
    let from_title = from_declaration(title, Style::Title);

    // The path wins when both fire: a title is prose that happens to start
    // with a keyword often enough to matter, and `struct.Vec.html` cannot be
    // anything else.
    out.primary = from_path.clone().or_else(|| from_title.clone());
    add(from_path, &mut out);
    add(from_title, &mut out);

    // 3. Headings, for the declarations a reference page lists.
    let mut headings = Vec::new();
    collect_headings(content, &mut headings);
    for heading in headings {
        add(from_declaration(&heading, Style::Heading), &mut out);
    }

    out
}

/// A rustdoc-style path: `std/vec/struct.Vec.html` → `Vec`, a type.
///
/// Matched on the *file stem* rather than anywhere in the path, so a directory
/// called `enum` cannot make every page under it an enum.
fn from_path(path: &str) -> Option<Symbol> {
    let file = path.rsplit('/').next()?;
    let stem = file.strip_suffix(".html").unwrap_or(file);
    let (kind, name) = stem.split_once('.')?;
    // The name must not itself contain a dot: `struct.Vec` is a symbol page,
    // `index.min.html` is a minified file that happens to split the same way.
    if name.is_empty() || name.contains('.') {
        return None;
    }
    let kind = match kind {
        "struct" | "enum" | "union" | "primitive" | "type" => SymbolKind::Type,
        "fn" | "method" => SymbolKind::Function,
        "trait" | "derive" => SymbolKind::Trait,
        "mod" => SymbolKind::Module,
        "constant" | "static" => SymbolKind::Constant,
        "macro" => SymbolKind::Macro,
        _ => return None,
    };
    Some(Symbol {
        name: name.to_owned(),
        kind,
    })
}

/// Where a candidate declaration came from.
///
/// It changes what counts, which is not a nicety. rustdoc titles are exactly
/// `Struct Vec` — a capitalised kind and a name — and rustdoc *headings*
/// include `Trait Implementations` and `Auto Trait Implementations`, which fit
/// the same shape and are prose. Applying the title rule to headings put
/// `Implementations` in the symbol field of 35 of the corpus's 84 rust-std
/// pages, which the extraction report caught.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// A page title. May use the capitalised `Kind Name` form.
    Title,
    /// A heading or definition term. Must look like source: a lowercase
    /// keyword, as it would be written in the language.
    Heading,
}

/// A declaration in a heading or title: `pub fn with_capacity(cap: usize)`,
/// `Struct Vec`, `class Foo(Base)`, `def open(file, mode='r')`.
///
/// Returns `None` for prose, which is most headings. The guards below are the
/// difference between a symbol field and a second copy of `headers`.
fn from_declaration(text: &str, style: Style) -> Option<Symbol> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > MAX_SIGNATURE_CHARS {
        return None;
    }

    // A capitalised keyword is *title* style — `Struct Vec`, `Module os` —
    // where the whole string is exactly the kind and the name. A lowercase one
    // is source style — `pub fn with_capacity(cap: usize) -> Vec<T>` — which
    // runs on for as long as the signature does.
    //
    // The two are separated because title style, allowed to run on, turns
    // every prose heading that opens with a keyword into a symbol: `Type
    // Conversions` would declare `Conversions`. Requiring exactly two words
    // costs the handful of three-word rustdoc titles and buys back the whole
    // category.
    let words: Vec<&str> = text.split_whitespace().collect();
    if style == Style::Title {
        if let [kind_word, name_word] = words.as_slice() {
            if let Some(kind) = kind_word
                .chars()
                .next()
                .filter(|c| c.is_uppercase())
                .and_then(|_| SymbolKind::from_keyword(&kind_word.to_lowercase()))
            {
                if let Some(name) = identifier(name_word) {
                    return Some(Symbol { name, kind });
                }
            }
        }
    }

    // A bare call signature, with no keyword at all. Node and Python write
    // their reference headings this way — `os.cpus()`, `path.join([...paths])`,
    // `path.basename(path[, suffix])` — and without this the whole of Node's
    // API extracts one symbol.
    if style == Style::Heading {
        if let Some(symbol) = from_call_signature(text) {
            return Some(symbol);
        }
    }

    let mut kind = None;
    for word in text.split_whitespace() {
        // Visibility and modifiers sit before the keyword in most languages.
        let word = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        if matches!(
            word,
            "pub" | "unsafe" | "async" | "const" | "extern" | "impl" | "export" | "default"
        ) && kind.is_none()
        {
            // `const fn` is a function; a bare `const` is a constant. Deciding
            // on the *last* keyword seen would get that backwards, so
            // modifiers are skipped rather than matched — except that `const`
            // is both, so it is allowed to fall through to `from_keyword`
            // below if nothing else follows.
            if word == "const" {
                kind = Some(SymbolKind::Constant);
            }
            continue;
        }

        if let Some(found) = SymbolKind::from_keyword(word) {
            // A later, more specific keyword replaces an earlier modifier:
            // `pub const fn new()` is a function.
            kind = Some(found);
            continue;
        }

        // The first word that is not a keyword or a modifier is the name.
        let kind = kind?;
        let name = identifier(word)?;
        return Some(Symbol { name, kind });
    }
    None
}

/// A heading that is nothing but a call signature: `os.cpus()`,
/// `path.basename(path[, suffix])`.
///
/// The test is that **everything before the opening parenthesis is a qualified
/// identifier with no whitespace in it**, which is what separates a signature
/// from a prose heading that happens to contain a bracket. `Sorting (advanced)`
/// has a space before its parenthesis and is rejected; `os.cpus()` does not and
/// is not.
///
/// Requiring the parenthesis at all is the other half. Node also heads its
/// *properties* this way — `os.EOL`, `path.delimiter` — and admitting those
/// means admitting every prose heading that happens to be one dotted word,
/// `Node.js` among them. Functions are what symbol search is for.
fn from_call_signature(text: &str) -> Option<Symbol> {
    let open = text.find('(')?;
    let before = &text[..open];
    if before.is_empty() || before.chars().any(char::is_whitespace) {
        return None;
    }
    let name = identifier(before)?;
    // `identifier` stops at the first character it does not accept, so this
    // also rejects `a+b(` and `foo::bar(` — the latter deliberately, since the
    // code tokenizer already splits `::` and a heading in that form is a Rust
    // signature that the keyword path handles.
    if name.len() != before.len() {
        return None;
    }
    Some(Symbol {
        name,
        kind: SymbolKind::Function,
    })
}

/// The leading identifier of a token, or `None` if there is not one.
///
/// `with_capacity(capacity:` → `with_capacity`; `Vec<T>` → `Vec`;
/// `os.cpus()` → `os.cpus`, because a qualified name is how people search for
/// Node and Python APIs and splitting it would lose the qualifier.
fn identifier(word: &str) -> Option<String> {
    let mut out = String::new();
    for character in word.chars() {
        if character.is_alphanumeric() || character == '_' || character == '.' {
            out.push(character);
        } else {
            break;
        }
    }
    let trimmed = out.trim_matches('.');
    // A name has to start with something a name can start with, or `1.2` and
    // `3` become symbols.
    let first = trimmed.chars().next()?;
    if !first.is_alphabetic() && first != '_' {
        return None;
    }
    Some(trimmed.to_owned())
}

/// Every heading's text, in document order.
///
/// Deliberately duplicated from [`super::extract`] rather than shared: that
/// one folds headings into the search fields and this one needs them
/// individually, and threading a second output through it would couple two
/// things that change for different reasons.
fn collect_headings(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::Heading { children, .. } => {
            let mut text = String::new();
            for child in children {
                super::extract::inline_text_for_symbols(child, &mut text);
            }
            let text = text.trim();
            if !text.is_empty() {
                out.push(text.to_owned());
            }
        }
        Node::Document { children }
        | Node::Paragraph { children }
        | Node::Blockquote { children }
        | Node::Emphasis { children }
        | Node::Strong { children }
        | Node::Admonition { children, .. }
        | Node::Link { children, .. } => {
            for child in children {
                collect_headings(child, out);
            }
        }
        Node::List { items, .. } => {
            for item in items {
                for child in &item.children {
                    collect_headings(child, out);
                }
            }
        }
        Node::DefinitionList { items } => {
            for item in items {
                // Sphinx puts the API signature in the definition term:
                // `open(file, mode='r')`. It is the highest-value string on a
                // Python reference page and it is not a heading.
                let mut text = String::new();
                for child in &item.term {
                    super::extract::inline_text_for_symbols(child, &mut text);
                }
                let text = text.trim();
                if !text.is_empty() {
                    out.push(text.to_owned());
                }
                for child in &item.definition {
                    collect_headings(child, out);
                }
            }
        }
        Node::Table { headers, rows } => {
            for cell in headers {
                for child in &cell.children {
                    collect_headings(child, out);
                }
            }
            for row in rows {
                for cell in &row.cells {
                    for child in &cell.children {
                        collect_headings(child, out);
                    }
                }
            }
        }
        // Code blocks are deliberately not read; see the module docs.
        Node::CodeBlock { .. }
        | Node::InlineCode { .. }
        | Node::Text { .. }
        | Node::Image { .. }
        | Node::Anchor { .. }
        | Node::ThematicBreak {}
        | Node::LineBreak {} => {}
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn heading(level: u8, value: &str) -> Node {
        Node::Heading {
            level,
            id: None,
            children: vec![Node::Text {
                value: value.to_owned(),
            }],
        }
    }

    fn document(children: Vec<Node>) -> Node {
        Node::Document { children }
    }

    #[test]
    fn rustdoc_paths_carry_the_kind() {
        for (path, name, kind) in [
            ("std/vec/struct.Vec.html", "Vec", SymbolKind::Type),
            (
                "std/fs/fn.read_to_string.html",
                "read_to_string",
                SymbolKind::Function,
            ),
            (
                "std/iter/trait.Iterator.html",
                "Iterator",
                SymbolKind::Trait,
            ),
            ("std/macro.vec.html", "vec", SymbolKind::Macro),
            ("std/primitive.str.html", "str", SymbolKind::Type),
        ] {
            assert_eq!(
                from_path(path),
                Some(Symbol {
                    name: name.to_owned(),
                    kind
                }),
                "{path}"
            );
        }
    }

    #[test]
    fn ordinary_paths_are_not_symbols() {
        // The guard that keeps this from labelling the whole library. A page
        // is only a symbol page if its filename says so.
        for path in [
            "index.html",
            "guide/dependencies.html",
            "api/os.html",
            "3/tutorial/errors.html",
            // Splits on `.` the same way a symbol page does and must not be
            // mistaken for one.
            "assets/index.min.html",
            "cargo/reference/manifest.html",
        ] {
            assert_eq!(from_path(path), None, "{path}");
        }
    }

    #[test]
    fn rustdoc_signatures_in_headings_are_declarations() {
        // The measurement that reshaped this ticket: these are headings on
        // `struct.Vec.html`, and they are where the methods actually live.
        for (text, name, kind) in [
            ("pub const fn new() -> Vec<T>", "new", SymbolKind::Function),
            (
                "pub fn with_capacity(capacity: usize) -> Vec<T>",
                "with_capacity",
                SymbolKind::Function,
            ),
            (
                "pub unsafe fn from_raw_parts(ptr: *mut T) -> Vec<T>",
                "from_raw_parts",
                SymbolKind::Function,
            ),
            ("Struct Vec", "Vec", SymbolKind::Type),
            (
                "Function read_to_string",
                "read_to_string",
                SymbolKind::Function,
            ),
            ("Trait Iterator", "Iterator", SymbolKind::Trait),
            ("Module os", "os", SymbolKind::Module),
        ] {
            assert_eq!(
                from_declaration(text, Style::Title),
                Some(Symbol {
                    name: name.to_owned(),
                    kind
                }),
                "{text:?}"
            );
        }
    }

    #[test]
    fn other_languages_declarations_are_recognised() {
        for (text, name, kind) in [
            ("def open(file, mode='r')", "open", SymbolKind::Function),
            ("class Counter(dict)", "Counter", SymbolKind::Type),
            (
                "function parseInt(string, radix)",
                "parseInt",
                SymbolKind::Function,
            ),
            (
                "func Fprintf(w io.Writer) (n int, err error)",
                "Fprintf",
                SymbolKind::Function,
            ),
            ("type Reader interface", "Reader", SymbolKind::Type),
            (
                "const MAX_SAFE_INTEGER",
                "MAX_SAFE_INTEGER",
                SymbolKind::Constant,
            ),
        ] {
            assert_eq!(
                from_declaration(text, Style::Title),
                Some(Symbol {
                    name: name.to_owned(),
                    kind
                }),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_const_fn_is_a_function_not_a_constant() {
        // `const` is both a modifier and a keyword, which is the one place
        // the "first keyword wins" rule would get the answer backwards.
        assert_eq!(
            from_declaration("pub const fn new() -> Vec<T>", Style::Heading).map(|s| s.kind),
            Some(SymbolKind::Function)
        );
        assert_eq!(
            from_declaration("const MAX: usize", Style::Heading).map(|s| s.kind),
            Some(SymbolKind::Constant)
        );
    }

    #[test]
    fn prose_headings_are_not_declarations() {
        // P2-015's "no false positives for prose". Every one of these is a
        // real heading from the relevance corpus.
        for text in [
            "Examples",
            "Errors",
            "Capacity and reallocation",
            "Installation",
            "Windows vs. POSIX",
            "Optimizing Build Performance",
            "The Manifest Format",
            "Using the module",
            "Concepts",
            "Overview",
            "Guarantees",
            "Indexing",
        ] {
            assert_eq!(
                from_declaration(text, Style::Title),
                None,
                "{text:?} is prose"
            );
        }
    }

    #[test]
    fn a_paragraph_that_opens_with_a_keyword_is_not_a_declaration() {
        // "Functions are first-class objects in Python. They can be…" starts
        // with a keyword and is prose. Length is the discriminator, because a
        // signature is a line.
        let prose = format!("function {}", "word ".repeat(60));
        assert!(prose.chars().count() > MAX_SIGNATURE_CHARS);
        assert_eq!(from_declaration(&prose, Style::Heading), None);
    }

    #[test]
    fn qualified_names_keep_their_qualifier() {
        // How people search Node and Python APIs. Splitting `os.cpus` into
        // `os` and `cpus` would lose the thing that makes it findable.
        assert_eq!(identifier("os.cpus()"), Some("os.cpus".to_owned()));
        assert_eq!(
            identifier("path.join(...paths)"),
            Some("path.join".to_owned())
        );
        assert_eq!(identifier("Vec<T>"), Some("Vec".to_owned()));
        assert_eq!(
            identifier("with_capacity(capacity:"),
            Some("with_capacity".to_owned())
        );
    }

    #[test]
    fn numbers_and_punctuation_are_not_identifiers() {
        assert_eq!(identifier("1.2.3"), None);
        assert_eq!(identifier("->"), None);
        assert_eq!(identifier(""), None);
        assert_eq!(identifier("_private"), Some("_private".to_owned()));
    }

    #[test]
    fn the_page_path_outranks_the_title_for_the_primary_symbol() {
        // A title is prose that starts with a keyword often enough to matter;
        // `struct.Vec.html` cannot be anything else.
        let symbols = extract(
            "std/vec/struct.Vec.html",
            "Struct Vec",
            &document(vec![heading(1, "Struct Vec")]),
        );
        assert_eq!(
            symbols.primary,
            Some(Symbol {
                name: "Vec".to_owned(),
                kind: SymbolKind::Type
            })
        );
    }

    #[test]
    fn a_reference_page_yields_its_methods() {
        let symbols = extract(
            "std/vec/struct.Vec.html",
            "Struct Vec",
            &document(vec![
                heading(1, "Struct Vec"),
                heading(2, "Examples"),
                heading(3, "impl<T> Vec<T>"),
                heading(4, "pub const fn new() -> Vec<T>"),
                heading(4, "pub fn with_capacity(capacity: usize) -> Vec<T>"),
                heading(2, "Guarantees"),
            ]),
        );
        let names = symbols.names();
        assert!(names.contains("Vec"), "{names}");
        assert!(names.contains("new"), "{names}");
        assert!(names.contains("with_capacity"), "{names}");
        // And nothing from the prose headings.
        assert!(!names.contains("Examples"), "{names}");
        assert!(!names.contains("Guarantees"), "{names}");
    }

    #[test]
    fn a_symbol_is_not_repeated() {
        // `Vec` is in the path, the title and the h1. It should be one entry,
        // or term frequency in the symbols field becomes a measure of how
        // repetitive a documentation generator is.
        let symbols = extract(
            "std/vec/struct.Vec.html",
            "Struct Vec",
            &document(vec![heading(1, "Struct Vec"), heading(2, "Struct Vec")]),
        );
        assert_eq!(
            symbols.all.iter().filter(|s| s.name == "Vec").count(),
            1,
            "got {:?}",
            symbols.all
        );
    }

    #[test]
    fn a_prose_page_has_no_symbols() {
        let symbols = extract(
            "guide/dependencies.html",
            "Dependencies",
            &document(vec![
                heading(1, "Dependencies"),
                heading(2, "Adding a dependency"),
            ]),
        );
        assert_eq!(symbols.primary, None);
        assert!(symbols.all.is_empty(), "got {:?}", symbols.all);
    }

    #[test]
    fn sphinx_definition_terms_are_declarations() {
        // Python's reference pages put the signature in a definition term
        // rather than a heading, so it is read from there too.
        let symbols = extract(
            "3/library/functions.html",
            "Built-in Functions",
            &document(vec![Node::DefinitionList {
                items: vec![crate::model::Definition {
                    id: None,
                    term: vec![Node::InlineCode {
                        code: "def open(file, mode='r')".to_owned(),
                    }],
                    definition: vec![],
                }],
            }]),
        );
        assert!(symbols.names().contains("open"), "{:?}", symbols.all);
    }

    #[test]
    fn kinds_round_trip_through_their_stored_form() {
        for kind in [
            SymbolKind::Function,
            SymbolKind::Type,
            SymbolKind::Trait,
            SymbolKind::Module,
            SymbolKind::Constant,
            SymbolKind::Macro,
        ] {
            assert_eq!(SymbolKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(SymbolKind::parse("something-newer"), None);
        assert_eq!(SymbolKind::parse(""), None);
    }
}
