//! The HTML → AST parser must not panic on any input.
//!
//! This is P1-012's "zero panics on any input" acceptance criterion, which
//! is otherwise a sentence in a document. html5ever itself never rejects
//! input; the panics this target hunts live in *our* walk — an unwrap on a
//! child that isn't there, an index into a heading name, a dt/dd shape the
//! fixture sites never produce.
//!
//! Invariants asserted beyond "no panic": the root is always a Document,
//! text extraction works on whatever came out, serde round-trips it (the
//! frozen shape has no unrepresentable trees), and no returned link is a
//! non-http(s) scheme.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tome_core::parse::parse_page;

fuzz_target!(|data: &[u8]| {
    let Ok(html) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(base) = url::Url::parse("https://fuzz.test/dir/page.html") else {
        return;
    };

    // Odd-length inputs also exercise the configured-selector path.
    let selector = if data.len() % 2 == 1 { Some("div.body") } else { None };
    let page = parse_page(html, &base, selector);

    assert!(matches!(page.body, tome_core::model::Node::Document { .. }));
    let _ = page.body.text_content();

    let encoded = serde_json::to_string(&page.body).expect("AST serializes");
    let decoded: tome_core::model::Node =
        serde_json::from_str(&encoded).expect("frozen shape parses");
    assert_eq!(decoded, page.body);

    for link in &page.links {
        assert!(matches!(link.scheme(), "http" | "https"));
        assert!(link.fragment().is_none());
    }
});
