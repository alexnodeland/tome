//! The error taxonomy.
//!
//! Frozen early (implementation plan S0-4) so that every later module returns
//! into it rather than inventing its own. Two rules:
//!
//! 1. **Messages are user-facing.** No stack traces, no internal type names,
//!    and every message is a whole sentence ending in a full stop — which is
//!    why an interpolated detail goes in parentheses rather than after a
//!    colon at the end. `error::tests` audits all three, exhaustively.
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

    #[error("Could not create {path} ({source}).")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Permission denied: {path}. Check that you can write to this location.")]
    PermissionDenied { path: PathBuf },

    #[error("Invalid configuration in {file} ({message}).")]
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
    #[error("The database reported an error ({message}).")]
    Database { message: String },

    // Fetch errors carry no URL: which page was being fetched is the
    // caller's to log at a level it controls, not this message's.
    #[error("The download failed ({message}).")]
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
    #[error("Search failed ({message}).")]
    Search { message: String },

    /// Rendering a manual page failed (S2-11, P2-013).
    ///
    /// Carries the message because the remedy is usually specific and
    /// actionable — a missing `mandoc`, an unreadable directory — and because
    /// nothing here is user content: a man page path is a system path, not
    /// reading history.
    #[error("The manual page could not be rendered ({message}).")]
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
    // The message named `tome pull --all` until S4-3, which would have worked
    // and would also have re-crawled every site: hours of network for a file
    // that can be rebuilt from local content in seconds. `tome debug
    // rebuild-index` exists now, so the message names it.
    #[error(
        "The search index was built for an older version of Tome and cannot be read. \
         Run `tome debug rebuild-index` to rebuild it from local content."
    )]
    IndexSchemaOutdated,

    // Parenthesised and terminated rather than bare `{0}`, because the
    // wrapped message is the OS's and ends without punctuation — and a
    // taxonomy where most messages are sentences and one is a fragment reads
    // as a bug in the one.
    #[error("A file operation failed ({0}).")]
    Io(#[from] std::io::Error),
}

/// Every command name any error message or suggestion is allowed to mention.
///
/// The P5-004 sketch made exactly this mistake — it suggested
/// `tome debug rebuild-index` at a time when no such command existed — and
/// left a note saying so. This is that note, enforced: a message naming a
/// command outside this list fails the audit below, and adding a name here
/// without adding the command fails
/// `crates/tome-cli/tests/debug_commands.rs`.
#[cfg(test)]
const REAL_COMMANDS: &[&str] = &[
    "tome add",
    "tome pull",
    "tome pull --all",
    "tome search",
    "tome list",
    "tome remove",
    "tome status",
    "tome config",
    "tome serve",
    "tome mcp",
    "tome debug check",
    "tome debug rebuild-index",
    "tome debug report",
];

impl Error {
    /// A concrete next step for the user, where one exists.
    ///
    /// Audited exhaustively by `suggestions_are_actionable_and_name_real_commands`
    /// below: **the match is deliberately not `_ =>`**, so adding a variant
    /// stops the build until someone decides what a person should do about it.
    /// That is the whole mechanism. A catch-all arm is how twenty variants came
    /// to share two suggestions.
    ///
    /// Every command named here must exist. An error that says to run something
    /// that is not a command is worse than one that says nothing, because it
    /// costs the reader a round trip to find that out.
    pub fn suggestion(&self) -> Option<&'static str> {
        match self {
            Self::NoHomeDirectory => Some("Set $TOME_HOME to a writable directory."),
            Self::CreateDirectory { .. } => {
                Some("Check that the parent directory exists and is writable, or set $TOME_HOME.")
            }
            Self::PermissionDenied { .. } => {
                Some("Check the directory's permissions, or set $TOME_HOME elsewhere.")
            }
            Self::Config { .. } => {
                Some("Run `tome debug check` to see every configuration that will not parse.")
            }
            Self::InvalidSourceId { .. } => {
                Some("Source ids are lowercase letters, digits and hyphens.")
            }
            // Both of these describe stored data that does not match this
            // build's rules, which a re-pull rewrites.
            Self::InvalidPagePath { .. } | Self::InvalidContentHash => {
                Some("Run `tome pull` for the source to rewrite its stored pages.")
            }
            Self::Database { .. } => Some(
                "Run `tome debug check`. If the database itself is damaged, move it aside \
                 and run `tome pull --all` — bookmarks and annotations are in that file.",
            ),
            Self::Fetch { .. } => Some("Check your network connection and try again."),
            Self::Http { status } => match status {
                404 => Some("The page may have moved. Run `tome pull` to re-crawl the source."),
                429 | 500..=599 => Some("The server is busy or unwell. Try again later."),
                // 401/403 and the rest of 4xx: the site is refusing, and no
                // action on this side changes that.
                _ => None,
            },
            // Deliberately terminal. `robots.txt` is obeyed by default and is
            // not overridable for registry-shipped configurations, so the
            // honest next step is "there isn't one".
            Self::BlockedByRobots => None,
            Self::BlockedByFilter => Some(
                "Set `fetch.allow_insecure` only for a server you own, such as an \
                 intranet mirror.",
            ),
            Self::TooLarge { .. } => {
                Some("Raise `fetch.max_asset_bytes` in the source's configuration if you trust it.")
            }
            Self::PageStore { .. } => Some("Run `tome pull` for the source to re-fetch it."),
            Self::Search { .. } => Some(
                "Run `tome debug rebuild-index`. The index is derived from local content \
                 and needs no network.",
            ),
            Self::Man { .. } => Some("Check that `mandoc` is installed — `brew install mandoc`."),
            Self::IndexSchemaOutdated => Some(
                "Run `tome debug rebuild-index`. The index is derived from local content \
                 and needs no network.",
            ),
            // A bare io::Error could be anything, and inventing a next step
            // for "anything" produces advice that is wrong more often than it
            // is right. The message carries the OS's own wording, which is at
            // least specific.
            Self::Io(_) => None,
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// One error of every variant.
    ///
    /// Kept in step with the enum by `every_variant_has_a_sample`: the count
    /// is asserted, so adding a variant and forgetting this list fails rather
    /// than silently narrowing the audit.
    fn samples() -> Vec<Error> {
        vec![
            Error::NoHomeDirectory,
            Error::CreateDirectory {
                path: PathBuf::from("/nope"),
                source: std::io::Error::other("no"),
            },
            Error::PermissionDenied {
                path: PathBuf::from("/nope"),
            },
            Error::Config {
                file: PathBuf::from("/nope.yaml"),
                message: "unknown field".to_owned(),
            },
            Error::InvalidSourceId {
                message: "must not be empty",
            },
            Error::InvalidPagePath {
                message: "must not escape the source root",
            },
            Error::InvalidContentHash,
            Error::Database {
                message: "disk image is malformed".to_owned(),
            },
            Error::Fetch {
                message: "connection reset".to_owned(),
            },
            Error::Http { status: 404 },
            Error::Http { status: 503 },
            Error::Http { status: 403 },
            Error::BlockedByRobots,
            Error::BlockedByFilter,
            Error::TooLarge { limit: 1024 },
            Error::PageStore {
                message: "unexpected end of input".to_owned(),
            },
            Error::Search {
                message: "schema mismatch".to_owned(),
            },
            Error::Man {
                message: "mandoc exited 1".to_owned(),
            },
            Error::IndexSchemaOutdated,
            Error::Io(std::io::Error::other("something")),
        ]
    }

    /// The name of a variant, from an exhaustive match.
    ///
    /// Adding a variant to `Error` stops the build here. That is the point:
    /// it is what makes "every variant is audited" a fact rather than a claim
    /// that was true when it was written.
    fn variant(error: &Error) -> &'static str {
        match error {
            Error::NoHomeDirectory => "NoHomeDirectory",
            Error::CreateDirectory { .. } => "CreateDirectory",
            Error::PermissionDenied { .. } => "PermissionDenied",
            Error::Config { .. } => "Config",
            Error::InvalidSourceId { .. } => "InvalidSourceId",
            Error::InvalidPagePath { .. } => "InvalidPagePath",
            Error::InvalidContentHash => "InvalidContentHash",
            Error::Database { .. } => "Database",
            Error::Fetch { .. } => "Fetch",
            Error::Http { .. } => "Http",
            Error::BlockedByRobots => "BlockedByRobots",
            Error::BlockedByFilter => "BlockedByFilter",
            Error::TooLarge { .. } => "TooLarge",
            Error::PageStore { .. } => "PageStore",
            Error::Search { .. } => "Search",
            Error::Man { .. } => "Man",
            Error::IndexSchemaOutdated => "IndexSchemaOutdated",
            Error::Io(_) => "Io",
        }
    }

    #[test]
    fn every_variant_has_a_sample() {
        // Bump this when you add a variant — and add it to `samples` too, or
        // the audit below quietly stops covering it.
        const VARIANTS: usize = 18;
        let covered: std::collections::BTreeSet<&str> = samples().iter().map(variant).collect();
        assert_eq!(
            covered.len(),
            VARIANTS,
            "samples() covers {covered:?}; the enum has {VARIANTS} variants"
        );
    }

    #[test]
    fn messages_are_written_for_a_person() {
        for error in samples() {
            let message = error.to_string();
            let name = variant(&error);
            assert!(!message.is_empty(), "{name} has an empty message");
            assert!(
                message.chars().next().is_some_and(char::is_uppercase),
                "{name}: messages are sentences — {message:?}"
            );
            assert!(
                matches!(message.chars().last(), Some('.' | '?' | '!')),
                "{name}: messages are sentences — {message:?}"
            );
            assert!(
                !message.contains('\n'),
                "{name}: one line, so a log entry stays one entry — {message:?}"
            );
        }
    }

    #[test]
    fn messages_leak_no_internals() {
        // A stack trace or a Debug-formatted struct in a user-facing message
        // is the specific thing P5-004 forbids. These are the shapes they
        // arrive in: `Custom { kind: Other, .. }` from a Debug'd io::Error,
        // `::` from a Rust path, `RUST_BACKTRACE` from a panic hook.
        for error in samples() {
            let message = error.to_string();
            let name = variant(&error);
            for forbidden in ["Custom {", "kind:", "RUST_BACKTRACE", "panicked at", "::"] {
                assert!(
                    !message.contains(forbidden),
                    "{name}: {forbidden:?} is an internal detail — {message:?}"
                );
            }
        }
    }

    #[test]
    fn suggestions_are_actionable_and_name_real_commands() {
        // Variants with no suggestion, and why. Each is a decision, not an
        // omission — see `suggestion`'s match arms for the reasoning.
        const NO_ACTION: &[&str] = &[
            "BlockedByRobots", // obeying robots.txt is not overridable
            "Io",              // could be anything; invented advice is wrong advice
        ];

        for error in samples() {
            let name = variant(&error);
            let suggestion = error.suggestion();

            // 403 is the deliberate `_ => None` arm inside `Http`.
            let may_be_none =
                NO_ACTION.contains(&name) || matches!(error, Error::Http { status: 403 });
            if suggestion.is_none() {
                assert!(
                    may_be_none,
                    "{name} has no suggestion. Either give it one or add it to NO_ACTION \
                     with a reason."
                );
                continue;
            }

            let text = suggestion.unwrap_or_default();
            assert!(
                matches!(text.chars().last(), Some('.' | '?' | '!')),
                "{name}: suggestions are sentences — {text:?}"
            );

            // Any backticked `tome …` must be a command that exists.
            for phrase in [text, &error.to_string()] {
                for fragment in phrase.split('`') {
                    if !fragment.starts_with("tome ") {
                        continue;
                    }
                    assert!(
                        REAL_COMMANDS.contains(&fragment),
                        "{name} names `{fragment}`, which is not in REAL_COMMANDS. \
                         Either the command does not exist, or this list is out of date."
                    );
                }
            }
        }
    }

    #[test]
    fn retryability_matches_what_the_caller_would_do() {
        assert!(Error::Http { status: 503 }.is_retryable());
        assert!(Error::Http { status: 429 }.is_retryable());
        assert!(!Error::Http { status: 404 }.is_retryable());
        assert!(!Error::BlockedByRobots.is_retryable());
        // A blocked address does not become fetchable by asking again, and a
        // retry loop over it is a scan.
        assert!(!Error::BlockedByFilter.is_retryable());
    }
}
