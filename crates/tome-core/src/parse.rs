//! HTML → AST (implementation plan S1-7, ticket P1-012).
//!
//! Raw fetched HTML goes in, the frozen [`Node`] AST comes out. html5ever
//! does the parsing (via `scraper`'s tree — the same crate that validates
//! selectors in S1-3), which is what "handle malformed HTML gracefully"
//! actually means: the WHATWG algorithm never rejects input, so this module
//! never sees a parse error, only strange trees. Its own contract is the
//! P1-012 metric that is otherwise unverifiable: **zero panics on any
//! input** — enforced by the `html_parser` fuzz target from day one.
//!
//! Shape of the work:
//!
//! 1. Find the content root: the config's `content_selector` if given,
//!    else `<main>`, `[role=main]`, `<article>`, else `<body>`. Everything
//!    outside it never enters the AST — "strip navigation" is mostly not a
//!    stripping step, it is never picking navigation up.
//! 2. Walk the subtree into [`Node`]s. Structural elements map 1:1
//!    (headings, lists, `<dl>`, tables, blockquotes, `<pre>`, images);
//!    unknown or purely presentational containers (`div`, `span`,
//!    `section`) are *unwrapped* — their children survive, the wrapper
//!    does not. Non-content subtrees (`script`, `style`, `nav`, `aside`,
//!    `footer`, forms, iframes) are dropped whole.
//! 3. Anything else — comments, processing instructions, templates — is
//!    ignored.
//!
//! What this module does NOT do, and where it went instead: sanitization is
//! S1-9 (the AST having no raw-HTML node is the first wall, not the only
//! one); heading-level normalization, whitespace cleanup, and URL rewriting
//! are S1-8; asset localization is S1-10.
//!
//! Two P1-012 criteria adjusted, stated rather than fudged:
//!
//! - "Support incremental/streaming parsing for large docs" — not
//!   implemented. SPIKE-002 measured a 500 KB page parsing in single-digit
//!   milliseconds; the P95 page-size budget makes streaming machinery pure
//!   weight. If a real corpus page ever busts the parse-time metric, this
//!   is the note to revisit.
//! - "Extract links with href resolution" — resolved links are returned
//!   *beside* the AST (for the S1-6 crawler); inside the AST, hrefs stay
//!   as written, because rewriting is a normalization decision.

use scraper::{ElementRef, Html};
use url::Url;

use crate::model::{Definition, ListItem, Node, TableCell, TableRow};

/// What one page parses into.
#[derive(Debug)]
pub struct ParsedPage {
    /// `<title>` text, else the first `<h1>` in content, else `None` —
    /// choosing a fallback title is the caller's policy, not the parser's.
    pub title: Option<String>,
    /// A [`Node::Document`] root.
    pub body: Node,
    /// Every `<a href>` in the *content*, resolved against `base`,
    /// http(s) only, fragments dropped. Crawl fodder for S1-6.
    pub links: Vec<Url>,
}

/// Parse one fetched page.
///
/// `content_selector` comes from the source config (S1-3 validated it with
/// this same selector engine); `base` is the URL the page was fetched from,
/// used only for resolving the returned `links`.
pub fn parse_page(html: &str, base: &Url, content_selector: Option<&str>) -> ParsedPage {
    let document = Html::parse_document(html);

    let title = document
        .select(&selector("title"))
        .next()
        .map(|t| collapse_ws(&t.text().collect::<String>()))
        .filter(|t| !t.is_empty());

    let root = find_content_root(&document, content_selector);

    let mut children = Vec::new();
    let mut links = Vec::new();
    if let Some(root) = root {
        for child in root.children() {
            walk(child, &mut children, &mut links);
        }
    }

    let title = title.or_else(|| first_heading_text(&children));

    ParsedPage {
        title,
        body: Node::Document { children },
        links: resolve_links(links, base),
    }
}

// ---------------------------------------------------------------------------
// Content root discovery.
// ---------------------------------------------------------------------------

fn find_content_root<'a>(
    document: &'a Html,
    content_selector: Option<&str>,
) -> Option<ElementRef<'a>> {
    if let Some(sel) = content_selector {
        // Validated at config-parse time; a selector that fails to compile
        // here means it bypassed S1-3, and falling back beats panicking.
        if let Ok(parsed) = scraper::Selector::parse(sel) {
            if let Some(found) = document.select(&parsed).next() {
                return Some(found);
            }
            // Configured selector matching nothing falls through to the
            // defaults: a config tuned for one page layout must not turn
            // every other page into an empty document.
        }
    }
    for candidate in ["main", "[role=main]", "article", "body"] {
        if let Some(found) = document.select(&selector(candidate)).next() {
            return Some(found);
        }
    }
    None
}

/// Selectors for the static strings in this module — all known-valid, and a
/// parse failure on one is a bug worth a loud panic in tests, but in
/// production the affected selector just matches nothing.
fn selector(s: &str) -> scraper::Selector {
    #[allow(clippy::expect_used)] // static, known-valid selector strings
    scraper::Selector::parse(s).expect("static selector must parse")
}

// ---------------------------------------------------------------------------
// The walk.
// ---------------------------------------------------------------------------

/// Elements whose entire subtree is not content. `header`/`footer`/`nav`
/// at any depth: on documentation pages these are chrome even when they
/// appear inside the content column.
const DROP: &[&str] = &[
    "script", "style", "noscript", "template", "nav", "aside", "header", "footer", "form",
    "iframe", "object", "embed", "select", "button", "svg", "canvas", "dialog",
];

fn walk(
    node: ego_tree::NodeRef<'_, scraper::node::Node>,
    out: &mut Vec<Node>,
    links: &mut Vec<String>,
) {
    match node.value() {
        scraper::node::Node::Text(text) => {
            let value = collapse_ws(&text.text);
            if !value.is_empty() {
                // Merge with a preceding text node: html5ever splits text
                // around entities, and downstream (anchoring!) wants prose,
                // not confetti.
                if let Some(Node::Text { value: previous }) = out.last_mut() {
                    previous.push(' ');
                    previous.push_str(&value);
                } else {
                    out.push(Node::Text { value });
                }
            }
        }
        scraper::node::Node::Element(element) => {
            let Some(element_ref) = ElementRef::wrap(node) else {
                return;
            };
            element_to_nodes(element.name(), element_ref, out, links);
        }
        // Comments, doctypes, PIs: nothing.
        _ => {}
    }
}

fn children_to_nodes(element: ElementRef<'_>, links: &mut Vec<String>) -> Vec<Node> {
    let mut out = Vec::new();
    for child in element.children() {
        walk(child, &mut out, links);
    }
    out
}

fn element_to_nodes(name: &str, el: ElementRef<'_>, out: &mut Vec<Node>, links: &mut Vec<String>) {
    match name {
        _ if DROP.contains(&name) => {}

        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            // Sphinx puts a pilcrow permalink inside every heading
            // (`a.headerlink`); it is chrome, and stripping it here keeps
            // heading text usable as titles and TOC labels.
            let children = children_to_nodes(el, links)
                .into_iter()
                .filter(|n| !is_headerlink(n))
                .collect();
            #[allow(clippy::unwrap_used)] // name is one of the six literals above
            let level = name.strip_prefix('h').unwrap().parse::<u8>().unwrap_or(6);
            out.push(Node::Heading {
                level,
                id: attr(el, "id"),
                children,
            });
        }

        "p" => out.push(Node::Paragraph {
            children: children_to_nodes(el, links),
        }),

        "pre" => {
            // `<pre><code class="language-x">` and Sphinx's
            // `<div class="highlight-x"><pre>` both funnel here; language
            // *normalization* (rs → rust) is S1-8's alias table.
            let code_el = el.select(&selector("code")).next();
            let language = code_el
                .and_then(|c| language_from_class(&class_list(c)))
                .or_else(|| language_from_class(&class_list(el)))
                // Sphinx hangs `highlight-<lang>` on a wrapper div two
                // levels up; wrappers unwrap before this pre is reached,
                // so look upward rather than hoping the class trickled in.
                .or_else(|| {
                    el.ancestors()
                        .filter_map(ElementRef::wrap)
                        .find_map(|a| language_from_class(&class_list(a)))
                });
            let code = el.text().collect::<String>();
            out.push(Node::CodeBlock {
                language,
                code: code.trim_end().to_owned(),
            });
        }

        "code" | "kbd" | "samp" => out.push(Node::InlineCode {
            code: el.text().collect(),
        }),

        "a" => {
            if let Some(href) = attr(el, "href") {
                links.push(href.clone());
                out.push(Node::Link {
                    href,
                    title: attr(el, "title"),
                    children: children_to_nodes(el, links),
                });
            } else if let Some(id) = attr(el, "id") {
                // <a name>/<a id> anchor without href: a link target.
                out.push(Node::Anchor { id });
            } else {
                out.extend(children_to_nodes(el, links));
            }
        }

        "em" | "i" | "cite" | "var" | "dfn" => out.push(Node::Emphasis {
            children: children_to_nodes(el, links),
        }),
        "strong" | "b" => out.push(Node::Strong {
            children: children_to_nodes(el, links),
        }),

        "ul" | "ol" => {
            let items = el
                .children()
                .filter_map(ElementRef::wrap)
                .filter(|c| c.value().name() == "li")
                .map(|li| ListItem {
                    children: children_to_nodes(li, links),
                })
                .collect();
            out.push(Node::List {
                ordered: name == "ol",
                start: attr(el, "start").and_then(|s| s.parse().ok()),
                items,
            });
        }

        "dl" => out.push(definition_list(el, links)),

        "table" => out.push(table(el, links)),

        "blockquote" => out.push(Node::Blockquote {
            children: children_to_nodes(el, links),
        }),

        "img" => {
            if let Some(src) = attr(el, "src") {
                out.push(Node::Image {
                    src,
                    alt: attr(el, "alt").unwrap_or_default(),
                });
            }
        }

        "hr" => out.push(Node::ThematicBreak {}),
        "br" => out.push(Node::LineBreak {}),

        "span" if attr(el, "id").is_some() && el.text().next().is_none() => {
            // Sphinx emits empty `<span id="...">` as section anchor
            // targets; deep links depend on them surviving.
            #[allow(clippy::unwrap_used)] // guarded by the match arm
            out.push(Node::Anchor {
                id: attr(el, "id").unwrap(),
            });
        }

        "div" | "section" if is_admonition(el) => {
            let (kind, title, children) = admonition(el, links);
            out.push(Node::Admonition {
                kind,
                title,
                children,
            });
        }

        // Everything else — div, span, section, figure, details, article,
        // and the long tail of unknowns — is unwrapped: children survive,
        // the wrapper does not. An element this walk has never heard of
        // loses its box, not its words. But keep its anchor if it had one:
        // Sphinx hangs permalinks off wrapper ids too.
        _ => {
            if let Some(id) = attr(el, "id") {
                out.push(Node::Anchor { id });
            }
            out.extend(children_to_nodes(el, links));
        }
    }
}

// ---------------------------------------------------------------------------
// Compound structures.
// ---------------------------------------------------------------------------

fn definition_list(el: ElementRef<'_>, links: &mut Vec<String>) -> Node {
    let mut items: Vec<Definition> = Vec::new();
    for child in el.children().filter_map(ElementRef::wrap) {
        match child.value().name() {
            "dt" => {
                // Strip the same headerlink chrome headings carry.
                let term = children_to_nodes(child, links)
                    .into_iter()
                    .filter(|n| !is_headerlink(n))
                    .collect();
                items.push(Definition {
                    id: attr(child, "id"),
                    term,
                    definition: Vec::new(),
                });
            }
            "dd" => {
                let definition = children_to_nodes(child, links);
                match items.last_mut() {
                    // A <dd> may follow several <dt>s; it belongs to the
                    // last one. A <dd> with NO preceding <dt> (malformed)
                    // becomes a term-less definition rather than vanishing.
                    Some(last) if last.definition.is_empty() => last.definition = definition,
                    _ => items.push(Definition {
                        id: None,
                        term: Vec::new(),
                        definition,
                    }),
                }
            }
            _ => {}
        }
    }
    Node::DefinitionList { items }
}

fn table(el: ElementRef<'_>, links: &mut Vec<String>) -> Node {
    let mut headers: Vec<TableCell> = Vec::new();
    let mut rows: Vec<TableRow> = Vec::new();

    for tr in el.select(&selector("tr")) {
        let cells: Vec<TableCell> = tr
            .children()
            .filter_map(ElementRef::wrap)
            .filter(|c| matches!(c.value().name(), "td" | "th"))
            .map(|cell| TableCell {
                children: children_to_nodes(cell, links),
            })
            .collect();
        let is_header_row = headers.is_empty()
            && rows.is_empty()
            && tr
                .children()
                .filter_map(ElementRef::wrap)
                .all(|c| c.value().name() != "td");
        if is_header_row && !cells.is_empty() {
            headers = cells;
        } else {
            rows.push(TableRow { cells });
        }
    }

    Node::Table { headers, rows }
}

/// Sphinx and MkDocs admonitions: `<div class="admonition warning">` with a
/// `<p class="admonition-title">` inside.
fn is_admonition(el: ElementRef<'_>) -> bool {
    class_list(el).iter().any(|c| c == "admonition")
}

const ADMONITION_KINDS: &[&str] = &[
    "note",
    "warning",
    "tip",
    "important",
    "caution",
    "danger",
    "attention",
    "hint",
    "error",
    "seealso",
];

fn admonition(el: ElementRef<'_>, links: &mut Vec<String>) -> (String, Option<String>, Vec<Node>) {
    let classes = class_list(el);
    let kind = classes
        .iter()
        .find(|c| ADMONITION_KINDS.contains(&c.as_str()))
        .cloned()
        .unwrap_or_else(|| "note".to_owned());

    let mut title = None;
    let mut children = Vec::new();
    for child in el.children() {
        if let Some(child_el) = ElementRef::wrap(child) {
            if class_list(child_el).iter().any(|c| c == "admonition-title") {
                let text = collapse_ws(&child_el.text().collect::<String>());
                if !text.is_empty() {
                    title = Some(text);
                }
                continue;
            }
        }
        walk(child, &mut children, links);
    }
    (kind, title, children)
}

// ---------------------------------------------------------------------------
// Small helpers.
// ---------------------------------------------------------------------------

fn attr(el: ElementRef<'_>, name: &str) -> Option<String> {
    el.value().attr(name).map(str::to_owned)
}

fn class_list(el: ElementRef<'_>) -> Vec<String> {
    el.value()
        .attr("class")
        .map(|c| c.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// `language-rust`, `lang-rust`, `highlight-rust` (Sphinx), or a bare
/// well-known name among the classes.
fn language_from_class(classes: &[String]) -> Option<String> {
    for class in classes {
        for prefix in ["language-", "lang-", "highlight-"] {
            if let Some(rest) = class.strip_prefix(prefix) {
                if !rest.is_empty() && rest != "default" {
                    return Some(rest.to_owned());
                }
            }
        }
    }
    None
}

/// Sphinx's `<a class="headerlink">¶</a>` permalink chrome.
fn is_headerlink(node: &Node) -> bool {
    match node {
        Node::Link { href, children, .. } => {
            href.starts_with('#')
                && matches!(children.as_slice(),
                    [Node::Text { value }] if value == "¶" || value == "§")
        }
        Node::Text { value } => value == "¶",
        _ => false,
    }
}

fn first_heading_text(children: &[Node]) -> Option<String> {
    children.iter().find_map(|n| match n {
        Node::Heading { level: 1, .. } => {
            let text = collapse_ws(&n.text_content());
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    })
}

fn resolve_links(hrefs: Vec<String>, base: &Url) -> Vec<Url> {
    let mut out = Vec::new();
    for href in hrefs {
        if let Ok(mut resolved) = base.join(&href) {
            if matches!(resolved.scheme(), "http" | "https") {
                resolved.set_fragment(None);
                if !out.contains(&resolved) {
                    out.push(resolved);
                }
            }
        }
    }
    out
}

/// Collapse runs of whitespace to single spaces and trim. The AST stores
/// prose, not indentation — EXCEPT inside `<pre>`, which never routes
/// through here.
fn collapse_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
