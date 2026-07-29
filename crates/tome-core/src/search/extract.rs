//! Flattening a page's AST into the schema's text fields.
//!
//! Search operates on text, the store holds a tree, and this is the one place
//! that converts between them. Which node's text lands in which field decides
//! what ranking can distinguish later, so the routing is spelled out rather
//! than left to a generic walk:
//!
//! | Node | Field |
//! |---|---|
//! | `Heading` | `headers` (one value per heading) |
//! | `CodeBlock`, `InlineCode` | `code` |
//! | everything else with text | `body` |
//!
//! `Image.alt` goes to `body`: it is prose describing the page, and a diagram's
//! alt text is often the only place a concept is named. `Link.title` likewise.
//! **`Link.href` and `Image.src` are not indexed at all** — after S1-10 and
//! `pipeline::relink` those are local library paths and content-addressed asset
//! filenames, so indexing them would fill the term dictionary with hashes.

use crate::model::Node;

/// A page's text, split into the fields [`super::schema`] defines.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Extracted {
    /// One entry per heading, in document order.
    pub headers: Vec<String>,
    pub body: String,
    pub code: String,
}

/// Flatten a document tree.
///
/// The argument is normally [`Node::Document`], but any node works — the walk
/// is structural, not rooted.
pub fn extract(root: &Node) -> Extracted {
    let mut out = Extracted::default();
    walk(root, &mut out);
    out
}

fn walk(node: &Node, out: &mut Extracted) {
    match node {
        Node::Heading { children, .. } => {
            // Collected into its own string rather than appended to `body`,
            // so `headers` stays one value per heading. A phrase query must
            // not be able to run off the end of one heading into the next.
            let mut text = String::new();
            for child in children {
                inline_text(child, &mut text);
            }
            let text = text.trim();
            if !text.is_empty() {
                out.headers.push(text.to_owned());
            }
        }

        Node::CodeBlock { code, .. } => push(&mut out.code, code),
        Node::InlineCode { code } => push(&mut out.code, code),

        Node::Text { value } => push(&mut out.body, value),
        Node::Image { alt, .. } => push(&mut out.body, alt),
        Node::Link {
            title, children, ..
        } => {
            if let Some(title) = title {
                push(&mut out.body, title);
            }
            for child in children {
                walk(child, out);
            }
        }

        Node::Admonition {
            kind,
            title,
            children,
        } => {
            // The kind ("warning", "deprecated") is searchable text: "python
            // deprecated" is a real query, and the word may appear nowhere in
            // the prose.
            push(&mut out.body, kind);
            if let Some(title) = title {
                push(&mut out.body, title);
            }
            for child in children {
                walk(child, out);
            }
        }

        Node::Document { children }
        | Node::Paragraph { children }
        | Node::Blockquote { children }
        | Node::Emphasis { children }
        | Node::Strong { children } => {
            for child in children {
                walk(child, out);
            }
        }

        Node::List { items, .. } => {
            for item in items {
                for child in &item.children {
                    walk(child, out);
                }
            }
        }

        Node::DefinitionList { items } => {
            for item in items {
                // Sphinx puts the API signature in the term, which is the
                // single highest-value string on a reference page.
                for child in &item.term {
                    walk(child, out);
                }
                for child in &item.definition {
                    walk(child, out);
                }
            }
        }

        Node::Table { headers, rows } => {
            for cell in headers {
                for child in &cell.children {
                    walk(child, out);
                }
            }
            for row in rows {
                for cell in &row.cells {
                    for child in &cell.children {
                        walk(child, out);
                    }
                }
            }
        }

        // No text: `Anchor` is an id, and ids are navigation targets rather
        // than prose. `ThematicBreak` and `LineBreak` carry nothing.
        Node::Anchor { .. } | Node::ThematicBreak {} | Node::LineBreak {} => {} // Deliberately no catch-all arm. `Node` is `#[non_exhaustive]`, but
                                                                                // that only binds other crates — inside `tome-core` this match must
                                                                                // stay exhaustive, so adding a variant fails the build here and forces
                                                                                // a decision about which field its text belongs in. A `_ => {}` would
                                                                                // route new content to nowhere and be discoverable only as a page
                                                                                // that cannot be found by words it visibly contains.
    }
}

/// Append inline text only, ignoring block structure.
///
/// Used for headings, where the whole point is to keep one heading's words in
/// one value.
fn inline_text(node: &Node, out: &mut String) {
    match node {
        Node::Text { value } => push(out, value),
        Node::InlineCode { code } => push(out, code),
        Node::Emphasis { children } | Node::Strong { children } => {
            for child in children {
                inline_text(child, out);
            }
        }
        Node::Link { children, .. } => {
            for child in children {
                inline_text(child, out);
            }
        }
        _ => {}
    }
}

/// Append with a separating space.
///
/// The space matters: concatenating adjacent fragments without one turns
/// `<code>Vec</code><code>new</code>` into the single term `Vecnew`. This is
/// the same class of defect S1-8 hit from the other direction, where inserting
/// a space between text fragments turned `a&amp;b` into "a & b" — there, the
/// fragments were one word split by an entity; here they are separate nodes.
/// Tokenization discards the whitespace either way, so a spurious separator
/// costs nothing and a missing one merges two terms.
fn push(out: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(text);
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::model::{Definition, ListItem, TableCell, TableRow};

    fn text(value: &str) -> Node {
        Node::Text {
            value: value.to_owned(),
        }
    }

    fn heading(level: u8, value: &str) -> Node {
        Node::Heading {
            level,
            id: None,
            children: vec![text(value)],
        }
    }

    fn para(value: &str) -> Node {
        Node::Paragraph {
            children: vec![text(value)],
        }
    }

    #[test]
    fn headings_become_separate_values() {
        let doc = Node::Document {
            children: vec![heading(1, "Reading Files"), heading(2, "Error Handling")],
        };
        assert_eq!(
            extract(&doc).headers,
            ["Reading Files", "Error Handling"],
            "each heading is its own value so a phrase cannot straddle two"
        );
    }

    #[test]
    fn code_is_separated_from_prose() {
        let doc = Node::Document {
            children: vec![
                para("Open the file."),
                Node::CodeBlock {
                    language: Some("rust".to_owned()),
                    code: "let s = read_to_string(p)?;".to_owned(),
                },
            ],
        };
        let got = extract(&doc);
        assert_eq!(got.body, "Open the file.");
        assert_eq!(got.code, "let s = read_to_string(p)?;");
    }

    #[test]
    fn inline_code_lands_in_the_code_field() {
        let doc = Node::Document {
            children: vec![Node::Paragraph {
                children: vec![
                    text("Call"),
                    Node::InlineCode {
                        code: "Vec::new".to_owned(),
                    },
                    text("first."),
                ],
            }],
        };
        let got = extract(&doc);
        assert_eq!(got.body, "Call first.");
        assert_eq!(got.code, "Vec::new");
    }

    #[test]
    fn adjacent_fragments_do_not_merge_into_one_term() {
        let doc = Node::Document {
            children: vec![Node::Paragraph {
                children: vec![text("alpha"), text("beta")],
            }],
        };
        assert_eq!(extract(&doc).body, "alpha beta");
    }

    #[test]
    fn local_paths_and_asset_hashes_are_not_indexed() {
        // After S1-10 and relink these are content-addressed filenames and
        // library paths. Indexing them would fill the dictionary with hashes.
        let doc = Node::Document {
            children: vec![
                Node::Image {
                    src: "assets/9f86d081884c7d65.png".to_owned(),
                    alt: "Pipeline diagram".to_owned(),
                },
                Node::Link {
                    href: "tutorial/index.html".to_owned(),
                    title: Some("The tutorial".to_owned()),
                    children: vec![text("start here")],
                },
            ],
        };
        let got = extract(&doc);
        assert!(got.body.contains("Pipeline diagram"));
        assert!(got.body.contains("The tutorial"));
        assert!(got.body.contains("start here"));
        assert!(!got.body.contains("9f86d081884c7d65"));
        assert!(!got.body.contains("tutorial/index.html"));
    }

    #[test]
    fn admonition_kind_is_searchable() {
        let doc = Node::Document {
            children: vec![Node::Admonition {
                kind: "deprecated".to_owned(),
                title: Some("Since 3.12".to_owned()),
                children: vec![para("Use the new API.")],
            }],
        };
        let body = extract(&doc).body;
        assert!(body.contains("deprecated"), "got {body:?}");
        assert!(body.contains("Since 3.12"));
        assert!(body.contains("Use the new API."));
    }

    #[test]
    fn definition_terms_are_indexed() {
        let doc = Node::Document {
            children: vec![Node::DefinitionList {
                items: vec![Definition {
                    id: None,
                    term: vec![Node::InlineCode {
                        code: "open(file, mode='r')".to_owned(),
                    }],
                    definition: vec![para("Open file and return a stream.")],
                }],
            }],
        };
        let got = extract(&doc);
        assert!(got.code.contains("open(file, mode='r')"));
        assert!(got.body.contains("Open file and return a stream."));
    }

    #[test]
    fn table_cells_are_indexed() {
        let doc = Node::Document {
            children: vec![Node::Table {
                headers: vec![TableCell {
                    children: vec![text("Method")],
                }],
                rows: vec![TableRow {
                    cells: vec![TableCell {
                        children: vec![text("readlines")],
                    }],
                }],
            }],
        };
        let body = extract(&doc).body;
        assert!(body.contains("Method"));
        assert!(body.contains("readlines"));
    }

    #[test]
    fn list_items_are_indexed() {
        let doc = Node::Document {
            children: vec![Node::List {
                ordered: false,
                start: None,
                items: vec![ListItem {
                    children: vec![para("First point")],
                }],
            }],
        };
        assert!(extract(&doc).body.contains("First point"));
    }

    #[test]
    fn heading_keeps_inline_code_in_the_same_value() {
        let doc = Node::Document {
            children: vec![Node::Heading {
                level: 2,
                id: Some("vec-new".to_owned()),
                children: vec![
                    Node::InlineCode {
                        code: "Vec::new".to_owned(),
                    },
                    text("constructor"),
                ],
            }],
        };
        assert_eq!(extract(&doc).headers, ["Vec::new constructor"]);
    }

    #[test]
    fn empty_document_extracts_nothing() {
        let got = extract(&Node::Document { children: vec![] });
        assert_eq!(got, Extracted::default());
    }
}
