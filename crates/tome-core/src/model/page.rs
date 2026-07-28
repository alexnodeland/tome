//! [`Page`] metadata and the two validated scalars it is keyed and
//! change-detected by.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::SourceId;

/// A page's path relative to its source root: `"api/reference.html"`.
///
/// Half of a page's identity (with [`SourceId`]), and — like `SourceId` — a
/// value that ends up joined into filesystem paths under the cache root, so
/// validation here is a containment guarantee, not tidiness:
///
/// - non-empty, at most 2048 bytes, no NUL
/// - no backslash (a crawler that produces one is confused, and on the
///   filesystem it would be a literal character on macOS but a separator in
///   any Windows-adjacent tooling that ever touches an export)
/// - not absolute: no leading `/`
/// - no empty, `.`, or `..` segments — `a//b`, `./a`, and `a/../b` are all
///   rejected rather than normalized, because a crawler emitting them is
///   emitting URLs Tome did not plan for, and silent normalization is how
///   two spellings of one page become two pages
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PagePath(String);

impl PagePath {
    pub const MAX_LEN: usize = 2048;

    pub fn new(path: impl Into<String>) -> Result<Self> {
        let path = path.into();
        let message = if path.is_empty() {
            Some("must not be empty")
        } else if path.len() > Self::MAX_LEN {
            Some("must be at most 2048 bytes")
        } else if path.contains('\0') {
            Some("must not contain NUL")
        } else if path.contains('\\') {
            Some("must use forward slashes")
        } else if path.starts_with('/') {
            Some("must be relative to the source root, without a leading slash")
        } else if path
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
        {
            Some("must not contain empty, '.', or '..' segments")
        } else {
            None
        };
        match message {
            // Not echoed: page paths are reading history (see error.rs), and
            // this message can end up in a log or a pasted issue.
            Some(message) => Err(Error::InvalidPagePath { message }),
            None => Ok(Self(path)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PagePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for PagePath {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl From<PagePath> for String {
    fn from(path: PagePath) -> Self {
        path.0
    }
}

impl AsRef<str> for PagePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// SHA-256 of a page's *normalized* content, lowercase hex. Change detection
/// compares these; equal hashes mean re-render and re-index can be skipped.
///
/// Strictly 64 lowercase hex characters — uppercase is rejected rather than
/// folded, because two spellings of one hash defeats the only purpose the
/// value has (equality).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentHash(String);

impl ContentHash {
    /// From a SHA-256 digest.
    pub fn from_digest(digest: [u8; 32]) -> Self {
        let mut hex = String::with_capacity(64);
        for byte in digest {
            // Manual hex keeps this dependency-free; the hashing itself
            // lives with the ingest pipeline, which owns the sha2 dep.
            let _ = fmt::Write::write_fmt(&mut hex, format_args!("{byte:02x}"));
        }
        Self(hex)
    }

    pub fn new(hash: impl Into<String>) -> Result<Self> {
        let hash = hash.into();
        if hash.len() == 64
            && hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(hash))
        } else {
            Err(Error::InvalidContentHash)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for ContentHash {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl From<ContentHash> for String {
    fn from(hash: ContentHash) -> Self {
        hash.0
    }
}

/// One page's metadata. Identity is `(source, path)` — see the module docs
/// in `model/mod.rs` for why there is no surrogate id. The page *content*
/// deliberately lives elsewhere (rendered files under `pages_dir`, AST in
/// [`DocPage`](crate::model::DocPage)): metadata is loaded in bulk for lists
/// and search results, content one page at a time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub source: SourceId,
    pub path: PagePath,
    pub title: String,
    pub content_hash: ContentHash,
    /// When Tome fetched it, not the upstream mtime — the HTTP validators
    /// below carry upstream's claims.
    pub fetched_at: DateTime<Utc>,
    /// `ETag` from the last fetch, for `If-None-Match` on re-sync.
    pub etag: Option<String>,
    /// `Last-Modified` from the last fetch, verbatim, for `If-Modified-Since`.
    pub last_modified: Option<String>,
}

impl Page {
    pub fn new(
        source: SourceId,
        path: PagePath,
        title: impl Into<String>,
        content_hash: ContentHash,
    ) -> Self {
        Self {
            source,
            path,
            title: title.into(),
            content_hash,
            fetched_at: Utc::now(),
            etag: None,
            last_modified: None,
        }
    }
}
