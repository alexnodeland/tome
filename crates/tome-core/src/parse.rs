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
    /// Every `<a href>` in the *whole document* (content and navigation
    /// alike), resolved against `base`, http(s) only, fragments dropped,
    /// deduplicated. Crawl fodder for S1-6 — navigation is where a docs site
    /// lists its pages, so link discovery is deliberately wider than the
    /// content root.
    pub links: Vec<Url>,
}

/// Parse one fetched page.
///
/// `content_selector` comes from the source config (S1-3 validated it with
/// this same selector engine); `base` is the URL the page was fetched from,
/// used only for resolving the returned `links`.
pub fn parse_page(html: &str, base: &Url, content_selector: Option<&str>) -> ParsedPage {
    parse_page_with(html, base, content_selector, None)
}

/// [`parse_page`] with a platform profile (S2-11).
///
/// The profile adds exact-class furniture rules the generic path cannot safely
/// apply — see [`crate::scrape`]. Passing `None` is the generic path exactly as
/// before.
pub fn parse_page_with(
    html: &str,
    base: &Url,
    content_selector: Option<&str>,
    profile: Option<&crate::scrape::Profile>,
) -> ParsedPage {
    let document = Html::parse_document(html);

    let title = document
        .select(&selector("title"))
        .next()
        .map(|t| collapse_ws(&t.text().collect::<String>()))
        .filter(|t| !t.is_empty());

    let root = find_content_root(&document, content_selector);

    let mut children = Vec::new();
    let mut content_links = Vec::new();
    if let Some(root) = root {
        for child in root.children() {
            walk(profile, child, &mut children, &mut content_links);
        }
    }
    // The content root is a block: whitespace at its edges is the source's
    // own indentation.
    let children = tidy_block_children(children);

    // Links for crawl discovery come from the WHOLE document, not just the
    // content root: a documentation site advertises its pages through the
    // navigation sidebar, which the content walk deliberately drops as
    // chrome. The content AST stays content-only; `links` is everything the
    // crawler could follow.
    let all_hrefs: Vec<String> = document
        .select(&selector("a[href]"))
        .filter_map(|a| a.value().attr("href").map(str::to_owned))
        .collect();

    let title = title.or_else(|| first_heading_text(&children));

    ParsedPage {
        title,
        body: Node::Document { children },
        links: resolve_links(all_hrefs, base),
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
/// Class or id fragments whose element and subtree are page furniture.
///
/// Matched as a **case-insensitive substring** of any class token or the id,
/// because every generator compounds these words differently — go.dev writes
/// `SiteBreadcrumb` and `BreadcrumbNav-li`, rustdoc writes
/// `rustdoc-breadcrumbs`, Docsy writes `td-page-meta__lastmod`. A substring
/// list is a blunt instrument and each entry is here because it was seen
/// leaking into the golden corpus, not because it might:
///
/// - `breadcrumb` — the trail above the title. go.dev, rustdoc, kubernetes.io.
/// - `hideme` — rustdoc's `<summary>` label, literally named for being hidden.
///   It is the "Expand description" toggle, an affordance the reader has no
///   equivalent of.
/// - `screen-reader`, `sr-only`, `visually-hidden`, `assistive-text` — text
///   that is invisible on the source page by construction. go.dev uses it for
///   "Press Enter to activate/deactivate dropdown".
/// - `code-toolbar` — the copy-button strip Node puts **inside** `<pre>`.
///   Belt and braces: the `pre` arm now reads the `<code>` element rather
///   than the whole `<pre>`, which is the real fix.
/// - `pre-footer`, `page-meta`, `feedback` — end-of-page furniture on Docsy
///   sites (kubernetes.io): "Was this page helpful?", "Last modified …".
///
/// The risk of substring matching is a false positive on real content. It is
/// accepted here because the corpus is the check: adding a term shows up as a
/// golden diff on 26 real pages, so an over-broad rule is visible before it
/// ships rather than after.
const DROP_CLASS_FRAGMENTS: &[&str] = &[
    "breadcrumb",
    "hideme",
    "screen-reader",
    "sr-only",
    "visually-hidden",
    "assistive-text",
    "code-toolbar",
    "pre-footer",
    "page-meta",
    "feedback",
];

/// Whether this element is page furniture rather than content.
///
/// Two signals, and the first two are the HTML's own rather than a guess:
///
/// - **`hidden`** — the author says it is not rendered. A reader showing text
///   the source page hides is showing something nobody wrote for a reader.
/// - **`aria-hidden="true"`** — decorative, and explicitly not for anyone
///   consuming the document as text, which is exactly what this AST is for.
/// - a class or id fragment from [`DROP_CLASS_FRAGMENTS`].
fn is_chrome(el: ElementRef<'_>, profile: Option<&crate::scrape::Profile>) -> bool {
    if el.value().attr("hidden").is_some() {
        return true;
    }
    if el
        .value()
        .attr("aria-hidden")
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    {
        return true;
    }

    let mut haystack = String::new();
    if let Some(class) = el.value().attr("class") {
        haystack.push_str(class);
        haystack.push(' ');
    }
    if let Some(id) = el.value().attr("id") {
        haystack.push_str(id);
    }
    if haystack.is_empty() {
        return false;
    }
    // The platform's own exact-token rules first: they are precise, so a hit
    // here is certain where a substring hit is a judgement.
    if let (Some(profile), Some(class)) = (profile, el.value().attr("class")) {
        if profile.drops(class) {
            return true;
        }
    }

    let haystack = haystack.to_ascii_lowercase();
    DROP_CLASS_FRAGMENTS
        .iter()
        .any(|fragment| haystack.contains(fragment))
}

const DROP: &[&str] = &[
    "script", "style", "noscript", "template", "nav", "aside", "header", "footer", "form",
    "iframe", "object", "embed", "select", "button", "svg", "canvas", "dialog",
];

fn walk(
    profile: Option<&crate::scrape::Profile>,
    node: ego_tree::NodeRef<'_, scraper::node::Node>,
    out: &mut Vec<Node>,
    links: &mut Vec<String>,
) {
    match node.value() {
        scraper::node::Node::Text(text) => {
            let value = collapse_inline_ws(&text.text);
            if !value.is_empty() {
                // Merge with a preceding text node: html5ever splits text
                // around entities, and downstream (anchoring!) wants prose,
                // not confetti.
                if let Some(Node::Text { value: previous }) = out.last_mut() {
                    // Concatenated, NOT joined with a space. The old version
                    // pushed one unconditionally, which turned `a&amp;b`
                    // into "a & b" — the mirror image of the bug that
                    // deleted spaces elsewhere. Each fragment now carries
                    // its own boundary whitespace.
                    previous.push_str(&value);
                    // Two fragments that each ended and began with a space
                    // would otherwise leave a double.
                    if previous.ends_with("  ") {
                        previous.truncate(previous.len() - 1);
                    }
                } else {
                    out.push(Node::Text { value });
                }
            }
        }
        scraper::node::Node::Element(element) => {
            let Some(element_ref) = ElementRef::wrap(node) else {
                return;
            };
            element_to_nodes(element.name(), element_ref, out, links, profile);
        }
        // Comments, doctypes, PIs: nothing.
        _ => {}
    }
}

/// Children of an **inline** element: boundary whitespace is part of the
/// surrounding flow and is kept.
/// All text in a subtree, skipping the parts that are not content.
///
/// `ElementRef::text()` walks everything, including the `<button>`s and
/// toolbars that documentation generators put inside otherwise-plain
/// elements. Used for `<pre>` blocks that have no `<code>` inside them.
fn content_text(element: ElementRef<'_>, profile: Option<&crate::scrape::Profile>) -> String {
    let mut out = String::new();
    let mut skip_until = None;
    for node in element.descendants() {
        // Skipping a subtree means ignoring every node until traversal
        // returns above the element that started the skip.
        if let Some(id) = skip_until {
            if node.ancestors().any(|a| a.id() == id) {
                continue;
            }
            skip_until = None;
        }
        match node.value() {
            scraper::node::Node::Element(el) => {
                let dropped = DROP.contains(&el.name())
                    || ElementRef::wrap(node).is_some_and(|el| is_chrome(el, profile));
                if dropped {
                    skip_until = Some(node.id());
                }
            }
            scraper::node::Node::Text(text) => out.push_str(&text.text),
            _ => {}
        }
    }
    out
}

fn children_to_nodes(
    element: ElementRef<'_>,
    links: &mut Vec<String>,
    profile: Option<&crate::scrape::Profile>,
) -> Vec<Node> {
    let mut out = Vec::new();
    for child in element.children() {
        walk(profile, child, &mut out, links);
    }
    out
}

/// Children of a **block** element: the whitespace at each end is
/// indentation in the source, not content.
fn block_children(
    element: ElementRef<'_>,
    links: &mut Vec<String>,
    profile: Option<&crate::scrape::Profile>,
) -> Vec<Node> {
    tidy_block_children(children_to_nodes(element, links, profile))
}

fn element_to_nodes(
    name: &str,
    el: ElementRef<'_>,
    out: &mut Vec<Node>,
    links: &mut Vec<String>,
    profile: Option<&crate::scrape::Profile>,
) {
    match name {
        _ if DROP.contains(&name) => {}
        _ if is_chrome(el, profile) => {}

        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            // Sphinx puts a pilcrow permalink inside every heading
            // (`a.headerlink`); it is chrome, and stripping it here keeps
            // heading text usable as titles and TOC labels.
            let children = unwrap_self_permalink(tidy_block_children(
                children_to_nodes(el, links, profile)
                    .into_iter()
                    .filter(|n| !is_headerlink(n))
                    .collect(),
            ));
            #[allow(clippy::unwrap_used)] // name is one of the six literals above
            let level = name.strip_prefix('h').unwrap().parse::<u8>().unwrap_or(6);
            out.push(Node::Heading {
                level,
                id: attr(el, "id"),
                children,
            });
        }

        "p" => out.push(Node::Paragraph {
            children: block_children(el, links, profile),
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
            // **The `<code>` element's text, not the `<pre>`'s.**
            //
            // Node's API docs put a copy-button strip *inside* the `<pre>`:
            // `<pre><code>…</code><div class="code-toolbar"><span>js</span>
            // <button>copy</button></div></pre>`. Collecting the whole
            // subtree appended "jscopy" to the end of 37 code blocks in the
            // corpus — garbage that a reader would copy along with the code.
            //
            // Falling back to the `<pre>` when there is no `<code>` is not a
            // compromise: Sphinx emits `<div class="highlight"><pre><span>…`
            // with no `<code>` at all, and there the spans *are* the code.
            // That path skips dropped subtrees so a stray button cannot leak
            // in the same way.
            let code = match code_el {
                Some(code_el) => code_el.text().collect::<String>(),
                None => content_text(el, profile),
            };
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
                // A permalink is chrome wherever it appears, not only inside
                // a heading. rustdoc hangs a `§` off every method signature
                // and mdBook a `↩` off every footnote; both used to render as
                // stray glyphs in the middle of the prose.
                //
                // Its `id`, if it has one, is a deep-link target and outlives
                // the link — the same care the wrapper-unwrapping arm takes.
                if is_permalink(&href, el) {
                    if let Some(id) = attr(el, "id") {
                        out.push(Node::Anchor { id });
                    }
                    return;
                }
                links.push(href.clone());
                out.push(Node::Link {
                    href,
                    title: attr(el, "title"),
                    children: children_to_nodes(el, links, profile),
                });
            } else if let Some(id) = attr(el, "id") {
                // <a name>/<a id> anchor without href: a link target.
                out.push(Node::Anchor { id });
            } else {
                out.extend(children_to_nodes(el, links, profile));
            }
        }

        "em" | "i" | "cite" | "var" | "dfn" => out.push(Node::Emphasis {
            children: children_to_nodes(el, links, profile),
        }),
        "strong" | "b" => out.push(Node::Strong {
            children: children_to_nodes(el, links, profile),
        }),

        "ul" | "ol" => {
            let items = el
                .children()
                .filter_map(ElementRef::wrap)
                .filter(|c| c.value().name() == "li")
                .map(|li| ListItem {
                    children: block_children(li, links, profile),
                })
                .collect();
            out.push(Node::List {
                ordered: name == "ol",
                start: attr(el, "start").and_then(|s| s.parse().ok()),
                items,
            });
        }

        "dl" => out.push(definition_list(el, links, profile)),

        "table" => out.push(table(el, links, profile)),

        "blockquote" => out.push(Node::Blockquote {
            children: block_children(el, links, profile),
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
            let (kind, title, children) = admonition(el, links, profile);
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
            out.extend(children_to_nodes(el, links, profile));
        }
    }
}

// ---------------------------------------------------------------------------
// Compound structures.
// ---------------------------------------------------------------------------

fn definition_list(
    el: ElementRef<'_>,
    links: &mut Vec<String>,
    profile: Option<&crate::scrape::Profile>,
) -> Node {
    let mut items: Vec<Definition> = Vec::new();
    for child in el.children().filter_map(ElementRef::wrap) {
        match child.value().name() {
            "dt" => {
                // Strip the same headerlink chrome headings carry.
                let term = unwrap_self_permalink(tidy_block_children(
                    children_to_nodes(child, links, profile)
                        .into_iter()
                        .filter(|n| !is_headerlink(n))
                        .collect(),
                ));
                items.push(Definition {
                    id: attr(child, "id"),
                    term,
                    definition: Vec::new(),
                });
            }
            "dd" => {
                let definition = block_children(child, links, profile);
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

fn table(
    el: ElementRef<'_>,
    links: &mut Vec<String>,
    profile: Option<&crate::scrape::Profile>,
) -> Node {
    let mut headers: Vec<TableCell> = Vec::new();
    let mut rows: Vec<TableRow> = Vec::new();

    for tr in el.select(&selector("tr")) {
        let cells: Vec<TableCell> = tr
            .children()
            .filter_map(ElementRef::wrap)
            .filter(|c| matches!(c.value().name(), "td" | "th"))
            .map(|cell| TableCell {
                children: block_children(cell, links, profile),
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

fn admonition(
    el: ElementRef<'_>,
    links: &mut Vec<String>,
    profile: Option<&crate::scrape::Profile>,
) -> (String, Option<String>, Vec<Node>) {
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
        walk(profile, child, &mut children, links);
    }
    (kind, title, tidy_block_children(children))
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
/// Markers a documentation generator uses for "permalink to this heading".
///
/// Sphinx uses `¶`, Node's API docs use `#`, and several generators use the
/// link glyph. They are all chrome: stripping them keeps heading text usable
/// as a title and a TOC label. The corpus is what turned this from a
/// Sphinx-only list into a list — every Node page's title came out as `OS#`,
/// `Path#`, `Query string#` until `#` was on it.
/// Markers that mean "permalink to this thing", not content.
///
/// `↩` is here for a different reason from the rest: it is the *back*-link at
/// the end of a footnote (mdBook writes one on every footnote in the Cargo
/// manifest page). Same shape — a fragment href whose whole text is one
/// glyph — and the same answer.
const PERMALINK_MARKERS: &[&str] = &["¶", "§", "#", "🔗", "⚓", "↩", "↵", "&para;"];

/// Whether an `<a>` is a permalink affordance: a same-page fragment whose
/// entire visible text is one marker glyph.
///
/// Both halves matter. A heading may legitimately contain a link, and a
/// heading may legitimately contain a `#` — `is_permalink` says yes only when
/// the link points into this page *and* has nothing to say but the glyph.
fn is_permalink(href: &str, el: ElementRef<'_>) -> bool {
    href.starts_with('#') && {
        let text = el.text().collect::<String>();
        PERMALINK_MARKERS.contains(&text.trim())
    }
}

fn is_headerlink(node: &Node) -> bool {
    match node {
        Node::Link { href, children, .. } => {
            // A fragment href AND nothing but a marker for text. Both halves
            // matter: a heading may legitimately contain a link, and a
            // heading may legitimately contain a `#` (`# in a URL`).
            href.starts_with('#')
                && matches!(children.as_slice(),
                    [Node::Text { value }] if PERMALINK_MARKERS.contains(&value.trim()))
        }
        Node::Text { value } => value == "¶",
        _ => false,
    }
}

/// Unwrap a heading whose entire content is a link to itself.
///
/// mdBook renders every heading as
/// `<h1 id="x"><a class="header" href="#x">Title</a></h1>`, so the whole
/// heading is one permalink. Left alone it renders as a giant underlined
/// link — every heading on doc.rust-lang.org/cargo looked like that until
/// this existed.
///
/// This is the same family as the Sphinx pilcrow that [`is_headerlink`]
/// already strips; the difference is only that Sphinx puts the permalink
/// *beside* the text and mdBook wraps the text *in* it. The test is
/// deliberately narrow — one child, a fragment href — so that a heading
/// which genuinely links somewhere ("See <a href='other.html'>the guide</a>")
/// keeps its link.
fn unwrap_self_permalink(children: Vec<Node>) -> Vec<Node> {
    match children.as_slice() {
        [Node::Link {
            href,
            children: inner,
            ..
        }] if href.starts_with('#') => inner.clone(),
        _ => children,
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
///
/// For *titles and labels*. Inline prose must use [`collapse_inline_ws`],
/// which keeps the boundary spaces this one throws away.
fn collapse_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Collapse runs of whitespace to single spaces **without trimming the
/// ends**.
///
/// This exists because [`collapse_ws`] silently deleted every space that sat
/// next to an inline element. HTML collapses a *run* of whitespace to one
/// space; it never removes the space between two inline siblings. So
/// `the interactive <a>REPL</a>.` came out as "the interactiveREPL." — on
/// every page, in every source, wherever prose met a link, an `<em>`, or a
/// piece of inline code. It is the kind of defect that is invisible in a
/// unit test written from the same misunderstanding and obvious the moment a
/// real page is on screen.
///
/// A whitespace-only node collapses to a single space rather than to
/// nothing, because that is exactly the separator in `<a>x</a> <em>y</em>`.
/// Block edges are trimmed separately, by [`trim_block_edges`].
fn collapse_inline_ws(text: &str) -> String {
    let collapsed = collapse_ws(text);
    if collapsed.is_empty() {
        return if text.is_empty() {
            String::new()
        } else {
            " ".to_owned()
        };
    }
    let leading = text.starts_with(char::is_whitespace);
    let trailing = text.ends_with(char::is_whitespace);
    let mut out = String::with_capacity(collapsed.len() + 2);
    if leading {
        out.push(' ');
    }
    out.push_str(&collapsed);
    if trailing {
        out.push(' ');
    }
    out
}

/// Tidy the whitespace in a **block's** children.
///
/// Two things, and both are about telling content apart from source layout:
///
/// 1. **The two ends are trimmed.** `<p>  hello  </p>` is "hello". Applied
///    only to blocks — doing it inside an inline element would delete a
///    space belonging to the surrounding flow, turning `a<em> b</em>c` into
///    "abc".
/// 2. **A whitespace-only node survives only between two inline siblings
///    that carry text.** The newline-and-indent between `</h1>` and `<dl>`
///    is layout and goes; the single space in `<a>x</a> <em>y</em>` is the
///    only thing separating two words and stays.
///
/// The golden corpus caught the second half: the first version of this kept
/// every interior space, and the normalized output grew a stray `" "` node
/// between every pair of block elements.
fn tidy_block_children(children: Vec<Node>) -> Vec<Node> {
    let mut children = merge_adjacent_text(children);
    if let Some(Node::Text { value }) = children.first_mut() {
        let trimmed = value.trim_start().to_owned();
        *value = trimmed;
    }
    if let Some(Node::Text { value }) = children.last_mut() {
        let trimmed = value.trim_end().to_owned();
        *value = trimmed;
    }

    let keep: Vec<bool> = children
        .iter()
        .enumerate()
        .map(|(index, node)| match node {
            Node::Text { value } if value.is_empty() => false,
            Node::Text { value } if value.trim().is_empty() => {
                let before = index.checked_sub(1).and_then(|i| children.get(i));
                let after = children.get(index + 1);
                before.is_some_and(separates_words) && after.is_some_and(separates_words)
            }
            _ => true,
        })
        .collect();

    let mut keep = keep.into_iter();
    children.retain(|_| keep.next().unwrap_or(true));
    children
}

/// Fold runs of adjacent text nodes into one.
///
/// `walk` already merges text as it goes, but a later filter can leave two
/// text nodes touching — removing a Sphinx pilcrow permalink from the middle
/// of a `<dt>` does exactly that. The trailing-edge trim then lands on the
/// wrong node, which is why a signature came out as `resize(size) ` with a
/// space before the permalink that was no longer there.
fn merge_adjacent_text(children: Vec<Node>) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::with_capacity(children.len());
    for node in children {
        match (out.last_mut(), &node) {
            (Some(Node::Text { value: previous }), Node::Text { value }) => {
                previous.push_str(value);
                while previous.contains("  ") {
                    *previous = previous.replace("  ", " ");
                }
            }
            _ => out.push(node),
        }
    }
    out
}

/// Whether a space next to this node is holding two words apart.
///
/// Inline nodes that render text. `Anchor` is inline but empty, so a space
/// beside one is layout, not a separator; every block is layout too.
fn separates_words(node: &Node) -> bool {
    matches!(
        node,
        Node::Text { .. }
            | Node::Emphasis { .. }
            | Node::Strong { .. }
            | Node::InlineCode { .. }
            | Node::Link { .. }
            | Node::Image { .. }
    )
}
