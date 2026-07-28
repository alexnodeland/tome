//! [`Source`] and its satellite types: identity, kind, sync, attribution.

use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, Result};

/// A source's identity: the slug that names its config file
/// (`sources/<id>.yaml`), its data directories, and its half of every
/// bookmark and annotation key.
///
/// Validation is the point of the type. The id reaches
/// [`Paths`](crate::Paths) accessors that join it into filesystem paths, so
/// the character set admits no separator, no NUL, and cannot begin with a
/// dot — `SourceId` is the reason `pages_dir(hostile)` cannot escape the
/// cache root once the S1 loaders take `&SourceId` instead of `&str`. Rules:
///
/// - 1 to 64 bytes, ASCII only
/// - first character alphanumeric
/// - the rest alphanumeric or `.`, `_`, `+`, `-`
///
/// (The same grammar as the `benign_source_id` proptest generator; if one
/// changes, change the other.)
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceId(String);

impl SourceId {
    pub const MAX_LEN: usize = 64;

    pub fn new(id: impl Into<String>) -> Result<Self> {
        let id = id.into();
        let mut chars = id.chars();
        let message = match chars.next() {
            None => Some("must not be empty"),
            Some(first) if !first.is_ascii_alphanumeric() => {
                Some("must start with an ASCII letter or digit")
            }
            Some(_) => {
                if id.len() > Self::MAX_LEN {
                    Some("must be at most 64 characters")
                } else if !chars.all(|c| c.is_ascii_alphanumeric() || ".-_+".contains(c)) {
                    Some("may contain only ASCII letters, digits, and the characters . - _ +")
                } else {
                    None
                }
            }
        };
        match message {
            // The offending value is deliberately not echoed: source ids come
            // from config files and registry entries, and the error taxonomy
            // promises messages free of user content.
            Some(message) => Err(Error::InvalidSourceId { message }),
            None => Ok(Self(id)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for SourceId {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl From<SourceId> for String {
    fn from(id: SourceId) -> Self {
        id.0
    }
}

impl AsRef<str> for SourceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// What kind of documentation this source is, which decides the scraper and
/// the normalization rules. Mirrors `source.type` in the config schema
/// (PRD Appendix A) — if a variant is added there, it is added here in the
/// same change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum SourceType {
    ReadTheDocs,
    Rustdoc,
    MdBook,
    Man,
    Generic,
    Local,
    Docset,
}

impl SourceType {
    /// The config-schema spelling (`readthedocs`, `mdbook`, …).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadTheDocs => "readthedocs",
            Self::Rustdoc => "rustdoc",
            Self::MdBook => "mdbook",
            Self::Man => "man",
            Self::Generic => "generic",
            Self::Local => "local",
            Self::Docset => "docset",
        }
    }
}

/// When a source re-fetches. The strategy carries its own parameters so the
/// invalid combinations the PRD warns about (`schedule` without `scheduled`,
/// `weekly` as a *strategy*) are unrepresentable here; mapping the flat YAML
/// into this shape — and rejecting the nonsense — is the S1-3 parser's job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SyncStrategy {
    Manual,
    OnLaunch,
    Scheduled {
        schedule: Schedule,
    },
    /// Re-fetch when an external registry publishes a new version
    /// (`"crates:serde"`, `"npm:react"`). Kept as an opaque string: how watch
    /// behaves — fetch vs notify — is DEC-006, which is open. Do not build on
    /// this variant until that is decided.
    Watch {
        source: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Schedule {
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncConfig {
    pub strategy: SyncStrategy,
    /// If true, never auto-update — even a scheduled sync only revalidates
    /// the pinned version.
    pub pin_version: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            strategy: SyncStrategy::Manual,
            pin_version: false,
        }
    }
}

/// Where this documentation came from and under what terms. Captured at
/// ingest, shown in the reader footer, embedded in exports — SPIKE-010's
/// attribution rules made this a first-class part of the model rather than
/// display metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribution {
    /// Canonical upstream URL, distinct from the crawl entry point.
    pub homepage: Option<Url>,
    /// SPDX identifier where determinable. `None` renders as "licence
    /// unknown" — an honest value, never a guess.
    pub licence: Option<String>,
}

/// A source's icon, as the config schema allows it: emoji, remote URL, or a
/// local file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Icon {
    Emoji(String),
    Remote(Url),
    File(PathBuf),
}

/// A documentation source: one entry in the library.
///
/// This is the domain type. The YAML config that *creates* one is S1-3's
/// concern, and the database row that *stores* one is S1-2's; both map into
/// this and neither shape leaks out of its module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub id: SourceId,
    /// Human-readable display name ("Python 3.13").
    pub name: String,
    pub kind: SourceType,
    /// Crawl entry point for remote sources. `https` unless the config set
    /// `allow_insecure` for a host the user owns.
    pub url: Option<Url>,
    /// Root directory for `SourceType::Local` and `SourceType::Docset`.
    pub local_path: Option<PathBuf>,
    /// Upstream version as displayed ("3.13", "1.0.219").
    pub version: Option<String>,
    pub category: String,
    pub icon: Option<Icon>,
    /// Hex colour for UI accents; validated by the config parser, carried
    /// verbatim here.
    pub accent_color: Option<String>,
    pub attribution: Attribution,
    pub sync: SyncConfig,
    pub created_at: DateTime<Utc>,
    pub last_synced: Option<DateTime<Utc>>,
    /// Derived statistics, maintained by ingest. Zero until first sync.
    pub page_count: u32,
    pub index_size_bytes: u64,
}

impl Source {
    /// The default category for sources that declare none, as in the S1-2
    /// schema.
    pub const DEFAULT_CATEGORY: &'static str = "Uncategorized";

    /// A new, never-synced source with the given identity and kind.
    pub fn new(id: SourceId, name: impl Into<String>, kind: SourceType) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            url: None,
            local_path: None,
            version: None,
            category: Self::DEFAULT_CATEGORY.to_owned(),
            icon: None,
            accent_color: None,
            attribution: Attribution::default(),
            sync: SyncConfig::default(),
            created_at: Utc::now(),
            last_synced: None,
            page_count: 0,
            index_size_bytes: 0,
        }
    }
}
