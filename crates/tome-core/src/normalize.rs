//! AST normalization (implementation plan S1-8, ticket P1-013).
//!
//! Takes the parsed [`Node`] AST for one page and produces a *normalized*
//! one: platform-specific quirks flattened into a consistent shape the reader
//! and search can rely on. The parser (S1-7) already dropped chrome and built
//! the tree; normalization is the second, consistency-focused pass:
//!
//! 1. **URLs resolved to absolute.** The parser kept `href`/`src` as written
//!    (rewriting is a normalization decision, it said); this is where that
//!    decision is made, against the page's own URL.
//! 2. **Heading levels normalized so the top heading is `h1`.** Sphinx pages
//!    already lead with `h1`, but rustdoc and many hand-written pages start
//!    at `h2`; shifting every heading by the same offset makes "the title is
//!    h1, sections are h2" true everywhere without flattening the hierarchy.
//! 3. **Code languages canonicalized** through an alias table (`rs` → `rust`),
//!    so the highlighter (S1-11) sees one name per language.
//! 4. **Empties pruned.** Whitespace-only paragraphs and text nodes that
//!    survived parsing are dropped — they are visual noise and they confuse
//!    annotation anchoring.
//! 5. **Metadata extracted** — the title and a short description — for the
//!    library list and search snippets.
//!
//! The PRD sketches a `dyn Transform` pipeline of boxed steps. That shape
//! fits a stringly-typed HTML-in-HTML-out pass; here the AST is a typed tree,
//! so each transform is a plain function over `Node`, composed in
//! [`normalize`]. Boxing them behind a trait object would buy configurability
//! nothing uses yet and cost the compiler's exhaustiveness checking on the
//! node kinds. If per-source transform ordering is ever needed, this is the
//! place, but not before.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::model::{Definition, ListItem, Node, TableCell, TableRow};

/// A page after normalization: its metadata and its consistent AST.
///
/// Serializable so the golden corpus can diff it and so the reader/API can
/// receive it; the shape is part of the S1-1 freeze the same way the AST is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedPage {
    /// The `h1` text after heading normalization, or `None` if the page has
    /// no heading at all.
    pub title: Option<String>,
    /// The first paragraph's text, trimmed to a snippet — a description for
    /// the library list and search. `None` if the page has no prose.
    pub description: Option<String>,
    /// The normalized document. Always a [`Node::Document`] root.
    pub body: Node,
}

/// Normalize one parsed document against the URL it was fetched from.
pub fn normalize(document: Node, base: &Url) -> NormalizedPage {
    // 1. resolve URLs, 2. canonicalize code languages, 3. prune empties —
    // all one recursive pass so the tree is walked once.
    let resolved = map_node(document, base);
    // 4. heading levels need the whole tree to find the minimum, so it is a
    // second pass over the already-cleaned tree.
    let shift = heading_shift(&resolved);
    let body = shift_headings(resolved, shift);

    let title = first_h1_text(&body);
    let description = first_paragraph_text(&body);

    NormalizedPage {
        title,
        description,
        body,
    }
}

// ---------------------------------------------------------------------------
// One recursive pass: URL resolution + code language + empty pruning.
// ---------------------------------------------------------------------------

fn map_node(node: Node, base: &Url) -> Node {
    match node {
        Node::Link {
            href,
            title,
            children,
        } => Node::Link {
            href: resolve(&href, base),
            title,
            children: map_children(children, base),
        },
        Node::Image { src, alt } => Node::Image {
            src: resolve(&src, base),
            alt,
        },
        Node::CodeBlock { language, code } => Node::CodeBlock {
            language: language.map(|l| canonical_language(&l)),
            code,
        },

        Node::Document { children } => Node::Document {
            children: prune(map_children(children, base)),
        },
        Node::Heading {
            level,
            id,
            children,
        } => Node::Heading {
            level,
            id,
            children: prune(map_children(children, base)),
        },
        Node::Paragraph { children } => Node::Paragraph {
            children: prune(map_children(children, base)),
        },
        Node::Blockquote { children } => Node::Blockquote {
            children: prune(map_children(children, base)),
        },
        Node::Emphasis { children } => Node::Emphasis {
            children: prune(map_children(children, base)),
        },
        Node::Strong { children } => Node::Strong {
            children: prune(map_children(children, base)),
        },
        Node::Admonition {
            kind,
            title,
            children,
        } => Node::Admonition {
            kind,
            title,
            children: prune(map_children(children, base)),
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
                    children: prune(map_children(item.children, base)),
                })
                .collect(),
        },
        Node::DefinitionList { items } => Node::DefinitionList {
            items: items
                .into_iter()
                .map(|d| Definition {
                    id: d.id,
                    term: prune(map_children(d.term, base)),
                    definition: prune(map_children(d.definition, base)),
                })
                .collect(),
        },
        Node::Table { headers, rows } => Node::Table {
            headers: map_cells(headers, base),
            rows: rows
                .into_iter()
                .map(|r| TableRow {
                    cells: map_cells(r.cells, base),
                })
                .collect(),
        },

        // Leaves and already-terminal nodes pass through unchanged.
        other => other,
    }
}

fn map_children(children: Vec<Node>, base: &Url) -> Vec<Node> {
    children.into_iter().map(|c| map_node(c, base)).collect()
}

fn map_cells(cells: Vec<TableCell>, base: &Url) -> Vec<TableCell> {
    cells
        .into_iter()
        .map(|c| TableCell {
            children: prune(map_children(c.children, base)),
        })
        .collect()
}

/// Drop empty text nodes and empty paragraphs. Runs after the children are
/// mapped, so a paragraph that held only whitespace text becomes empty and is
/// removed.
fn prune(children: Vec<Node>) -> Vec<Node> {
    children
        .into_iter()
        .filter(|n| match n {
            Node::Text { value } => !value.trim().is_empty(),
            Node::Paragraph { children }
            | Node::Emphasis { children }
            | Node::Strong { children } => !children.is_empty(),
            _ => true,
        })
        .collect()
}

/// Resolve a possibly-relative URL against the page base. On failure (a
/// malformed href), the original is kept — dropping a link silently would
/// lose content, and the sanitizer (S1-9) is the layer that decides what is
/// safe to render, not this one.
fn resolve(href: &str, base: &Url) -> String {
    match base.join(href) {
        Ok(url) => url.to_string(),
        Err(_) => href.to_owned(),
    }
}

/// Canonical language name for the highlighter. Aliases and a few common
/// shorthands map to one spelling; an unknown language is lowercased and kept
/// verbatim (the highlighter falls back to plain rendering, never errors).
fn canonical_language(language: &str) -> String {
    let lower = language.trim().to_ascii_lowercase();
    match lower.as_str() {
        "rs" => "rust",
        "py" | "python3" | "py3" => "python",
        "js" | "node" | "nodejs" => "javascript",
        "ts" => "typescript",
        "sh" | "shell" | "console" | "shell-session" | "bash-session" => "bash",
        "yml" => "yaml",
        "md" => "markdown",
        "c++" | "cpp" => "cpp",
        "c#" | "cs" => "csharp",
        "rb" => "ruby",
        "text" | "plain" | "plaintext" | "none" => "text",
        _ => return lower,
    }
    .to_owned()
}

// ---------------------------------------------------------------------------
// Heading normalization: shift every heading so the shallowest becomes h1.
// ---------------------------------------------------------------------------

/// How much to subtract from every heading level so the minimum present level
/// becomes 1. Zero if there are no headings or the top is already h1.
fn heading_shift(node: &Node) -> u8 {
    fn min_level(node: &Node, current: &mut u8) {
        if let Node::Heading { level, .. } = node {
            *current = (*current).min(*level);
        }
        for child in child_nodes(node) {
            min_level(child, current);
        }
    }
    let mut min = 7u8;
    min_level(node, &mut min);
    if min == 7 || min <= 1 {
        0
    } else {
        min - 1
    }
}

fn shift_headings(node: Node, shift: u8) -> Node {
    if shift == 0 {
        return node;
    }
    map_headings(node, &|level| level.saturating_sub(shift).max(1))
}

/// Apply `f` to every heading level in the tree, rebuilding it.
fn map_headings(node: Node, f: &dyn Fn(u8) -> u8) -> Node {
    match node {
        Node::Heading {
            level,
            id,
            children,
        } => Node::Heading {
            level: f(level).clamp(1, 6),
            id,
            children: children.into_iter().map(|c| map_headings(c, f)).collect(),
        },
        Node::Document { children } => Node::Document {
            children: children.into_iter().map(|c| map_headings(c, f)).collect(),
        },
        Node::Paragraph { children } => Node::Paragraph {
            children: children.into_iter().map(|c| map_headings(c, f)).collect(),
        },
        Node::Blockquote { children } => Node::Blockquote {
            children: children.into_iter().map(|c| map_headings(c, f)).collect(),
        },
        Node::Admonition {
            kind,
            title,
            children,
        } => Node::Admonition {
            kind,
            title,
            children: children.into_iter().map(|c| map_headings(c, f)).collect(),
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
                    children: item
                        .children
                        .into_iter()
                        .map(|c| map_headings(c, f))
                        .collect(),
                })
                .collect(),
        },
        Node::DefinitionList { items } => Node::DefinitionList {
            items: items
                .into_iter()
                .map(|d| Definition {
                    id: d.id,
                    term: d.term.into_iter().map(|c| map_headings(c, f)).collect(),
                    definition: d
                        .definition
                        .into_iter()
                        .map(|c| map_headings(c, f))
                        .collect(),
                })
                .collect(),
        },
        // Headings do not occur inside tables/inline nodes in practice, and
        // leaves have no children; pass through.
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Metadata extraction.
// ---------------------------------------------------------------------------

fn first_h1_text(node: &Node) -> Option<String> {
    for child in child_nodes(node) {
        if let Node::Heading { level: 1, .. } = child {
            let text = collapse(&child.text_content());
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

/// The first paragraph's text, trimmed to ~200 characters on a word boundary.
fn first_paragraph_text(node: &Node) -> Option<String> {
    fn find(node: &Node) -> Option<String> {
        if let Node::Paragraph { .. } = node {
            let text = collapse(&node.text_content());
            if !text.is_empty() {
                return Some(text);
            }
        }
        child_nodes(node).iter().find_map(|c| find(c))
    }
    find(node).map(|text| truncate_words(&text, 200))
}

fn truncate_words(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out = String::new();
    for word in text.split_whitespace() {
        if out.chars().count() + word.chars().count() + 1 > max {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out.push('…');
    out
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The direct children of a container node, for recursion. Non-container
/// nodes yield nothing.
fn child_nodes(node: &Node) -> Vec<&Node> {
    match node {
        Node::Document { children }
        | Node::Heading { children, .. }
        | Node::Paragraph { children }
        | Node::Blockquote { children }
        | Node::Emphasis { children }
        | Node::Strong { children }
        | Node::Admonition { children, .. }
        | Node::Link { children, .. } => children.iter().collect(),
        Node::List { items, .. } => items.iter().flat_map(|i| i.children.iter()).collect(),
        Node::DefinitionList { items } => items
            .iter()
            .flat_map(|d| d.term.iter().chain(d.definition.iter()))
            .collect(),
        Node::Table { headers, rows } => headers
            .iter()
            .flat_map(|c| c.children.iter())
            .chain(
                rows.iter()
                    .flat_map(|r| r.cells.iter().flat_map(|c| c.children.iter())),
            )
            .collect(),
        _ => Vec::new(),
    }
}
