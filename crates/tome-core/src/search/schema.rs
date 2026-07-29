//! The search index schema (P2-002).
//!
//! One function builds it and one struct holds the field handles, because a
//! schema whose field names are looked up by string at each call site is a
//! schema that drifts: `get_field("body")` compiles fine and fails at runtime.
//! [`Fields`] is resolved once, when the index is opened, and every read and
//! write goes through it.
//!
//! # Stored versus indexed
//!
//! Only what a result list must display is stored: `source_id`, `path`,
//! `title`. `headers`, `body`, and `code` are indexed and **not** stored.
//!
//! That is a deliberate departure from the obvious approach, and it costs
//! nothing here for a specific reason: Tome already keeps every page's AST on
//! disk in the [`crate::store::PageStore`]. Storing the body in Tantivy as
//! well would roughly double the index — SPIKE-003's measured 224 MB per
//! 100 000 pages assumes this schema — to hold a second copy of bytes we can
//! already read, keyed by the very `source_id`/`path` pair the hit carries.
//!
//! The consequence to know about: **snippet generation cannot use Tantivy's
//! `SnippetGenerator`**, which requires a stored field. Result snippets
//! (P2-005, in S2-7) must re-read the page from the `PageStore` and highlight
//! there. That is the right place anyway — the store holds structured nodes,
//! so a snippet can respect block boundaries instead of slicing raw text.

use tantivy::schema::{
    Field, Schema, TextFieldIndexing, TextOptions, FAST, INDEXED, STORED, STRING, TEXT,
};

/// Name of the code-aware tokenizer, registered on the index's tokenizer
/// manager by [`super::SearchEngine`].
///
/// It must be registered on the `Index` itself rather than only where
/// documents are written: the `QueryParser` resolves tokenizers through the
/// same manager, and a query analysed differently from the documents silently
/// returns nothing.
pub const CODE_TOKENIZER: &str = "code";

/// Field handles for [`build`]'s schema.
///
/// `Field` is a `u32` index into the schema; copying is free.
#[derive(Debug, Clone, Copy)]
pub struct Fields {
    /// Owning source, exact-match. Filtering by source (P2-016) and deleting
    /// a source's documents both key on this, so it is `STRING` — one token,
    /// unanalysed — not `TEXT`.
    pub source_id: Field,
    /// Page path within its source. With `source_id`, the page's identity.
    pub path: Field,
    pub title: Field,
    /// Multi-valued: one value per heading in the page. Tantivy indexes
    /// repeated values of one field as a single stream, so a phrase cannot
    /// straddle two headings only because a large position gap separates them.
    pub headers: Field,
    pub body: Field,
    /// Indexed with [`CODE_TOKENIZER`] rather than the default one.
    pub code: Field,
    /// Hierarchical facet (`/Python/Standard Library`) for category
    /// filtering.
    pub category: Field,
    /// The name of the symbol this page is a reference page *for* (S2-6) —
    /// `Vec` on `struct.Vec.html`, and nothing at all on a tutorial.
    ///
    /// One term per page, and that is the point. This is the field the ordinary
    /// ranking blends, because a match in it means "this page is about the
    /// thing you asked for", which is unambiguous in a way no other field is.
    pub symbol: Field,
    /// Every symbol the page declares, including the primary one.
    ///
    /// **Not searched by ordinary queries** — only by `@symbol` (P2-015).
    /// Measured: blending it made relevance markedly worse, because a
    /// reference page declares `from`, `into`, `borrow`, `fmt` and `try_from`
    /// as trait boilerplate, and a short field makes each of those a strong
    /// BM25 signal. As an explicit filter it is exactly right — someone typing
    /// `@borrow` wants the pages that declare `borrow` — and as an implicit
    /// boost it is noise. See `Ranking::symbols`.
    pub declarations: Field,
    /// What the page's primary symbol *is* — `function`, `type`, `module` —
    /// for P2-015's "symbol type in results".
    ///
    /// **Stored but not indexed**, and only the *primary* symbol's kind, not
    /// every symbol's. One short string per page rather than a list that grows
    /// with the page: a result names one thing, and SPIKE-003 finding 2 makes
    /// anything per-symbol a running cost. Indexing it would be nearly free
    /// (six distinct values), and is deliberately not done until something
    /// wants to filter by it — see P2-016.
    pub symbol_kind: Field,
    /// The page's content hash, for incremental indexing (S2-3).
    ///
    /// **Stored but deliberately NOT indexed.** Nothing ever queries a hash by
    /// term, and SPIKE-003 finding 2 is that index size scales with
    /// vocabulary: 100 000 unique hashes would be 100 000 term-dictionary
    /// entries bought for nothing. The sync path reads it from the stored
    /// document instead.
    pub content_hash: Field,
}

/// Build the schema and resolve its field handles.
pub fn build() -> (Schema, Fields) {
    let mut builder = Schema::builder();

    // `code` is the only field with a non-default tokenizer. It is indexed
    // with positions so that phrase queries over code work -- `TEXT`'s default
    // indexing option, spelled out here because the tokenizer override
    // replaces the whole `TextFieldIndexing`, and omitting the record option
    // would silently downgrade to frequency-only and break phrase search.
    let code_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer(CODE_TOKENIZER)
            .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions),
    );

    // The symbol fields share the code tokenizer so that `with_capacity` is
    // findable as `capacity` and `os.cpus` as `cpus`, the same way `code` is.
    let symbol_options = code_options.clone();

    let fields = Fields {
        source_id: builder.add_text_field("source_id", STRING | STORED | FAST),
        path: builder.add_text_field("path", STRING | STORED),
        title: builder.add_text_field("title", TEXT | STORED),
        headers: builder.add_text_field("headers", TEXT),
        body: builder.add_text_field("body", TEXT),
        code: builder.add_text_field("code", code_options),
        category: builder.add_facet_field("category", INDEXED),
        symbol: builder.add_text_field("symbol", symbol_options.clone()),
        declarations: builder.add_text_field("declarations", symbol_options),
        symbol_kind: builder.add_text_field("symbol_kind", STORED),
        content_hash: builder.add_text_field("content_hash", STORED),
    };

    (builder.build(), fields)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn every_field_resolves_by_name() {
        // Guards the drift this module exists to prevent: if a field is
        // renamed in `build` without updating `Fields`, this fails rather
        // than surfacing as an empty result set at runtime.
        let (schema, fields) = build();
        for (name, field) in [
            ("source_id", fields.source_id),
            ("path", fields.path),
            ("title", fields.title),
            ("headers", fields.headers),
            ("body", fields.body),
            ("code", fields.code),
            ("category", fields.category),
            ("symbol", fields.symbol),
            ("declarations", fields.declarations),
            ("symbol_kind", fields.symbol_kind),
            ("content_hash", fields.content_hash),
        ] {
            assert_eq!(schema.get_field(name).expect("field present"), field);
        }
    }

    #[test]
    fn only_result_display_fields_are_stored() {
        // The index-size budget in SPIKE-003 assumes this. Storing `body`
        // would roughly double the index to hold a copy of what PageStore
        // already has.
        let (schema, _) = build();
        let stored: Vec<_> = schema
            .fields()
            .filter(|(_, entry)| entry.is_stored())
            .map(|(_, entry)| entry.name().to_owned())
            .collect();
        assert_eq!(
            stored,
            vec!["source_id", "path", "title", "symbol_kind", "content_hash"],
            "only result-display fields and the sync key are stored"
        );
    }

    #[test]
    fn content_hash_is_stored_but_not_indexed() {
        // A hash is never queried by term, and 100k unique hashes in the term
        // dictionary is exactly the vocabulary growth SPIKE-003 measured as
        // the driver of index size.
        let (schema, fields) = build();
        let entry = schema.get_field_entry(fields.content_hash);
        assert!(entry.is_stored());
        assert!(
            !entry.is_indexed(),
            "content_hash must not enter the term dictionary"
        );
    }

    #[test]
    fn code_field_keeps_positions() {
        // A tokenizer override replaces the whole TextFieldIndexing. If the
        // record option is dropped, phrase queries over code stop matching
        // and nothing else fails.
        let (schema, fields) = build();
        let entry = schema.get_field_entry(fields.code);
        let indexing = match entry.field_type() {
            tantivy::schema::FieldType::Str(options) => {
                options.get_indexing_options().expect("code is indexed")
            }
            other => panic!("code should be a text field, got {other:?}"),
        };
        assert_eq!(indexing.tokenizer(), CODE_TOKENIZER);
        assert_eq!(
            indexing.index_option(),
            tantivy::schema::IndexRecordOption::WithFreqsAndPositions
        );
    }
}
