# Search Directory

Full-text search using Tantivy.

## What Belongs Here

- **Tantivy index** configuration and management
- **Search queries** and result ranking
- **Indexing logic** for adding/updating documents
- **Search result types** and formatting

## What Does NOT Belong Here

- HTML parsing (use `parser/`)
- Database operations (use `storage/`)
- HTTP fetching (use `scraper/`)
- Command handlers (use `commands/`)

## Module Structure

```
search/
├── mod.rs              # Module exports
├── engine.rs           # Main search engine
├── schema.rs           # Tantivy schema definition
├── indexer.rs          # Document indexing
├── query.rs            # Query parsing and execution
├── ranking.rs          # Custom ranking/scoring
└── tests.rs            # Unit tests
```

## Schema Definition

```rust
// schema.rs
use tantivy::schema::{Schema, STORED, TEXT, STRING, Field};

pub struct SearchSchema {
    pub schema: Schema,
    pub source_id: Field,
    pub page_path: Field,
    pub title: Field,
    pub content: Field,
    pub symbols: Field,
}

impl SearchSchema {
    pub fn new() -> Self {
        let mut builder = Schema::builder();

        // Source and path for identification (not analyzed)
        let source_id = builder.add_text_field("source_id", STRING | STORED);
        let page_path = builder.add_text_field("page_path", STRING | STORED);

        // Title - analyzed for search, stored for display
        let title = builder.add_text_field("title", TEXT | STORED);

        // Main content - analyzed for search
        let content = builder.add_text_field("content", TEXT);

        // Symbols - special handling for code identifiers
        // Uses custom tokenizer that preserves camelCase, snake_case
        let symbols = builder.add_text_field("symbols", TEXT);

        let schema = builder.build();

        Self {
            schema,
            source_id,
            page_path,
            title,
            content,
            symbols,
        }
    }
}

impl Default for SearchSchema {
    fn default() -> Self {
        Self::new()
    }
}
```

## Search Engine

```rust
// engine.rs
use tantivy::{
    Index, IndexWriter, IndexReader, ReloadPolicy,
    query::QueryParser, collector::TopDocs,
    Document, Term,
};
use std::path::Path;
use crate::error::SearchError;
use super::{schema::SearchSchema, query::SearchQuery, SearchResult};

pub struct SearchEngine {
    index: Index,
    schema: SearchSchema,
    reader: IndexReader,
}

impl SearchEngine {
    /// Open or create search index at path
    pub fn open(index_path: &Path) -> Result<Self, SearchError> {
        let schema = SearchSchema::new();

        let index = if index_path.exists() {
            Index::open_in_dir(index_path)?
        } else {
            std::fs::create_dir_all(index_path)?;
            Index::create_in_dir(index_path, schema.schema.clone())?
        };

        // Register custom tokenizers
        Self::register_tokenizers(&index)?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommit)
            .try_into()?;

        Ok(Self { index, schema, reader })
    }

    /// Create in-memory index (for testing)
    pub fn in_memory() -> Result<Self, SearchError> {
        let schema = SearchSchema::new();
        let index = Index::create_in_ram(schema.schema.clone());

        Self::register_tokenizers(&index)?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommit)
            .try_into()?;

        Ok(Self { index, schema, reader })
    }

    fn register_tokenizers(index: &Index) -> Result<(), SearchError> {
        // Register symbol tokenizer for code identifiers
        // This preserves camelCase and snake_case boundaries
        // ...
        Ok(())
    }

    /// Get writer for indexing
    pub fn writer(&self) -> Result<IndexWriter, SearchError> {
        // 50MB heap for indexing
        self.index.writer(50_000_000).map_err(Into::into)
    }

    /// Execute a search query
    pub fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>, SearchError> {
        let searcher = self.reader.searcher();

        // Build query
        let mut query_parser = QueryParser::for_index(
            &self.index,
            vec![self.schema.title, self.schema.content, self.schema.symbols],
        );

        // Enable fuzzy matching
        query_parser.set_field_fuzzy(self.schema.title, true, 1, true);
        query_parser.set_field_fuzzy(self.schema.content, true, 1, true);

        let tantivy_query = query_parser.parse_query(&query.query)?;

        // Apply source filter if specified
        let final_query = if let Some(source_ids) = &query.source_ids {
            self.with_source_filter(tantivy_query, source_ids)?
        } else {
            tantivy_query
        };

        // Execute search
        let limit = query.limit.unwrap_or(20);
        let top_docs = searcher.search(&final_query, &TopDocs::with_limit(limit))?;

        // Convert to results
        let mut results = Vec::with_capacity(top_docs.len());
        for (score, doc_address) in top_docs {
            let doc = searcher.doc(doc_address)?;
            results.push(self.doc_to_result(doc, score)?);
        }

        Ok(results)
    }

    fn doc_to_result(&self, doc: Document, score: f32) -> Result<SearchResult, SearchError> {
        let source_id = doc
            .get_first(self.schema.source_id)
            .and_then(|v| v.as_text())
            .ok_or(SearchError::MissingField("source_id"))?
            .to_string();

        let page_path = doc
            .get_first(self.schema.page_path)
            .and_then(|v| v.as_text())
            .ok_or(SearchError::MissingField("page_path"))?
            .to_string();

        let title = doc
            .get_first(self.schema.title)
            .and_then(|v| v.as_text())
            .ok_or(SearchError::MissingField("title"))?
            .to_string();

        Ok(SearchResult {
            source_id,
            page_path,
            title,
            snippet: String::new(), // TODO: Generate snippet
            score,
        })
    }

    /// Remove all documents for a source
    pub fn remove_source(&self, source_id: &str) -> Result<(), SearchError> {
        let mut writer = self.writer()?;
        let term = Term::from_field_text(self.schema.source_id, source_id);
        writer.delete_term(term);
        writer.commit()?;
        Ok(())
    }
}
```

## Indexer

```rust
// indexer.rs
use tantivy::{IndexWriter, Document};
use crate::parser::ast::Node;
use super::SearchSchema;

pub struct Indexer<'a> {
    writer: IndexWriter,
    schema: &'a SearchSchema,
}

impl<'a> Indexer<'a> {
    pub fn new(writer: IndexWriter, schema: &'a SearchSchema) -> Self {
        Self { writer, schema }
    }

    /// Index a single page
    pub fn index_page(
        &mut self,
        source_id: &str,
        page_path: &str,
        title: &str,
        ast: &Node,
    ) -> Result<(), tantivy::TantivyError> {
        let mut doc = Document::new();

        doc.add_text(self.schema.source_id, source_id);
        doc.add_text(self.schema.page_path, page_path);
        doc.add_text(self.schema.title, title);

        // Extract text content from AST
        let content = ast.text_content();
        doc.add_text(self.schema.content, &content);

        // Extract symbols (function names, types, etc.)
        let symbols = extract_symbols(ast);
        doc.add_text(self.schema.symbols, &symbols);

        self.writer.add_document(doc)?;
        Ok(())
    }

    /// Commit all pending changes
    pub fn commit(mut self) -> Result<(), tantivy::TantivyError> {
        self.writer.commit()?;
        Ok(())
    }
}

/// Extract code symbols from AST
fn extract_symbols(node: &Node) -> String {
    let mut symbols = Vec::new();

    fn walk(node: &Node, symbols: &mut Vec<String>) {
        match node {
            Node::CodeBlock { content, .. } | Node::InlineCode { content } => {
                // Extract identifiers from code
                for word in content.split(|c: char| !c.is_alphanumeric() && c != '_') {
                    if !word.is_empty() && word.len() > 2 {
                        symbols.push(word.to_string());
                    }
                }
            }
            Node::Document { children }
            | Node::Heading { children, .. }
            | Node::Paragraph { children }
            | Node::Link { children, .. }
            | Node::Strong { children }
            | Node::Emphasis { children }
            | Node::Container { children, .. } => {
                for child in children {
                    walk(child, symbols);
                }
            }
            _ => {}
        }
    }

    walk(node, &mut symbols);
    symbols.join(" ")
}
```

## Testing Pattern

```rust
// tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::Node;

    #[test]
    fn test_search_by_title() {
        let engine = SearchEngine::in_memory().unwrap();
        let mut indexer = Indexer::new(engine.writer().unwrap(), &engine.schema);

        indexer.index_page(
            "rust-std",
            "/std/vec",
            "Vec - Rust Standard Library",
            &Node::Text { content: "A growable array type".into() },
        ).unwrap();

        indexer.commit().unwrap();

        let results = engine.search(SearchQuery {
            query: "Vec".into(),
            ..Default::default()
        }).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Vec - Rust Standard Library");
    }

    #[test]
    fn test_fuzzy_search() {
        let engine = SearchEngine::in_memory().unwrap();
        let mut indexer = Indexer::new(engine.writer().unwrap(), &engine.schema);

        indexer.index_page(
            "rust-std",
            "/std/iterator",
            "Iterator trait",
            &Node::Text { content: "Methods for iteration".into() },
        ).unwrap();

        indexer.commit().unwrap();

        // Search with typo
        let results = engine.search(SearchQuery {
            query: "itertor".into(), // typo
            ..Default::default()
        }).unwrap();

        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_by_source() {
        let engine = SearchEngine::in_memory().unwrap();
        let mut indexer = Indexer::new(engine.writer().unwrap(), &engine.schema);

        indexer.index_page("rust", "/vec", "Vec", &Node::Text { content: "Rust vec".into() }).unwrap();
        indexer.index_page("python", "/list", "List", &Node::Text { content: "Python list".into() }).unwrap();

        indexer.commit().unwrap();

        let results = engine.search(SearchQuery {
            query: "array".into(),
            source_ids: Some(vec!["rust".into()]),
            ..Default::default()
        }).unwrap();

        // Should only return Rust results
        assert!(results.iter().all(|r| r.source_id == "rust"));
    }
}
```

## Architectural Rules

1. Search **cannot import from** `commands/`, `scraper/`
2. Search **can import from** `parser/`, `config/`, `error.rs`
3. Index operations must be **async-safe** (Tantivy is thread-safe)
4. Use **in-memory index** for all tests
5. Implement **proper snippet generation** with highlighting
6. Consider **index size** - set appropriate memory limits
