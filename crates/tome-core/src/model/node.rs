//! The normalized document AST.
//!
//! This is the shape documentation has *after* parsing (S1-7) and
//! normalization (S1-8): platform HTML goes in, [`Node`] comes out, and the
//! renderer, the sanitizer's anchor corpus, search snippets, and exports all
//! consume this rather than HTML. The serde form is part of the freeze —
//! `tests/model_serde_shape.rs` pins the exact JSON.
//!
//! Design rules, each learned somewhere:
//!
//! - **Every variant is struct-shaped.** Internally-tagged serde (`"type":
//!   "heading"`) does not support tuple variants, and positional fields
//!   cannot gain a member without breaking every consumer anyway.
//! - **There is no raw-HTML passthrough variant.** A `Raw(String)` node
//!   would be a sanitizer bypass by construction — whatever S1-9 guarantees,
//!   one `Raw` in the tree un-guarantees it. Anything the AST cannot
//!   represent is transformed by normalization or dropped, and the golden
//!   corpus is where "dropped too much" gets caught.
//! - **`DefinitionList` is not optional sugar.** Sphinx renders every API
//!   entry — every function, class, and method on docs.python.org — as a
//!   `<dl>`; an AST without definition lists mangles the exact pages the
//!   Stage 1 exit gate renders.
//! - **`id` anchors survive.** TOC deep links and `#fragment` cross-references
//!   resolve against these. The original sanitizer draft stripped `id` and
//!   silently broke the TOC; the AST keeps anchors as data so no later stage
//!   has to re-derive them from markup.

use serde::{Deserialize, Serialize};

/// One node of a normalized document. The root is always
/// [`Node::Document`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Node {
    Document {
        children: Vec<Node>,
    },
    Heading {
        /// 1..=6. Normalization (S1-8) owns clamping — by the time a tree is
        /// stored, exactly one h1 exists and it is the title.
        level: u8,
        /// Anchor target, kept verbatim from the source page so existing
        /// `#fragment` links keep working.
        id: Option<String>,
        children: Vec<Node>,
    },
    Paragraph {
        children: Vec<Node>,
    },
    CodeBlock {
        /// Normalized language identifier (`"rust"`, `"python"`), or `None`
        /// when unknown. S1-8 owns the alias table (`rs` → `rust`).
        language: Option<String>,
        code: String,
    },
    Blockquote {
        children: Vec<Node>,
    },
    List {
        ordered: bool,
        /// Starting number for ordered lists that resume (`<ol start="4">`).
        start: Option<u32>,
        items: Vec<ListItem>,
    },
    DefinitionList {
        items: Vec<Definition>,
    },
    Table {
        headers: Vec<TableCell>,
        rows: Vec<TableRow>,
    },
    /// Sphinx/MkDocs note, warning, tip, … `kind` is the lowercase name as
    /// normalization classified it; renderers must treat unknown kinds as a
    /// plain note rather than dropping the block.
    Admonition {
        kind: String,
        title: Option<String>,
        children: Vec<Node>,
    },
    Image {
        /// As written in the source until S1-10 rewrites it to a local asset
        /// path — the offline gate asserts no `http` survives to render.
        src: String,
        alt: String,
    },
    ThematicBreak {},

    // Inline nodes.
    Text {
        value: String,
    },
    Emphasis {
        children: Vec<Node>,
    },
    Strong {
        children: Vec<Node>,
    },
    InlineCode {
        code: String,
    },
    Link {
        /// Kept as written; link rewriting (internal vs external) is a
        /// normalization/render concern, not an AST one.
        href: String,
        title: Option<String>,
        children: Vec<Node>,
    },
    /// A bare anchor target (`<span id="…">`) that is not a heading. Kept so
    /// deep links into long sections survive normalization.
    Anchor {
        id: String,
    },
    LineBreak {},
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem {
    pub children: Vec<Node>,
}

/// One `<dt>`/`<dd>` pair. Sphinx puts the API signature in the term and the
/// prose in the definition; the `id` is the permalink target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Definition {
    pub id: Option<String>,
    pub term: Vec<Node>,
    pub definition: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCell {
    pub children: Vec<Node>,
}

impl Node {
    /// The concatenated text content, in document order. What annotation
    /// anchoring and snippet generation search within.
    pub fn text_content(&self) -> String {
        let mut out = String::new();
        self.collect_text(&mut out);
        out
    }

    fn collect_text(&self, out: &mut String) {
        match self {
            Self::Text { value } => out.push_str(value),
            Self::InlineCode { code } | Self::CodeBlock { code, .. } => out.push_str(code),
            Self::Image { alt, .. } => out.push_str(alt),
            Self::Document { children }
            | Self::Heading { children, .. }
            | Self::Paragraph { children }
            | Self::Blockquote { children }
            | Self::Admonition { children, .. }
            | Self::Emphasis { children }
            | Self::Strong { children }
            | Self::Link { children, .. } => {
                for child in children {
                    child.collect_text(out);
                }
            }
            Self::List { items, .. } => {
                for item in items {
                    for child in &item.children {
                        child.collect_text(out);
                    }
                }
            }
            Self::DefinitionList { items } => {
                for item in items {
                    for child in item.term.iter().chain(&item.definition) {
                        child.collect_text(out);
                    }
                }
            }
            Self::Table { headers, rows } => {
                for cell in headers {
                    for child in &cell.children {
                        child.collect_text(out);
                    }
                }
                for row in rows {
                    for cell in &row.cells {
                        for child in &cell.children {
                            child.collect_text(out);
                        }
                    }
                }
            }
            Self::ThematicBreak {} | Self::Anchor { .. } | Self::LineBreak {} => {}
        }
    }
}
