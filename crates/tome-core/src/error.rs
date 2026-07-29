//! The error taxonomy.
//!
//! Frozen early (implementation plan S0-4) so that every later module returns
//! into it rather than inventing its own. Two rules:
//!
//! 1. **Messages are user-facing.** No stack traces, no internal type names.
//! 2. **Messages never carry user content.** Page paths, search queries, and
//!    note text are reading history; they must not reach a log file or a
//!    diagnostics bundle that someone will paste into a public issue.

use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Could not determine your home directory.")]
    NoHomeDirectory,

    #[error("Could not create {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Permission denied: {path}. Check that you can write to this location.")]
    PermissionDenied { path: PathBuf },

    #[error("Invalid configuration in {file}: {message}")]
    Config { file: PathBuf, message: String },

    // The three validation errors below deliberately do not echo the
    // rejected value (rule 2 above): source ids arrive in config files and
    // registry entries, page paths are reading history. The message states
    // the rule that was violated; the caller that has the value decides
    // whether its context is safe enough to log it.
    #[error("Invalid source id: {message}.")]
    InvalidSourceId { message: &'static str },

    #[error("Invalid page path: {message}.")]
    InvalidPagePath { message: &'static str },

    #[error("Invalid content hash: expected 64 lowercase hex characters.")]
    InvalidContentHash,

    // SQLite's own messages are technical but carry no user content (no
    // page paths, no queries) — bound parameters never appear in them.
    #[error("Database error: {message}")]
    Database { message: String },

    // Fetch errors carry no URL: which page was being fetched is the
    // caller's to log at a level it controls, not this message's.
    #[error("The download failed: {message}")]
    Fetch { message: String },

    #[error("The server responded with HTTP {status}.")]
    Http { status: u16 },

    #[error("This site's robots.txt does not permit fetching that page.")]
    BlockedByRobots,

    #[error("That address is not a public destination and will not be fetched.")]
    BlockedByFilter,

    #[error("The response exceeded the size limit ({limit} bytes).")]
    TooLarge { limit: u64 },

    // Stored page content that will not load. The message names the file, a
    // content hash under the cache root, and never the page path — same rule
    // as `InvalidPagePath`: what someone reads is not diagnostics.
    #[error(
        "Stored page content could not be read ({message}). Re-sync the source to rebuild it."
    )]
    PageStore { message: String },

    // Unlike the validation errors above, this one DOES carry the underlying
    // message. Tantivy's errors name index internals — a segment file, a
    // schema mismatch, a held lock — and a query parse error names the syntax
    // that failed, not the library it was run against. A search error with the
    // reason stripped out is unactionable, and the index is a cache: the
    // remedy is usually "delete it and re-index", which the user can only be
    // told to do if we say what broke.
    #[error("Search failed: {message}")]
    Search { message: String },

    /// Rendering a manual page failed (S2-11, P2-013).
    ///
    /// Carries the message because the remedy is usually specific and
    /// actionable — a missing `mandoc`, an unreadable directory — and because
    /// nothing here is user content: a man page path is a system path, not
    /// reading history.
    #[error("Manual page: {message}")]
    Man { message: String },

    /// The index on disk was written against a different schema.
    ///
    /// Its own variant rather than a [`Search`](Self::Search) with a tantivy
    /// message in it, because it is the one search failure with a specific,
    /// safe remedy that the user can act on — re-index — and because a read
    /// command must be able to *say* that rather than silently deleting a
    /// derived-but-expensive-to-repopulate index behind the user's back.
    ///
    /// Adding, removing or retyping a field in `search::schema` causes this
    /// for every existing library. The index lives under the cache root
    /// precisely so that is recoverable: SPIKE-003 measured a rebuild at
    /// 5–21 seconds for 100 000 pages, against about seven hours to re-crawl.
    #[error(
        "The search index was built for an older version of Tome and cannot be read. \
         Run `tome pull --all` to rebuild it."
    )]
    IndexSchemaOutdated,

    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    /// A concrete next step for the user, where one exists.
    pub fn suggestion(&self) -> Option<&'static str> {
        match self {
            Self::NoHomeDirectory => Some("Set $TOME_HOME to a writable directory."),
            Self::PermissionDenied { .. } => {
                Some("Check the directory's permissions, or set $TOME_HOME elsewhere.")
            }
            _ => None,
        }
    }

    /// Whether retrying the same operation could plausibly succeed.
    ///
    /// Note the fetcher already retries transport errors and 5xx internally
    /// (S1-4); a fetch error surfacing here has exhausted those retries, so
    /// "retryable" means "worth trying again next sync", not "immediately".
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Io(_) | Self::Fetch { .. } => true,
            Self::Http { status } => *status == 429 || *status >= 500,
            _ => false,
        }
    }
}
