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
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Io(_))
    }
}
