//! AST → HTML (implementation plan S1-13, ticket P1-016).
//!
//! The last stage of the pipeline and the first one a person sees. It takes a
//! normalized, sanitized, asset-localized [`Node`] tree and produces the HTML
//! the reader iframe displays, plus the heading outline the TOC sidebar
//! (S1-14) draws.
//!
//! # This module owes S1-9 a contract, and it is the whole point
//!
//! The sanitizer restricts the AST's *token* fields — URL schemes, `id`s,
//! class names — to safe character sets, and deliberately does **not**
//! charset-strip the free-text ones: `Link.title`, `Image.alt`,
//! `Admonition.title`, and every `Text` / `InlineCode` / `CodeBlock` body. A
//! documentation title may legitimately contain a quote or an angle bracket,
//! and stripping it would destroy content to solve a problem escaping already
//! solves. From `sanitize.rs`:
//!
//! > **S1-13's reader must quote every attribute and HTML-escape every
//! > attribute value and every text node.**
//!
//! So: **no `push_str` of page-derived text anywhere in this file.** Text goes
//! through [`html::push_escaped`], attributes through [`html::attr`], which
//! quotes as well as escapes. `no_field_escapes_its_context` and the
//! `render` fuzz target are what keep that true as this module grows.
//!
//! # The offline guarantee is re-checked here
//!
//! Asset localization (S1-10) rewrites every `Image.src` to a relative
//! `assets/<sha256>.<ext>`, and the Stage 1 exit gate asserts no remote
//! reference survives it. This module asserts it *again* at the point of
//! emission ([`local_asset_ref`]) rather than trusting the earlier stage,
//! because "no page ever reaches the network" is the guarantee the whole
//! product rests on, and it costs a substring check to hold it twice.
//!
//! # Structure the stylesheet depends on
//!
//! `public/reader.css` (S1-12) styles what is emitted here. The couplings
//! that are not obvious from either file alone:
//!
//! - the page root is `<div class="tome-page">`, which carries the 70ch
//!   measure;
//! - tables are wrapped in `<div class="tome-table-scroll">` so a wide table
//!   scrolls inside itself instead of widening the document;
//! - code blocks are `<pre><code class="language-x">` with one
//!   `<span class="line">` per line, emitted by [`crate::highlight`];
//! - admonitions are `tome-admonition tome-admonition--<kind>`.

use std::collections::HashSet;

use crate::highlight::Highlighter;
use crate::html::{attr, push_escaped};
use crate::model::{Definition, ListItem, Node, TableCell, TableRow};

/// Where a page's localized assets are served from, and what to highlight
/// with.
pub struct RenderOptions<'a> {
    /// Prefix prepended to each `assets/<sha256>.<ext>` reference — the
    /// source's asset directory as the webview can reach it. The app passes
    /// its custom-protocol URL (`tome://localhost/<source-id>/`); tests pass
    /// whatever they like, including `""`.
    pub asset_base: &'a str,
    pub highlighter: &'a Highlighter,
}

/// One heading in a page's outline.
///
/// **Distinct from [`crate::model::TocEntry`], which is the *source's*
/// navigation tree** — the list of pages in a documentation set. This is the
/// outline *within* one page. The two were nearly given the same name, and a
/// reader with a "TOC" that sometimes means pages and sometimes means
/// headings is a reader nobody can reason about.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutlineEntry {
    /// The `id` to link to. Always present — [`render`] derives one for a
    /// heading that has none, so no heading is unreachable from the sidebar.
    pub id: String,
    pub title: String,
    pub level: u8,
    pub children: Vec<OutlineEntry>,
}

/// A rendered page: the HTML for the frame, and the outline for the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RenderedPage {
    pub html: String,
    /// The `h1`, if the page has one. The reader's title bar uses it.
    pub title: Option<String>,
    pub outline: Vec<OutlineEntry>,
}

/// Deepest heading level that appears in the outline. P1-019's criterion;
/// an `h5` in a Sphinx API page is a parameter name, not a section.
const OUTLINE_MAX_LEVEL: u8 = 4;

/// Render a document. `document` should be a [`Node::Document`]; anything
/// else renders as a single-node page rather than erroring, because a
/// renderer that can fail is a page that can fail to open.
pub fn render(document: &Node, options: &RenderOptions<'_>) -> RenderedPage {
    let mut ctx = Context {
        options,
        headings: Vec::new(),
        used_ids: HashSet::new(),
        title: None,
    };

    let mut html = String::with_capacity(4096);
    html.push_str("<div class=\"tome-page\">");
    match document {
        Node::Document { children } => ctx.children(&mut html, children),
        other => ctx.node(&mut html, other),
    }
    html.push_str("</div>");

    RenderedPage {
        html,
        title: ctx.title,
        outline: nest(ctx.headings),
    }
}

/// Flat heading record, before nesting.
struct FlatHeading {
    id: String,
    title: String,
    level: u8,
}

struct Context<'a> {
    options: &'a RenderOptions<'a>,
    headings: Vec<FlatHeading>,
    /// Every id emitted so far, so a derived one can be made unique. Also
    /// includes ids the page brought with it, so a derived id never collides
    /// with a real anchor and steals its `#fragment`.
    used_ids: HashSet<String>,
    title: Option<String>,
}

impl Context<'_> {
    fn children(&mut self, out: &mut String, children: &[Node]) {
        for child in children {
            self.node(out, child);
        }
    }

    fn node(&mut self, out: &mut String, node: &Node) {
        match node {
            Node::Document { children } => self.children(out, children),

            Node::Heading {
                level,
                id,
                children,
            } => {
                let level = (*level).clamp(1, 6);
                let text = Node::Heading {
                    level,
                    id: None,
                    children: children.clone(),
                }
                .text_content();

                let anchor = self.heading_id(id.as_deref(), &text);
                if level == 1 && self.title.is_none() {
                    self.title = Some(text.trim().to_owned());
                }
                if level <= OUTLINE_MAX_LEVEL {
                    self.headings.push(FlatHeading {
                        id: anchor.clone(),
                        title: text.trim().to_owned(),
                        level,
                    });
                }

                out.push_str("<h");
                out.push_str(&level.to_string());
                attr(out, "id", &anchor);
                out.push('>');
                self.children(out, children);
                out.push_str("</h");
                out.push_str(&level.to_string());
                out.push('>');
            }

            Node::Paragraph { children } => {
                out.push_str("<p>");
                self.children(out, children);
                out.push_str("</p>");
            }

            Node::CodeBlock { language, code } => {
                out.push_str("<pre><code");
                if let Some(language) = language {
                    // The sanitizer restricts `language` to `[a-z0-9-]`, so
                    // this is already a safe class token; `attr` escapes it
                    // anyway, because a renderer that relies on an upstream
                    // guarantee is a renderer that breaks when the upstream
                    // changes.
                    attr(out, "class", &format!("language-{language}"));
                }
                out.push('>');
                out.push_str(
                    &self
                        .options
                        .highlighter
                        .highlight(code, language.as_deref()),
                );
                out.push_str("</code></pre>");
            }

            Node::Blockquote { children } => {
                out.push_str("<blockquote>");
                self.children(out, children);
                out.push_str("</blockquote>");
            }

            Node::List {
                ordered,
                start,
                items,
            } => self.list(out, *ordered, *start, items),

            Node::DefinitionList { items } => self.definition_list(out, items),

            Node::Table { headers, rows } => self.table(out, headers, rows),

            Node::Admonition {
                kind,
                title,
                children,
            } => {
                out.push_str("<div");
                // An unknown kind keeps its own modifier class rather than
                // being dropped or coerced: the AST's rule is that unknown
                // admonitions render as notes, and the stylesheet's base
                // `.tome-admonition` is that note. A `.. seealso::` block
                // still looks like a block.
                attr(
                    out,
                    "class",
                    &format!("tome-admonition tome-admonition--{kind}"),
                );
                out.push('>');
                out.push_str("<p class=\"tome-admonition-title\">");
                // Free text: escaped, per the S1-9 contract.
                push_escaped(out, title.as_deref().unwrap_or(kind));
                out.push_str("</p>");
                self.children(out, children);
                out.push_str("</div>");
            }

            Node::Image { src, alt } => match local_asset_ref(src) {
                Some(local) => {
                    out.push_str("<img");
                    attr(out, "src", &format!("{}{}", self.options.asset_base, local));
                    attr(out, "alt", alt);
                    // `loading="lazy"` on a local file is not about bandwidth
                    // — it keeps a page with two hundred images from decoding
                    // all of them before first paint.
                    out.push_str(" loading=\"lazy\">");
                }
                None => {
                    // Not a localized asset. S1-10 should have made this
                    // unreachable; if it is reached, the reference is remote
                    // or malformed, and rendering the alt text is the only
                    // answer that keeps the offline guarantee true.
                    out.push_str("<em>");
                    push_escaped(
                        out,
                        &if alt.trim().is_empty() {
                            "image unavailable offline".to_owned()
                        } else {
                            format!("image unavailable offline: {}", alt.trim())
                        },
                    );
                    out.push_str("</em>");
                }
            },

            Node::ThematicBreak {} => out.push_str("<hr>"),
            Node::LineBreak {} => out.push_str("<br>"),

            Node::Text { value } => push_escaped(out, value),

            Node::Emphasis { children } => {
                out.push_str("<em>");
                self.children(out, children);
                out.push_str("</em>");
            }
            Node::Strong { children } => {
                out.push_str("<strong>");
                self.children(out, children);
                out.push_str("</strong>");
            }
            Node::InlineCode { code } => {
                out.push_str("<code>");
                push_escaped(out, code);
                out.push_str("</code>");
            }

            Node::Link {
                href,
                title,
                children,
            } => {
                out.push_str("<a");
                attr(out, "href", href);
                if let Some(title) = title {
                    attr(out, "title", title);
                }
                // Every link is intercepted by the frame's bootstrap and
                // routed through the app (S1-15), so `href` is a label, not a
                // navigation. `rel` and `target` are belt and braces for the
                // case where interception is somehow bypassed: without them a
                // click could navigate the frame to a page Tome does not
                // control, inside the app's own window.
                if is_external(href) {
                    out.push_str(" rel=\"noopener noreferrer\" target=\"_blank\"");
                }
                out.push('>');
                self.children(out, children);
                out.push_str("</a>");
            }

            Node::Anchor { id } => {
                let id = self.unique_id(id);
                out.push_str("<span");
                attr(out, "id", &id);
                out.push_str("></span>");
            } // No wildcard arm, deliberately. `Node` is `#[non_exhaustive]`
              // for downstream crates, but this match is inside the defining
              // crate, so it is exhaustive — and a new variant should stop the
              // build here rather than render as silent nothing. A page that
              // quietly loses a block is the failure mode the golden corpus was
              // built to catch; not compiling is cheaper.
        }
    }

    fn list(&mut self, out: &mut String, ordered: bool, start: Option<u32>, items: &[ListItem]) {
        if ordered {
            out.push_str("<ol");
            if let Some(start) = start {
                attr(out, "start", &start.to_string());
            }
            out.push('>');
        } else {
            out.push_str("<ul>");
        }
        for item in items {
            out.push_str("<li>");
            self.children(out, &item.children);
            out.push_str("</li>");
        }
        out.push_str(if ordered { "</ol>" } else { "</ul>" });
    }

    fn definition_list(&mut self, out: &mut String, items: &[Definition]) {
        out.push_str("<dl>");
        for item in items {
            out.push_str("<dt");
            if let Some(id) = &item.id {
                // The permalink target for a Sphinx API entry. Deep links
                // into docs.python.org land here, so the id must survive.
                let id = self.unique_id(id);
                attr(out, "id", &id);
            }
            out.push('>');
            self.children(out, &item.term);
            out.push_str("</dt><dd>");
            self.children(out, &item.definition);
            out.push_str("</dd>");
        }
        out.push_str("</dl>");
    }

    fn table(&mut self, out: &mut String, headers: &[TableCell], rows: &[TableRow]) {
        out.push_str("<div class=\"tome-table-scroll\"><table>");
        if !headers.is_empty() {
            out.push_str("<thead><tr>");
            for cell in headers {
                out.push_str("<th>");
                self.children(out, &cell.children);
                out.push_str("</th>");
            }
            out.push_str("</tr></thead>");
        }
        if !rows.is_empty() {
            out.push_str("<tbody>");
            for row in rows {
                out.push_str("<tr>");
                for cell in &row.cells {
                    out.push_str("<td>");
                    self.children(out, &cell.children);
                    out.push_str("</td>");
                }
                out.push_str("</tr>");
            }
            out.push_str("</tbody>");
        }
        out.push_str("</table></div>");
    }

    /// The id a heading will carry.
    ///
    /// A heading that brought its own keeps it — every existing `#fragment`
    /// link in the world depends on that, which is why the sanitizer goes to
    /// such lengths not to drop ids. A heading *without* one gets a slug of
    /// its text, because the TOC sidebar links by id and a heading with no id
    /// would be in the sidebar and unreachable from it.
    fn heading_id(&mut self, existing: Option<&str>, text: &str) -> String {
        match existing.filter(|id| !id.is_empty()) {
            Some(id) => self.unique_id(id),
            None => {
                let slug = slugify(text);
                self.unique_id(if slug.is_empty() { "section" } else { &slug })
            }
        }
    }

    /// `candidate`, or `candidate-2`, `candidate-3`, … if it is taken.
    ///
    /// Duplicate ids are legal HTML and `getElementById` returns the first,
    /// so two identically-named headings would make the second unreachable
    /// from the sidebar — a real case on API pages, where every class has an
    /// `__init__`.
    fn unique_id(&mut self, candidate: &str) -> String {
        if self.used_ids.insert(candidate.to_owned()) {
            return candidate.to_owned();
        }
        for n in 2u32.. {
            let attempt = format!("{candidate}-{n}");
            if self.used_ids.insert(attempt.clone()) {
                return attempt;
            }
        }
        // `2u32..` is unbounded; this is unreachable. Returning the candidate
        // is the harmless answer if it ever were not.
        candidate.to_owned()
    }
}

/// A URL-safe slug for a heading with no id of its own.
///
/// Non-ASCII survives, lowercased: the sanitizer went out of its way to keep
/// unicode ids working (`café`, CJK slugs) because stripping them killed the
/// TOC on non-English sites, and a *derived* id has no reason to be stricter
/// than a preserved one. Only whitespace, quotes, and control characters —
/// the things that break an attribute — are removed.
fn slugify(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    let mut pending_dash = false;
    for c in text.trim().chars() {
        if c.is_whitespace() || c == '"' || c == '\'' || c == '`' || c.is_control() {
            pending_dash = !slug.is_empty();
        } else {
            if pending_dash {
                slug.push('-');
                pending_dash = false;
            }
            slug.extend(c.to_lowercase());
        }
    }
    slug
}

/// The localized asset reference in `src`, or `None` if it is anything else.
///
/// Deliberately a strict allowlist rather than a denylist of schemes. S1-10
/// produces exactly one shape — `assets/<hex>.<ext>` — and anything that is
/// not that shape is either a remote reference the earlier stage failed to
/// localize, or a traversal attempt. Both render as alt text.
fn local_asset_ref(src: &str) -> Option<&str> {
    let rest = src.strip_prefix("assets/")?;
    let valid = !rest.is_empty()
        && !rest.contains('/')
        && !rest.contains('\\')
        && !rest.contains("..")
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_');
    valid.then_some(src)
}

/// Whether a link leaves the library. Used only to decide `rel`/`target`;
/// the actual routing decision is the app's (S1-15), which sees the same
/// href.
fn is_external(href: &str) -> bool {
    let lower = href.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

/// Turn the flat heading list into the nested tree the sidebar draws.
///
/// Documentation heading levels skip (an `h1` followed by an `h3` is
/// ordinary), so this nests by *relative* depth rather than by level number:
/// a heading goes under the nearest preceding heading of a smaller level,
/// whatever the gap.
fn nest(flat: Vec<FlatHeading>) -> Vec<OutlineEntry> {
    let mut roots: Vec<OutlineEntry> = Vec::new();
    // Indices, as a path from a root down to the last inserted entry.
    let mut path: Vec<usize> = Vec::new();
    let mut levels: Vec<u8> = Vec::new();

    for heading in flat {
        let entry = OutlineEntry {
            id: heading.id,
            title: heading.title,
            level: heading.level,
            children: Vec::new(),
        };

        while levels.last().is_some_and(|last| *last >= heading.level) {
            levels.pop();
            path.pop();
        }

        let mut siblings = &mut roots;
        for index in &path {
            siblings = &mut siblings[*index].children;
        }
        siblings.push(entry);
        path.push(siblings.len() - 1);
        levels.push(heading.level);
    }

    roots
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn opts<'a>(highlighter: &'a Highlighter) -> RenderOptions<'a> {
        RenderOptions {
            asset_base: "tome://localhost/demo/",
            highlighter,
        }
    }

    fn render_html(node: &Node) -> String {
        let h = Highlighter::shared();
        render(node, &opts(h)).html
    }

    fn text(value: &str) -> Node {
        Node::Text {
            value: value.to_owned(),
        }
    }

    fn doc(children: Vec<Node>) -> Node {
        Node::Document { children }
    }

    #[test]
    fn renders_the_page_wrapper_the_stylesheet_expects() {
        let html = render_html(&doc(vec![]));
        assert_eq!(html, "<div class=\"tome-page\"></div>");
    }

    #[test]
    fn no_field_escapes_its_context() {
        // The S1-9 contract, exercised on every free-text field the sanitizer
        // deliberately leaves unstripped. Each payload closes the attribute
        // or element it lands in; none may survive as markup.
        let payload = r#"" onload="alert(1)"><script>alert(2)</script>"#;
        let document = doc(vec![
            Node::Heading {
                level: 1,
                id: Some(payload.to_owned()),
                children: vec![text(payload)],
            },
            Node::Paragraph {
                children: vec![
                    text(payload),
                    Node::InlineCode {
                        code: payload.to_owned(),
                    },
                    Node::Link {
                        href: "/page.html".to_owned(),
                        title: Some(payload.to_owned()),
                        children: vec![text(payload)],
                    },
                ],
            },
            Node::Image {
                src: "assets/abc123.png".to_owned(),
                alt: payload.to_owned(),
            },
            Node::Admonition {
                kind: "note".to_owned(),
                title: Some(payload.to_owned()),
                children: vec![text(payload)],
            },
            Node::CodeBlock {
                language: Some("rust".to_owned()),
                code: payload.to_owned(),
            },
        ]);

        let html = render_html(&document);

        // The assertion that actually expresses the contract: every tag in
        // the output is one this module wrote. A `<` that leaked from page
        // content would show up here as an unexpected tag name.
        //
        // (The first draft of this test asserted `!html.contains("onload=")`,
        // which fails on correct output -- the payload's text appears, safely
        // escaped, inside a quoted attribute value. "The string is absent" is
        // the wrong shape of assertion for escaping; "no tag came from the
        // input" is the right one.)
        for name in tag_names(&html) {
            assert!(
                matches!(
                    name.as_str(),
                    "div" | "h1" | "p" | "code" | "a" | "img" | "pre" | "span" | "em"
                ),
                "tag {name:?} escaped from page content into {html}"
            );
        }

        // Neutralised, not lost: a page documenting XSS must still be able to
        // show its own examples.
        assert!(html.contains("&quot;"), "{html}");
        assert!(html.contains("&lt;script&gt;"), "{html}");
    }

    /// Every tag name in `html`, opening and closing.
    fn tag_names(html: &str) -> Vec<String> {
        let mut names = Vec::new();
        let mut rest = html;
        while let Some(at) = rest.find('<') {
            rest = &rest[at + 1..];
            let rest_trimmed = rest.strip_prefix('/').unwrap_or(rest);
            let end = rest_trimmed
                .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .unwrap_or(rest_trimmed.len());
            names.push(rest_trimmed[..end].to_ascii_lowercase());
        }
        names
    }

    #[test]
    fn attribute_values_are_always_quoted() {
        // An unquoted attribute is escapable by whitespace alone, which no
        // character escaping prevents. Every `=` this module emits inside a
        // tag must be followed by a quote.
        let html = render_html(&doc(vec![
            Node::Heading {
                level: 2,
                id: Some("a b".to_owned()),
                children: vec![text("x")],
            },
            Node::Image {
                src: "assets/x.png".to_owned(),
                alt: "y".to_owned(),
            },
            Node::List {
                ordered: true,
                start: Some(4),
                items: vec![ListItem {
                    children: vec![text("i")],
                }],
            },
        ]));
        for (at, _) in html.match_indices('=') {
            assert_eq!(
                html[at + 1..].chars().next(),
                Some('"'),
                "unquoted attribute at {at} in {html}"
            );
        }
    }

    #[test]
    fn headings_keep_their_own_ids() {
        let html = render_html(&doc(vec![Node::Heading {
            level: 2,
            id: Some("impl-From<T>-for-T".to_owned()),
            children: vec![text("From")],
        }]));
        // rustdoc's angle brackets are escaped in the attribute but the id is
        // otherwise intact -- the browser reads the entity back as `<`, so
        // `#impl-From<T>-for-T` still resolves.
        assert!(html.contains(r#"id="impl-From&lt;T&gt;-for-T""#), "{html}");
    }

    #[test]
    fn headings_without_ids_get_one_derived() {
        // Otherwise the heading appears in the sidebar and clicking it does
        // nothing.
        let page = render(
            &doc(vec![Node::Heading {
                level: 2,
                id: None,
                children: vec![text("Getting Started")],
            }]),
            &opts(Highlighter::shared()),
        );
        assert_eq!(page.outline[0].id, "getting-started");
        assert!(page.html.contains(r#"id="getting-started""#));
    }

    #[test]
    fn derived_ids_keep_unicode() {
        // The sanitizer went to real lengths to keep unicode ids working; a
        // derived id has no reason to be stricter than a preserved one.
        let page = render(
            &doc(vec![
                Node::Heading {
                    level: 2,
                    id: None,
                    children: vec![text("Café Configuration")],
                },
                Node::Heading {
                    level: 2,
                    id: None,
                    children: vec![text("日本語の設定")],
                },
            ]),
            &opts(Highlighter::shared()),
        );
        assert_eq!(page.outline[0].id, "café-configuration");
        assert_eq!(page.outline[1].id, "日本語の設定");
    }

    #[test]
    fn duplicate_ids_are_disambiguated() {
        // Every class on an API page has an `__init__`. Duplicate ids are
        // legal HTML and `getElementById` returns the first, so without this
        // the second heading is in the sidebar and unreachable from it.
        let page = render(
            &doc(vec![
                Node::Heading {
                    level: 3,
                    id: Some("init".to_owned()),
                    children: vec![text("__init__")],
                },
                Node::Heading {
                    level: 3,
                    id: Some("init".to_owned()),
                    children: vec![text("__init__")],
                },
                Node::Heading {
                    level: 3,
                    id: None,
                    children: vec![text("init")],
                },
            ]),
            &opts(Highlighter::shared()),
        );
        let ids: Vec<&str> = page.outline.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["init", "init-2", "init-3"]);
    }

    #[test]
    fn outline_nests_by_relative_depth() {
        // h1 → h3 → h2 is ordinary in real documentation. Nesting by level
        // number would put the h3 three levels deep and lose the h2.
        let page = render(
            &doc(vec![
                Node::Heading {
                    level: 1,
                    id: Some("top".into()),
                    children: vec![text("Top")],
                },
                Node::Heading {
                    level: 3,
                    id: Some("deep".into()),
                    children: vec![text("Deep")],
                },
                Node::Heading {
                    level: 2,
                    id: Some("mid".into()),
                    children: vec![text("Mid")],
                },
                Node::Heading {
                    level: 3,
                    id: Some("under-mid".into()),
                    children: vec![text("Under mid")],
                },
            ]),
            &opts(Highlighter::shared()),
        );
        assert_eq!(page.outline.len(), 1);
        let top = &page.outline[0];
        assert_eq!(top.id, "top");
        assert_eq!(top.children.len(), 2);
        assert_eq!(top.children[0].id, "deep");
        assert_eq!(top.children[1].id, "mid");
        assert_eq!(top.children[1].children[0].id, "under-mid");
    }

    #[test]
    fn outline_stops_at_h4() {
        let page = render(
            &doc(vec![
                Node::Heading {
                    level: 4,
                    id: Some("a".into()),
                    children: vec![text("A")],
                },
                Node::Heading {
                    level: 5,
                    id: Some("b".into()),
                    children: vec![text("B")],
                },
            ]),
            &opts(Highlighter::shared()),
        );
        assert_eq!(page.outline.len(), 1);
        assert_eq!(page.outline[0].id, "a");
        // Still rendered, just not in the sidebar.
        assert!(page.html.contains(r#"<h5 id="b">"#));
    }

    #[test]
    fn title_comes_from_the_first_h1() {
        let page = render(
            &doc(vec![
                Node::Heading {
                    level: 1,
                    id: None,
                    children: vec![text("  The Page  ")],
                },
                Node::Heading {
                    level: 1,
                    id: None,
                    children: vec![text("Not this one")],
                },
            ]),
            &opts(Highlighter::shared()),
        );
        assert_eq!(page.title.as_deref(), Some("The Page"));
    }

    #[test]
    fn only_localized_assets_are_emitted_as_images() {
        // The offline guarantee, re-checked at the point of emission. S1-10
        // should make every one of these unreachable; that is the argument
        // for testing them, not against it.
        for src in [
            "https://cdn.example/logo.png",
            "//cdn.example/logo.png",
            "assets/../../../etc/passwd",
            "assets/sub/dir.png",
            "data:image/svg+xml,<svg onload=alert(1)>",
            "assets/",
            "/assets/x.png",
        ] {
            let html = render_html(&doc(vec![Node::Image {
                src: src.to_owned(),
                alt: "diagram".to_owned(),
            }]));
            assert!(
                !html.contains("<img"),
                "{src:?} rendered as an image: {html}"
            );
            assert!(html.contains("unavailable offline"), "{src:?}: {html}");
        }
    }

    #[test]
    fn localized_assets_get_the_asset_base() {
        let html = render_html(&doc(vec![Node::Image {
            src: "assets/deadbeef.png".to_owned(),
            alt: "a diagram".to_owned(),
        }]));
        assert!(
            html.contains(
                r#"<img src="tome://localhost/demo/assets/deadbeef.png" alt="a diagram""#
            ),
            "{html}"
        );
    }

    #[test]
    fn rendered_html_carries_no_remote_reference() {
        // The Stage 1 exit gate, at the render layer. `assets/` references
        // are local; nothing else may appear.
        let html = render_html(&doc(vec![
            Node::Image {
                src: "assets/a.png".to_owned(),
                alt: String::new(),
            },
            Node::Paragraph {
                children: vec![text("see https://example.com in the prose, which is fine")],
            },
        ]));
        // Prose may *mention* a URL; no attribute may point at one.
        for (at, _) in html.match_indices("src=\"") {
            let value = &html[at + 5..];
            let value = &value[..value.find('"').unwrap()];
            assert!(
                value.starts_with("tome://localhost/demo/assets/"),
                "{value}"
            );
        }
    }

    #[test]
    fn external_links_cannot_navigate_the_frame_into_the_app() {
        let html = render_html(&doc(vec![Node::Paragraph {
            children: vec![
                Node::Link {
                    href: "https://example.com/".to_owned(),
                    title: None,
                    children: vec![text("out")],
                },
                Node::Link {
                    href: "other.html".to_owned(),
                    title: None,
                    children: vec![text("in")],
                },
            ],
        }]));
        assert!(html.contains(r#"<a href="https://example.com/" rel="noopener noreferrer""#));
        assert!(html.contains(r#"<a href="other.html">"#));
    }

    #[test]
    fn tables_are_wrapped_so_they_scroll_inside_themselves() {
        let html = render_html(&doc(vec![Node::Table {
            headers: vec![TableCell {
                children: vec![text("h")],
            }],
            rows: vec![TableRow {
                cells: vec![TableCell {
                    children: vec![text("c")],
                }],
            }],
        }]));
        assert_eq!(
            html,
            "<div class=\"tome-page\"><div class=\"tome-table-scroll\"><table>\
             <thead><tr><th>h</th></tr></thead><tbody><tr><td>c</td></tr></tbody>\
             </table></div></div>"
        );
    }

    #[test]
    fn definition_lists_keep_their_permalink_ids() {
        // Every API entry on docs.python.org is one of these, and its id is
        // what a deep link from anywhere else on the internet targets.
        let html = render_html(&doc(vec![Node::DefinitionList {
            items: vec![Definition {
                id: Some("json.dumps".to_owned()),
                term: vec![Node::InlineCode {
                    code: "json.dumps(obj)".to_owned(),
                }],
                definition: vec![Node::Paragraph {
                    children: vec![text("Serialize obj.")],
                }],
            }],
        }]));
        assert!(html.contains(r#"<dt id="json.dumps">"#), "{html}");
        assert!(html.contains("<code>json.dumps(obj)</code>"), "{html}");
    }

    #[test]
    fn code_blocks_are_highlighted_and_classed() {
        let html = render_html(&doc(vec![Node::CodeBlock {
            language: Some("rust".to_owned()),
            code: "fn main() {}\n".to_owned(),
        }]));
        assert!(
            html.contains(r#"<pre><code class="language-rust">"#),
            "{html}"
        );
        assert!(html.contains("tok-storage"), "{html}");
        assert!(html.contains(r#"<span class="line">"#), "{html}");
    }

    #[test]
    fn code_blocks_without_a_language_still_render() {
        let html = render_html(&doc(vec![Node::CodeBlock {
            language: None,
            code: "plain".to_owned(),
        }]));
        assert_eq!(
            html,
            "<div class=\"tome-page\"><pre><code><span class=\"line\">plain</span></code></pre></div>"
        );
    }

    #[test]
    fn unknown_admonitions_keep_their_kind_rather_than_vanishing() {
        let html = render_html(&doc(vec![Node::Admonition {
            kind: "seealso".to_owned(),
            title: None,
            children: vec![Node::Paragraph {
                children: vec![text("body")],
            }],
        }]));
        assert!(
            html.contains(r#"class="tome-admonition tome-admonition--seealso""#),
            "{html}"
        );
        assert!(html.contains("seealso</p><p>body</p>"), "{html}");
    }

    #[test]
    fn ordered_lists_resume_where_the_source_said() {
        let html = render_html(&doc(vec![Node::List {
            ordered: true,
            start: Some(4),
            items: vec![ListItem {
                children: vec![text("four")],
            }],
        }]));
        assert!(html.contains(r#"<ol start="4">"#), "{html}");
    }

    #[test]
    fn anchors_render_as_empty_targets() {
        let html = render_html(&doc(vec![Node::Anchor {
            id: "deep-link".to_owned(),
        }]));
        assert!(html.contains(r#"<span id="deep-link"></span>"#), "{html}");
    }

    #[test]
    fn a_bare_node_renders_rather_than_failing() {
        // A renderer that can fail is a page that can fail to open.
        let html = render_html(&text("just text"));
        assert_eq!(html, "<div class=\"tome-page\">just text</div>");
    }
}
