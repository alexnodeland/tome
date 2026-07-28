//! The sanitizer must never let a script-capable URL or an unsafe id token
//! reach its output, on any input.
//!
//! Feeds arbitrary bytes as HTML through parse → sanitize and asserts the
//! output's invariants: no link href or image src carries a script-capable
//! scheme (javascript/data/vbscript/file), every id is a safe attribute
//! token, every admonition kind is a safe class token, and the whole thing
//! still round-trips through the frozen serde shape. "No input produces
//! script-capable markup" is the property the fuzz README names for this
//! target.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tome_core::model::Node;
use tome_core::parse::parse_page;
use tome_core::sanitize::sanitize;

fn scheme(url: &str) -> String {
    // Mirror what a browser sees: strip leading controls/space, take up to
    // the first ':'.
    let trimmed = url.trim_start_matches(|c: char| c.is_control() || c == ' ' || c == '\t');
    match trimmed.split_once(':') {
        Some((s, _)) if !s.contains(['/', '?', '#']) => s.to_ascii_lowercase(),
        _ => String::new(),
    }
}

fn check(node: &Node) {
    match node {
        Node::Link { href, .. } => {
            let s = scheme(href);
            assert!(
                !matches!(s.as_str(), "javascript" | "data" | "vbscript" | "file"),
                "link href survived with scheme {s:?}: {href:?}"
            );
        }
        Node::Image { src, .. } => {
            let s = scheme(src);
            assert!(
                !matches!(s.as_str(), "javascript" | "data" | "vbscript" | "file"),
                "image src survived with scheme {s:?}: {src:?}"
            );
        }
        Node::Heading { id: Some(id), .. } | Node::Anchor { id } => assert_safe_id(id),
        Node::DefinitionList { items } => {
            for d in items {
                if let Some(id) = &d.id {
                    assert_safe_id(id);
                }
            }
        }
        Node::Admonition { kind, .. } => assert!(
            kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !kind.is_empty(),
            "admonition kind not a safe class token: {kind:?}"
        ),
        _ => {}
    }
    for child in child_nodes(node) {
        check(child);
    }
}

fn assert_safe_id(id: &str) {
    // The sanitizer keeps unicode and most punctuation (rustdoc/Sphinx ids
    // need it) and removes only what can break out of, or split, an
    // attribute: quotes, whitespace, control characters.
    assert!(
        !id.chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '`')),
        "id contains an attribute-breaking character: {id:?}"
    );
}

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

fuzz_target!(|data: &[u8]| {
    let Ok(html) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(base) = url::Url::parse("https://fuzz.test/dir/page.html") else {
        return;
    };
    let parsed = parse_page(html, &base, None);
    let clean = sanitize(parsed.body);

    check(&clean);

    // Sanitizing is idempotent, and the result still fits the frozen shape.
    assert_eq!(sanitize(clean.clone()), clean, "sanitize is not idempotent");
    let json = serde_json::to_string(&clean).expect("serializes");
    let _: Node = serde_json::from_str(&json).expect("round-trips");
});
