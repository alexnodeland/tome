# Parser Directory

HTML parsing and normalization pipeline.

## What Belongs Here

- **HTML-to-AST parser** converting raw HTML to structured tree
- **Normalization transforms** for consistent output
- **Syntax highlighting** for code blocks
- **Link resolution** and URL handling

## What Does NOT Belong Here

- HTTP fetching (use `scraper/`)
- Database operations (use `storage/`)
- Search indexing (use `search/`)
- Command handlers (use `commands/`)

## Module Structure

```
parser/
├── mod.rs              # Module exports
├── ast.rs              # AST node definitions
├── html_parser.rs      # HTML to AST conversion
├── normalize.rs        # Normalization pipeline
├── transforms/         # Individual transforms
│   ├── mod.rs
│   ├── strip_nav.rs    # Remove navigation elements
│   ├── headings.rs     # Normalize heading levels
│   ├── links.rs        # Resolve URLs
│   ├── code.rs         # Process code blocks
│   └── whitespace.rs   # Clean up whitespace
├── highlight.rs        # Syntax highlighting
├── toc.rs              # TOC extraction
└── tests.rs            # Unit tests
```

## AST Definitions

```rust
// ast.rs
use serde::{Deserialize, Serialize};

/// Document AST node types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Node {
    /// Root document node
    Document { children: Vec<Node> },

    /// Heading (h1-h6)
    Heading {
        level: u8,
        id: Option<String>,
        children: Vec<Node>,
    },

    /// Paragraph
    Paragraph { children: Vec<Node> },

    /// Fenced code block
    CodeBlock {
        language: Option<String>,
        content: String,
        highlighted: Option<String>,
    },

    /// Inline code
    InlineCode { content: String },

    /// Hyperlink
    Link {
        href: String,
        title: Option<String>,
        children: Vec<Node>,
    },

    /// Unordered or ordered list
    List {
        ordered: bool,
        start: Option<u32>,
        items: Vec<Vec<Node>>,
    },

    /// Table
    Table {
        headers: Vec<Vec<Node>>,
        rows: Vec<Vec<Vec<Node>>>,
    },

    /// Block quote
    BlockQuote { children: Vec<Node> },

    /// Horizontal rule
    ThematicBreak,

    /// Plain text
    Text { content: String },

    /// Strong (bold) text
    Strong { children: Vec<Node> },

    /// Emphasis (italic) text
    Emphasis { children: Vec<Node> },

    /// Image
    Image {
        src: String,
        alt: String,
        title: Option<String>,
    },

    /// Generic container (div)
    Container {
        class: Option<String>,
        children: Vec<Node>,
    },
}

impl Node {
    /// Get plain text content recursively
    pub fn text_content(&self) -> String {
        match self {
            Node::Text { content } => content.clone(),
            Node::InlineCode { content } => content.clone(),
            Node::CodeBlock { content, .. } => content.clone(),
            Node::Document { children }
            | Node::Heading { children, .. }
            | Node::Paragraph { children }
            | Node::Link { children, .. }
            | Node::BlockQuote { children }
            | Node::Strong { children }
            | Node::Emphasis { children }
            | Node::Container { children, .. } => {
                children.iter().map(|c| c.text_content()).collect()
            }
            Node::List { items, .. } => items
                .iter()
                .flat_map(|item| item.iter().map(|n| n.text_content()))
                .collect(),
            _ => String::new(),
        }
    }
}
```

## Parser Pattern

```rust
// html_parser.rs
use html5ever::{
    parse_document, tendril::TendrilSink,
    tree_builder::TreeBuilderOpts,
};
use markup5ever_rcdom::{RcDom, Handle, NodeData};
use crate::error::ParseError;
use super::ast::Node;

pub struct HtmlParser;

impl HtmlParser {
    /// Parse HTML string into AST
    pub fn parse(html: &str) -> Result<Node, ParseError> {
        let opts = TreeBuilderOpts {
            drop_doctype: true,
            ..Default::default()
        };

        let dom = parse_document(RcDom::default(), opts)
            .from_utf8()
            .read_from(&mut html.as_bytes())
            .map_err(|e| ParseError::HtmlError(e.to_string()))?;

        Self::convert_node(&dom.document)
    }

    fn convert_node(handle: &Handle) -> Result<Node, ParseError> {
        let node = handle.borrow();

        match &node.data {
            NodeData::Document => {
                let children = node.children.borrow();
                let converted: Result<Vec<_>, _> = children
                    .iter()
                    .filter_map(|child| Self::convert_node(child).ok())
                    .collect();
                Ok(Node::Document { children: converted? })
            }

            NodeData::Element { name, attrs, .. } => {
                let tag = name.local.as_ref();
                let attrs = attrs.borrow();
                let children = node.children.borrow();

                match tag {
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let level = tag.chars().nth(1).unwrap().to_digit(10).unwrap() as u8;
                        let id = attrs.iter()
                            .find(|a| a.name.local.as_ref() == "id")
                            .map(|a| a.value.to_string());

                        Ok(Node::Heading {
                            level,
                            id,
                            children: Self::convert_children(&children)?,
                        })
                    }

                    "p" => Ok(Node::Paragraph {
                        children: Self::convert_children(&children)?,
                    }),

                    "pre" => {
                        // Look for code child
                        // Extract language from class
                        // ...
                    }

                    "a" => {
                        let href = attrs.iter()
                            .find(|a| a.name.local.as_ref() == "href")
                            .map(|a| a.value.to_string())
                            .unwrap_or_default();

                        Ok(Node::Link {
                            href,
                            title: None,
                            children: Self::convert_children(&children)?,
                        })
                    }

                    // ... more element handlers
                    _ => Ok(Node::Container {
                        class: None,
                        children: Self::convert_children(&children)?,
                    })
                }
            }

            NodeData::Text { contents } => {
                Ok(Node::Text {
                    content: contents.borrow().to_string(),
                })
            }

            _ => Ok(Node::Text { content: String::new() }),
        }
    }

    fn convert_children(children: &[Handle]) -> Result<Vec<Node>, ParseError> {
        children
            .iter()
            .map(Self::convert_node)
            .collect()
    }
}
```

## Normalization Pipeline

```rust
// normalize.rs
use super::ast::Node;
use super::transforms::*;

/// Pipeline of transforms to normalize parsed HTML
pub struct NormalizationPipeline {
    transforms: Vec<Box<dyn Transform>>,
    base_url: String,
}

/// A single transformation step
pub trait Transform: Send + Sync {
    fn transform(&self, node: Node) -> Node;
}

impl NormalizationPipeline {
    pub fn new(base_url: &str) -> Self {
        Self {
            transforms: vec![
                Box::new(StripNavigation),
                Box::new(NormalizeHeadings),
                Box::new(ResolveLinks::new(base_url)),
                Box::new(ProcessCodeBlocks),
                Box::new(CleanWhitespace),
            ],
            base_url: base_url.to_string(),
        }
    }

    /// Run all transforms on the AST
    pub fn normalize(&self, mut node: Node) -> Node {
        for transform in &self.transforms {
            node = transform.transform(node);
        }
        node
    }
}
```

## Testing Pattern

```rust
// tests.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_html() {
        let html = r#"
            <html>
            <body>
                <h1>Title</h1>
                <p>Hello <strong>world</strong>!</p>
            </body>
            </html>
        "#;

        let ast = HtmlParser::parse(html).unwrap();

        // Verify structure
        if let Node::Document { children } = ast {
            // Find heading
            let heading = children.iter().find(|n| matches!(n, Node::Heading { .. }));
            assert!(heading.is_some());
        } else {
            panic!("Expected document node");
        }
    }

    #[test]
    fn test_normalize_resolves_links() {
        let html = r#"<a href="/docs/next">Next</a>"#;
        let ast = HtmlParser::parse(html).unwrap();
        let pipeline = NormalizationPipeline::new("https://example.com");

        let normalized = pipeline.normalize(ast);

        // Verify link is resolved to absolute URL
        // ...
    }

    #[test]
    fn test_extract_toc() {
        let html = r#"
            <h1 id="intro">Introduction</h1>
            <h2 id="setup">Setup</h2>
            <h2 id="usage">Usage</h2>
            <h3 id="basic">Basic</h3>
        "#;

        let ast = HtmlParser::parse(html).unwrap();
        let toc = extract_toc(&ast);

        assert_eq!(toc.len(), 1); // One h1
        assert_eq!(toc[0].children.len(), 2); // Two h2s under it
    }
}
```

## Architectural Rules

1. Parser **cannot import from** `commands/`, `scraper/`, `search/`, `sync/`
2. Parser **can import from** `config/`, `error.rs`
3. Parser must **never panic** on malformed input
4. Keep AST **serializable** for potential caching
5. Transforms must be **pure** and **stateless**
6. Use **proper benchmarks** for performance-critical code
