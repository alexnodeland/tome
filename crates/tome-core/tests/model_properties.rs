//! Property tests over the model's validated scalars and the AST (S1-1).
//!
//! The scalar properties are the containment story: whatever a config file
//! or a crawler produces, a value that *constructs* is a value that cannot
//! traverse. The AST properties pin that serde round-trips losslessly for
//! arbitrary trees — the freeze test checks one handmade tree, these check
//! trees nobody would write by hand.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use proptest::prelude::*;
use tome_core::model::{ListItem, Node, PagePath, SourceId};

/// Same grammar as `SourceId`'s documented rules; if one changes, change the
/// other (and `paths_properties.rs` uses the same generator for its ids).
fn valid_source_id() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9][a-zA-Z0-9._+-]{0,63}"
}

fn valid_page_path() -> impl Strategy<Value = String> {
    // Segments avoid ".", "..", empty — built from a charset that cannot
    // produce them with dots stripped from the leading position.
    proptest::collection::vec("[a-zA-Z0-9_-][a-zA-Z0-9._-]{0,12}", 1..6)
        .prop_map(|segs| segs.join("/"))
        .prop_filter("no dot segments", |p| {
            p.split('/').all(|s| s != "." && s != "..")
        })
}

proptest! {
    #[test]
    fn accepted_source_ids_contain_no_separators(id in valid_source_id()) {
        let parsed = SourceId::new(id.clone()).expect("grammar-valid id must construct");
        prop_assert!(!parsed.as_str().contains('/'));
        prop_assert!(!parsed.as_str().contains('\\'));
        prop_assert!(!parsed.as_str().contains('\0'));
        prop_assert_ne!(parsed.as_str(), "..");
        prop_assert!(!parsed.as_str().starts_with('.'));
    }

    #[test]
    fn arbitrary_strings_never_panic_source_id(s in ".*") {
        // Outcome is Ok or Err — never a panic, whatever the input.
        let _ = SourceId::new(s);
    }

    #[test]
    fn accepted_page_paths_have_no_traversal(p in valid_page_path()) {
        let parsed = PagePath::new(p).expect("grammar-valid path must construct");
        prop_assert!(!parsed.as_str().starts_with('/'));
        prop_assert!(parsed.as_str().split('/').all(|s| s != ".." && s != "." && !s.is_empty()));
    }

    #[test]
    fn arbitrary_strings_never_panic_page_path(s in ".*") {
        let _ = PagePath::new(s);
    }

    #[test]
    fn any_string_with_traversal_segment_is_rejected(
        prefix in "[a-z]{0,8}",
        suffix in "[a-z]{0,8}",
    ) {
        // "..", "a/..", "../a", "a/../b" — every placement rejects.
        let mut candidates = vec!["..".to_owned()];
        if !prefix.is_empty() {
            candidates.push(format!("{prefix}/.."));
        }
        if !suffix.is_empty() {
            candidates.push(format!("../{suffix}"));
        }
        if !prefix.is_empty() && !suffix.is_empty() {
            candidates.push(format!("{prefix}/../{suffix}"));
        }
        for c in candidates {
            prop_assert!(PagePath::new(c).is_err());
        }
    }
}

/// An arbitrary AST, depth-bounded. Inline and block kinds mixed freely —
/// the *renderer* cares about nesting discipline, serde must not.
fn arb_node() -> impl Strategy<Value = Node> {
    let leaf = prop_oneof![
        "[ -~]{0,20}".prop_map(|value| Node::Text { value }),
        "[ -~]{0,20}".prop_map(|code| Node::InlineCode { code }),
        (proptest::option::of("[a-z]{1,8}"), "[ -~]{0,40}")
            .prop_map(|(language, code)| Node::CodeBlock { language, code }),
        ("[a-z/.]{1,20}", "[ -~]{0,20}").prop_map(|(src, alt)| Node::Image { src, alt }),
        "[a-z-]{1,12}".prop_map(|id| Node::Anchor { id }),
        Just(Node::ThematicBreak {}),
        Just(Node::LineBreak {}),
    ];
    leaf.prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..4)
                .prop_map(|children| Node::Paragraph { children }),
            (
                1u8..=6,
                proptest::option::of("[a-z-]{1,10}"),
                proptest::collection::vec(inner.clone(), 0..3)
            )
                .prop_map(|(level, id, children)| Node::Heading {
                    level,
                    id,
                    children
                }),
            (
                any::<bool>(),
                proptest::option::of(any::<u32>()),
                proptest::collection::vec(
                    proptest::collection::vec(inner.clone(), 0..3)
                        .prop_map(|children| ListItem { children }),
                    0..3
                )
            )
                .prop_map(|(ordered, start, items)| Node::List {
                    ordered,
                    start,
                    items
                }),
            proptest::collection::vec(inner, 0..3).prop_map(|children| Node::Document { children }),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn ast_serde_round_trips(node in arb_node()) {
        let encoded = serde_json::to_string(&node).expect("AST must serialize");
        let decoded: Node = serde_json::from_str(&encoded).expect("frozen shape must parse");
        prop_assert_eq!(decoded, node);
    }

    #[test]
    fn text_content_never_panics(node in arb_node()) {
        let _ = node.text_content();
    }
}
