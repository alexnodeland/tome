//! Sanitization of the normalized AST (implementation plan S1-9, security).
//!
//! The reader renders documentation HTML that Tome did not write, inside a
//! sandboxed iframe with a restrictive CSP (SPIKE-002). The CSP is the outer
//! wall; this sanitizer is the inner one, and the plan requires both — a
//! security control that depends on exactly one layer holding is one edit
//! away from failing open.
//!
//! What makes this sanitizer unlike a classic HTML sanitizer: it runs on the
//! *typed* [`Node`] AST, which by construction has **no raw-HTML node** (see
//! `model/node.rs` — a `Raw(String)` would be a bypass by design). So there
//! is no markup to strip; the attack surface is the handful of AST fields
//! that carry attacker-controlled strings into a rendering context:
//!
//! - **`Link.href` / `Image.src`** — a `javascript:` or `data:text/html`
//!   URL is script execution when the renderer sets it as an attribute.
//!   These are scheme-allowlisted; a rejected link keeps its text (the
//!   content survives, the vector does not).
//! - **`id` attributes** (`Heading.id`, `Anchor.id`, `Definition.id`) — an
//!   id becomes an HTML `id=` attribute, and one containing a quote or angle
//!   bracket could break out of attribute context. These are sanitized to a
//!   safe token **without being dropped**: the original S1-9 draft stripped
//!   `id` wholesale and silently killed the TOC and every `#fragment` link,
//!   a security control breaking a headline feature. Anchors are a corpus
//!   this sanitizer must *not* break, tested alongside the XSS corpus.
//! - **`Admonition.kind`** — becomes a CSS class token; constrained to
//!   `[a-z0-9-]`.
//!
//! - **`CodeBlock.language`** — becomes a `language-<lang>` CSS class for the
//!   highlighter, so it is a *token* like an admonition kind, not free text;
//!   sanitized to `[a-z0-9-]` (a language of `x" onload="` would otherwise
//!   inject through the class attribute).
//!
//! # The renderer contract for free text
//!
//! Some fields are attribute surfaces but hold **free text**, not tokens:
//! `Link.title`, `Image.alt`, `Admonition.title`, and the text of `Text` /
//! `InlineCode` / `CodeBlock.code` nodes. These cannot be made safe by
//! charset-stripping without destroying legitimate content (a title may
//! legitimately contain a quote), so their safety is the **renderer's**
//! responsibility, and it is a hard contract, not a hope: **S1-13's reader
//! must quote every attribute and HTML-escape every attribute value and every
//! text node.** That is table stakes for any HTML renderer and is stated here
//! so the assumption is explicit rather than silent — the refute-panel flagged
//! these fields precisely because the contract was unwritten. The sanitizer's
//! own job stops at the *tokens* (URL schemes, ids, class names) that
//! charset-restriction genuinely can secure; escaping is the renderer's.

use crate::model::{Definition, ListItem, Node, TableCell, TableRow};

/// Schemes permitted in a link `href`. `mailto` is kept — documentation
/// links to project contacts; everything script-capable (`javascript`,
/// `data`, `vbscript`, `file`) is not here.
const LINK_SCHEMES: &[&str] = &["http", "https", "mailto"];

/// Schemes permitted in an image `src`. No `data:` — a `data:` image is a
/// known SVG-script vector, and asset localization (S1-10) rewrites real
/// image URLs to local paths anyway, so nothing legitimate needs it here.
const IMAGE_SCHEMES: &[&str] = &["http", "https"];

/// Sanitize a normalized document. Idempotent: sanitizing already-clean
/// output changes nothing.
pub fn sanitize(node: Node) -> Node {
    match node {
        Node::Link {
            href,
            title,
            children,
        } => {
            if scheme_allowed(&href, LINK_SCHEMES) {
                Node::Link {
                    // A same-page `#fragment` link is sanitized the same way
                    // its target id is, so the two stay consistent.
                    href: sanitize_fragment_href(&href),
                    title,
                    children: sanitize_all(children),
                }
            } else {
                // Neutralize the vector, keep the words: a link with a
                // forbidden scheme collapses to its own text content — inline
                // and safe, so "click [here](javascript:...)" still reads
                // "click here". Link children are inline, so flattening to
                // text loses only the (rare) formatting inside a bad link.
                let text = Node::Link {
                    href: String::new(),
                    title: None,
                    children,
                }
                .text_content();
                Node::Text { value: text }
            }
        }
        Node::Image { src, alt } => {
            if scheme_allowed(&src, IMAGE_SCHEMES) {
                Node::Image { src, alt }
            } else {
                // A blocked image degrades to its alt text (or nothing).
                if alt.trim().is_empty() {
                    Node::Text {
                        value: String::new(),
                    }
                } else {
                    Node::Text { value: alt }
                }
            }
        }

        Node::Heading {
            level,
            id,
            children,
        } => Node::Heading {
            level,
            id: id.map(sanitize_id).filter(|s| !s.is_empty()),
            children: sanitize_all(children),
        },
        Node::Anchor { id } => Node::Anchor {
            id: sanitize_id(id),
        },
        Node::DefinitionList { items } => Node::DefinitionList {
            items: items
                .into_iter()
                .map(|d| Definition {
                    id: d.id.map(sanitize_id).filter(|s| !s.is_empty()),
                    term: sanitize_all(d.term),
                    definition: sanitize_all(d.definition),
                })
                .collect(),
        },
        Node::Admonition {
            kind,
            title,
            children,
        } => Node::Admonition {
            kind: sanitize_class_token(&kind),
            title,
            children: sanitize_all(children),
        },

        // Recurse through the remaining containers.
        Node::Document { children } => Node::Document {
            children: sanitize_all(children),
        },
        Node::Paragraph { children } => Node::Paragraph {
            children: sanitize_all(children),
        },
        Node::Blockquote { children } => Node::Blockquote {
            children: sanitize_all(children),
        },
        Node::Emphasis { children } => Node::Emphasis {
            children: sanitize_all(children),
        },
        Node::Strong { children } => Node::Strong {
            children: sanitize_all(children),
        },
        Node::List {
            ordered,
            start,
            items,
        } => Node::List {
            ordered,
            start,
            items: items
                .into_iter()
                .map(|item| ListItem {
                    children: sanitize_all(item.children),
                })
                .collect(),
        },
        Node::Table { headers, rows } => Node::Table {
            headers: sanitize_cells(headers),
            rows: rows
                .into_iter()
                .map(|r| TableRow {
                    cells: sanitize_cells(r.cells),
                })
                .collect(),
        },

        // CodeBlock.language becomes a CSS class token (`language-<lang>`)
        // for the highlighter, so it is a token, not free text — a language
        // like `x" onload="` would inject through the class attribute. `code`
        // is text, escaped by the renderer, and is left alone.
        Node::CodeBlock { language, code } => Node::CodeBlock {
            language: language.and_then(sanitize_language),
            code,
        },

        // Text, InlineCode, ThematicBreak, LineBreak: rendered as text or
        // structure, no attribute/URL/class surface. Unchanged.
        other => other,
    }
}

fn sanitize_all(children: Vec<Node>) -> Vec<Node> {
    children.into_iter().map(sanitize).collect()
}

fn sanitize_cells(cells: Vec<TableCell>) -> Vec<TableCell> {
    cells
        .into_iter()
        .map(|c| TableCell {
            children: sanitize_all(c.children),
        })
        .collect()
}

/// Whether a URL's scheme is in the allowlist. A URL with no scheme (a
/// relative or fragment reference) is allowed — normalization already made
/// document URLs absolute, and a bare `#fragment` is same-page navigation.
///
/// Accepted residual (S1-9 round-2 refute-panel, ruled low/not-real): a
/// protocol-relative `//host/x` URL is "no scheme → allowed". For a *link*
/// that is ordinary external navigation, no worse than an `http://` link the
/// allowlist already permits. For an *image* it would be an off-origin load,
/// but that is caught downstream where the offline guarantee is actually
/// enforced: asset localization (S1-10) rewrites every image URL to a local
/// path, the reader CSP restricts `img-src`, and the offline exit-gate test
/// asserts no `http`/`//` reference survives to render. Blocking it here too
/// would be belt-and-braces the offline gate makes redundant.
///
/// The scheme is parsed the way a browser does: the part before the first
/// `:` that is not preceded by `/`, `?`, or `#`. This matters because
/// `javascript:alert(1)` and ` javascript:alert(1)` (leading control chars,
/// which browsers strip) must both be caught, while `path:with:colons.html`
/// (a relative path, no scheme) must not be mistaken for one.
fn scheme_allowed(url: &str, allowed: &[&str]) -> bool {
    match extract_scheme(url) {
        Some(scheme) => allowed.contains(&scheme.as_str()),
        // No scheme → relative/fragment → allowed.
        None => true,
    }
}

/// The scheme of a URL, lowercased, or `None` if it is relative.
///
/// Mirrors the WHATWG URL parser's two pre-processing steps that an attacker
/// hides a scheme behind:
///
/// 1. **All ASCII tab and newline (`\t`, `\n`, `\r`) are removed from the
///    whole URL**, not just the front. This is the confirmed `java\tscript:`
///    bypass (S1-9 refute-panel): a browser strips the interior tab and runs
///    `javascript:`, so the scheme parser must see the same collapsed string.
///    The earlier version only trimmed *leading* controls and treated an
///    interior one as "no scheme → allowed", which let the payload through.
/// 2. **Leading C0 controls and spaces are trimmed** — `\x01javascript:` also
///    runs in a browser.
///
/// After that, a scheme is `[a-z][a-z0-9+.-]*` up to the first `:`, before any
/// `/`, `?`, or `#`.
fn extract_scheme(url: &str) -> Option<String> {
    // Step 1: remove ASCII tab/LF/CR everywhere.
    let collapsed: String = url
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    // Step 2: trim leading controls and spaces.
    let trimmed = collapsed.trim_start_matches(|c: char| c.is_control() || c == ' ');

    let mut scheme = String::new();
    let mut chars = trimmed.chars();

    // First char must be a letter.
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => scheme.push(c.to_ascii_lowercase()),
        _ => return None,
    }
    for c in chars {
        match c {
            ':' => return Some(scheme),
            // Reached a path/query/fragment before a colon: no scheme.
            '/' | '?' | '#' => return None,
            c if c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-') => {
                scheme.push(c.to_ascii_lowercase());
            }
            // Any other interior character means this is not a well-formed
            // scheme; treat as relative. (Tab/LF/CR were already removed, so
            // they can no longer reach this arm and masquerade as no-scheme.)
            _ => return None,
        }
    }
    None
}

/// Sanitize an `id`/fragment token, **removing only what is genuinely unsafe
/// in an HTML `id=` attribute** and keeping everything else — including
/// non-ASCII letters and the punctuation real documentation uses.
///
/// The earlier version was an ASCII allowlist (`[A-Za-z0-9-_.:]`). The
/// refute-panel confirmed two ways that broke real docs: it emptied wholly
/// non-Latin ids (`café`, CJK slugs — killing the TOC and every `#fragment`
/// on non-English sites) and it dropped `<`, `>`, `%` from rustdoc anchors
/// like `impl-From<T>-for-T`. Those characters are *not* attribute-breaking:
/// inside a quoted attribute value the HTML parser treats `<`/`>`/`&` as
/// literal text — only the matching quote ends the value.
///
/// So this is a denylist of the characters that can break out of, or split,
/// an attribute even when the renderer is naive:
///
/// - ASCII quotes `"` and `'` (end a quoted value) and backtick,
/// - ASCII whitespace (splits an *unquoted* value into extra attributes),
/// - C0/C1 control characters (invalid in an id, and used for smuggling).
///
/// Everything else survives, so unicode ids and rustdoc/Sphinx punctuation
/// keep working. The renderer's own contract (see module docs) is to quote
/// and HTML-escape attribute values; this strip is the defense-in-depth that
/// holds even if it does not.
fn sanitize_id(id: String) -> String {
    id.chars()
        .filter(|c| !(c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '`')))
        .collect()
}

/// Sanitize the fragment of a same-page `#target` link the same way an id is,
/// so a link and the heading it points at stay consistent after
/// sanitization. Without this the two go through different code paths and a
/// stripped character desyncs the deep link (a confirmed anchor-break).
fn sanitize_fragment_href(href: &str) -> String {
    match href.strip_prefix('#') {
        Some(fragment) => format!("#{}", sanitize_id(fragment.to_owned())),
        None => href.to_owned(),
    }
}

/// Sanitize a code-block language to a CSS class token `[a-z0-9-]`, or `None`
/// if nothing valid remains (an unknown/empty language renders as plain text,
/// never errors). Distinct from [`sanitize_class_token`] only in that it has
/// no default fallback — "no language" is a real, correct state.
fn sanitize_language(language: String) -> Option<String> {
    let cleaned: String = language
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Sanitize an admonition kind to a CSS class token `[a-z0-9-]`. An empty or
/// fully-invalid kind becomes `note`, the renderer's default.
fn sanitize_class_token(kind: &str) -> String {
    let cleaned: String = kind
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if cleaned.is_empty() {
        "note".to_owned()
    } else {
        cleaned
    }
}
