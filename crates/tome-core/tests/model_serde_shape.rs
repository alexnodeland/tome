//! The serde shape of the model IS the S1-1 freeze, made falsifiable.
//!
//! These tests pin exact JSON. If an edit to `model/` fails one, that edit
//! changed the wire/storage format: stored ASTs, IPC payloads, and API
//! responses built on the old shape stop parsing. That is sometimes the
//! right call — with a migration and a version bump — but it must never be
//! an accident. Additive changes (a new variant, a new `Option` field with
//! `skip_serializing_if` semantics considered) pass these tests untouched.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use chrono::{DateTime, Utc};
use serde_json::json;
use tome_core::model::{
    Attribution, ContentHash, Definition, DocPage, DocSet, Icon, ListItem, Node, Page, PagePath,
    Schedule, Source, SourceId, SourceType, SyncConfig, SyncStrategy, TableCell, TableRow,
    TocEntry,
};

fn fixed_time() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-07-28T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

#[test]
fn node_shape_is_frozen() {
    let doc = Node::Document {
        children: vec![
            Node::Heading {
                level: 1,
                id: Some("top".into()),
                children: vec![Node::Text {
                    value: "Title".into(),
                }],
            },
            Node::Paragraph {
                children: vec![
                    Node::Text {
                        value: "See ".into(),
                    },
                    Node::Link {
                        href: "api.html".into(),
                        title: None,
                        children: vec![Node::InlineCode { code: "api".into() }],
                    },
                    Node::Anchor { id: "mid".into() },
                ],
            },
            Node::CodeBlock {
                language: Some("rust".into()),
                code: "fn main() {}".into(),
            },
            Node::List {
                ordered: true,
                start: Some(4),
                items: vec![ListItem {
                    children: vec![Node::Text { value: "x".into() }],
                }],
            },
            Node::DefinitionList {
                items: vec![Definition {
                    id: Some("os.path".into()),
                    term: vec![Node::Text {
                        value: "os.path".into(),
                    }],
                    definition: vec![Node::Text {
                        value: "Path tools.".into(),
                    }],
                }],
            },
            Node::Table {
                headers: vec![TableCell {
                    children: vec![Node::Text { value: "h".into() }],
                }],
                rows: vec![TableRow {
                    cells: vec![TableCell {
                        children: vec![Node::Text { value: "c".into() }],
                    }],
                }],
            },
            Node::Admonition {
                kind: "warning".into(),
                title: None,
                children: vec![Node::Paragraph {
                    children: vec![Node::Strong {
                        children: vec![Node::Text {
                            value: "Careful.".into(),
                        }],
                    }],
                }],
            },
            Node::Image {
                src: "img/x.png".into(),
                alt: "diagram".into(),
            },
            Node::ThematicBreak {},
            Node::Blockquote {
                children: vec![Node::Paragraph {
                    children: vec![
                        Node::Emphasis {
                            children: vec![Node::Text { value: "q".into() }],
                        },
                        Node::LineBreak {},
                    ],
                }],
            },
        ],
    };

    let expected = json!({
        "type": "document",
        "children": [
            { "type": "heading", "level": 1, "id": "top",
              "children": [ { "type": "text", "value": "Title" } ] },
            { "type": "paragraph", "children": [
                { "type": "text", "value": "See " },
                { "type": "link", "href": "api.html", "title": null,
                  "children": [ { "type": "inline_code", "code": "api" } ] },
                { "type": "anchor", "id": "mid" }
            ] },
            { "type": "code_block", "language": "rust", "code": "fn main() {}" },
            { "type": "list", "ordered": true, "start": 4,
              "items": [ { "children": [ { "type": "text", "value": "x" } ] } ] },
            { "type": "definition_list", "items": [
                { "id": "os.path",
                  "term": [ { "type": "text", "value": "os.path" } ],
                  "definition": [ { "type": "text", "value": "Path tools." } ] }
            ] },
            { "type": "table",
              "headers": [ { "children": [ { "type": "text", "value": "h" } ] } ],
              "rows": [ { "cells": [ { "children": [ { "type": "text", "value": "c" } ] } ] } ] },
            { "type": "admonition", "kind": "warning", "title": null, "children": [
                { "type": "paragraph", "children": [
                    { "type": "strong", "children": [ { "type": "text", "value": "Careful." } ] }
                ] }
            ] },
            { "type": "image", "src": "img/x.png", "alt": "diagram" },
            { "type": "thematic_break" },
            { "type": "blockquote", "children": [
                { "type": "paragraph", "children": [
                    { "type": "emphasis", "children": [ { "type": "text", "value": "q" } ] },
                    { "type": "line_break" }
                ] }
            ] }
        ]
    });

    assert_eq!(serde_json::to_value(&doc).unwrap(), expected);
    // And back: the frozen shape parses to the same tree.
    let back: Node = serde_json::from_value(expected).unwrap();
    assert_eq!(back, doc);
}

#[test]
fn source_shape_is_frozen() {
    let mut source = Source::new(
        SourceId::new("python").unwrap(),
        "Python 3.13",
        SourceType::ReadTheDocs,
    );
    source.url = Some("https://docs.python.org/3/".parse().unwrap());
    source.version = Some("3.13".into());
    source.icon = Some(Icon::Emoji("🐍".into()));
    source.attribution = Attribution {
        homepage: Some("https://www.python.org/".parse().unwrap()),
        licence: Some("PSF-2.0".into()),
    };
    source.sync = SyncConfig {
        strategy: SyncStrategy::Scheduled {
            schedule: Schedule::Weekly,
        },
        pin_version: false,
    };
    source.created_at = fixed_time();

    let expected = json!({
        "id": "python",
        "name": "Python 3.13",
        "kind": "readthedocs",
        "url": "https://docs.python.org/3/",
        "local_path": null,
        "version": "3.13",
        "category": "Uncategorized",
        "icon": { "emoji": "🐍" },
        "accent_color": null,
        "attribution": { "homepage": "https://www.python.org/", "licence": "PSF-2.0" },
        "sync": { "strategy": { "strategy": "scheduled", "schedule": "weekly" },
                  "pin_version": false },
        "created_at": "2026-07-28T12:00:00Z",
        "last_synced": null,
        "page_count": 0,
        "index_size_bytes": 0
    });

    assert_eq!(serde_json::to_value(&source).unwrap(), expected);
    let back: Source = serde_json::from_value(expected).unwrap();
    assert_eq!(back, source);
}

#[test]
fn page_and_docset_shape_is_frozen() {
    let mut page = Page::new(
        SourceId::new("python").unwrap(),
        PagePath::new("library/os.html").unwrap(),
        "os — OS interfaces",
        ContentHash::from_digest([0x11; 32]),
    );
    page.fetched_at = fixed_time();
    page.etag = Some("\"abc\"".into());

    let docset = DocSet::new(
        vec![DocPage {
            meta: page,
            body: Node::Document { children: vec![] },
        }],
        vec![TocEntry::new(
            "Library",
            Some(PagePath::new("library/index.html").unwrap()),
        )],
    );

    let expected = json!({
        "pages": [ {
            "meta": {
                "source": "python",
                "path": "library/os.html",
                "title": "os — OS interfaces",
                "content_hash": "11".repeat(32),
                "fetched_at": "2026-07-28T12:00:00Z",
                "etag": "\"abc\"",
                "last_modified": null
            },
            "body": { "type": "document", "children": [] }
        } ],
        "toc": [ {
            "title": "Library",
            "path": "library/index.html",
            "fragment": null,
            "children": []
        } ]
    });

    assert_eq!(serde_json::to_value(&docset).unwrap(), expected);
    let back: DocSet = serde_json::from_value(expected).unwrap();
    assert_eq!(back, docset);
}
