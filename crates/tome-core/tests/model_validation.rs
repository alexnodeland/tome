//! Validation behaviour of the model's scalar types (S1-1).
//!
//! `SourceId` and `PagePath` end up joined into filesystem paths under the
//! cache root, so the rejections here are containment guarantees. Every
//! rejection asserts on the *variant*, not the message text — messages are
//! user-facing copy and may be reworded.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use tome_core::model::{ContentHash, PagePath, SourceId};
use tome_core::Error;

// ---- SourceId ---------------------------------------------------------

#[test]
fn source_id_accepts_the_ordinary() {
    for id in [
        "python",
        "python-3.13",
        "rust_std",
        "c++",
        "A",
        "9p",
        "a.b-c_d+e",
    ] {
        assert!(SourceId::new(id).is_ok(), "should accept {id:?}");
    }
}

#[test]
fn source_id_accepts_max_length_and_rejects_beyond() {
    assert!(SourceId::new("a".repeat(64)).is_ok());
    assert!(matches!(
        SourceId::new("a".repeat(65)),
        Err(Error::InvalidSourceId { .. })
    ));
}

#[test]
fn source_id_rejects_hostile_and_malformed() {
    let cases = [
        "",        // empty
        "../etc",  // traversal
        "..",      // bare traversal — leading char must be alphanumeric
        ".hidden", // leading dot
        "-flag",   // leading dash: becomes an argv footgun in CLI use
        "a/b",     // separator
        "a\\b",    // separator, other spelling
        "a\0b",    // NUL
        "a b",     // whitespace
        "café",    // non-ASCII
        "~root",   // shell expansion
    ];
    for id in cases {
        assert!(
            matches!(SourceId::new(id), Err(Error::InvalidSourceId { .. })),
            "should reject {id:?}"
        );
    }
}

#[test]
fn source_id_serde_revalidates() {
    // Deserialize goes through the same validation as `new` — a hostile id
    // cannot enter through a JSON side door.
    let ok: Result<SourceId, _> = serde_json::from_str("\"python\"");
    assert_eq!(ok.unwrap().as_str(), "python");
    let bad: Result<SourceId, _> = serde_json::from_str("\"../etc\"");
    assert!(bad.is_err());
}

// ---- PagePath ---------------------------------------------------------

#[test]
fn page_path_accepts_the_ordinary() {
    for p in [
        "index.html",
        "api/reference.html",
        "guide/ch01/intro.html",
        "3.13/library/os.html",
        "man1/ls.1.html",
    ] {
        assert!(PagePath::new(p).is_ok(), "should accept {p:?}");
    }
}

#[test]
fn page_path_rejects_hostile_and_malformed() {
    let cases = [
        "",                // empty
        "/etc/passwd",     // absolute
        "../secrets.html", // traversal
        "a/../b.html",     // interior traversal
        "a/./b.html",      // dot segment: silent normalization forks identities
        "a//b.html",       // empty segment: same reason
        "a\\b.html",       // backslash
        "a\0b.html",       // NUL
        "a/..",            // trailing traversal
    ];
    for p in cases {
        assert!(
            matches!(PagePath::new(p), Err(Error::InvalidPagePath { .. })),
            "should reject {p:?}"
        );
    }
}

#[test]
fn page_path_rejects_oversized() {
    let long = format!("a/{}.html", "b".repeat(PagePath::MAX_LEN));
    assert!(matches!(
        PagePath::new(long),
        Err(Error::InvalidPagePath { .. })
    ));
}

#[test]
fn page_path_serde_revalidates() {
    let bad: Result<PagePath, _> = serde_json::from_str("\"../x\"");
    assert!(bad.is_err());
}

// ---- ContentHash ------------------------------------------------------

#[test]
fn content_hash_accepts_lowercase_hex_only() {
    let hex = "a".repeat(64);
    assert!(ContentHash::new(hex).is_ok());

    for bad in [
        "A".repeat(64), // uppercase: two spellings of one hash defeat equality
        "a".repeat(63), // short
        "a".repeat(65), // long
        "g".repeat(64), // not hex
        String::new(),  // empty
    ] {
        assert!(
            matches!(
                ContentHash::new(bad.clone()),
                Err(Error::InvalidContentHash)
            ),
            "should reject {bad:?}"
        );
    }
}

#[test]
fn content_hash_from_digest_round_trips() {
    let digest = [0xABu8; 32];
    let hash = ContentHash::from_digest(digest);
    assert_eq!(hash.as_str().len(), 64);
    assert_eq!(hash.as_str(), "ab".repeat(32));
    // And the lowercase form it produced re-validates.
    assert_eq!(ContentHash::new(hash.as_str()).unwrap(), hash);
}
