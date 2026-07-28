//! The model's validated scalars must not panic on any input.
//!
//! `SourceId` and `PagePath` are constructed from config files, registry
//! entries, and crawler output — all attacker-influenced. Their job is to
//! return `Err` on hostile input; a panic instead turns validation into a
//! denial of service. Beyond "no panic", this asserts the containment
//! invariants the types exist for: whatever constructs cannot traverse.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tome_core::model::{ContentHash, PagePath, SourceId};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    if let Ok(id) = SourceId::new(text) {
        assert!(!id.as_str().contains('/'));
        assert!(!id.as_str().contains('\\'));
        assert!(!id.as_str().contains('\0'));
        assert!(!id.as_str().starts_with('.'));
        assert!(!id.as_str().is_empty());
    }

    if let Ok(path) = PagePath::new(text) {
        assert!(!path.as_str().starts_with('/'));
        assert!(path
            .as_str()
            .split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != ".."));
        assert!(!path.as_str().contains('\\'));
        assert!(!path.as_str().contains('\0'));
    }

    let _ = ContentHash::new(text);
});
