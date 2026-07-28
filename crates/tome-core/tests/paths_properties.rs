//! Property tests for path resolution — implementation-plan **S0-8**.
//!
//! The unit tests in `paths.rs` assert the cases someone thought of. These
//! assert the invariants over inputs nobody thought of, which is the class of
//! bug that reaches a user: a source id containing a space, a NUL, a
//! right-to-left override, or 4 KB of Unicode.
//!
//! `proptest` is scaffolding here in the sense of S0-8 — the interesting
//! targets (normalization, sync convergence, annotation re-anchoring) do not
//! exist yet. It is not scaffolding in the sense of being fake: these
//! properties are the real contract `paths` owes every other module.
//!
//! # Containment, resolved
//!
//! The S0 version of this header documented a deliberate gap: a hostile
//! source id (`"../../etc"`) escaped the cache directory lexically, because
//! the accessors took `&str`. They now take
//! [`SourceId`](tome_core::model::SourceId), whose validation refuses
//! separators and dot-leading names — the hostile values fail *construction*
//! (asserted in `model_validation.rs` and the `model_ids` fuzz target), and
//! the properties here hold for every id that can exist at all.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::{Component, Path, PathBuf};

use proptest::prelude::*;
use tome_core::model::SourceId;
use tome_core::Paths;

/// Every constructible source id. The regex is `SourceId`'s documented
/// grammar (if one changes, change the other); mapping through
/// `SourceId::new` means a grammar/validator mismatch fails loudly here.
fn benign_source_id() -> impl Strategy<Value = SourceId> {
    "[a-zA-Z0-9][a-zA-Z0-9._+-]{0,63}"
        .prop_map(|s| SourceId::new(s).expect("generator emits only valid ids"))
}

/// Absolute roots, including the awkward ones: spaces (macOS paths are full of
/// them), dots, and non-ASCII.
///
/// A literal `~` component is excluded, and not because it would be awkward —
/// because the tilde property below is about what *this module* constructs. A
/// user who sets `TOME_HOME=/Users/x/~` gets a directory called `~`, and that
/// is their decision; the bug being guarded against is Tome building one.
fn absolute_root() -> impl Strategy<Value = PathBuf> {
    proptest::collection::vec("[^/~\\x00]{1,16}", 1..5)
        .prop_map(|parts| PathBuf::from(format!("/{}", parts.join("/"))))
}

fn every_path(paths: &Paths, source_id: &SourceId) -> Vec<PathBuf> {
    vec![
        paths.state_root().to_path_buf(),
        paths.cache_root().to_path_buf(),
        paths.config_file(),
        paths.sources_dir(),
        paths.source_config_file(source_id),
        paths.database_file(),
        paths.logs_dir(),
        paths.index_dir(),
        paths.source_data_dir(source_id),
        paths.pages_dir(source_id),
        paths.raw_dir(source_id),
        paths.assets_dir(source_id),
    ]
}

proptest! {
    /// State and cache must never nest. The split is what lets macOS evict the
    /// cache, `brew uninstall --zap` be correct, and a backup be one directory:
    /// if either root ever contained the other, deleting the cache would take
    /// bookmarks and annotations with it.
    #[test]
    fn state_and_cache_never_contain_each_other(root in absolute_root()) {
        let paths = Paths::under_root(&root);

        prop_assert_ne!(paths.state_root(), paths.cache_root());
        prop_assert!(!paths.state_root().starts_with(paths.cache_root()));
        prop_assert!(!paths.cache_root().starts_with(paths.state_root()));
    }

    /// Irreplaceable data lives under state; re-fetchable data lives under
    /// cache. Every accessor, for any id, on the correct side of that line.
    #[test]
    fn each_path_lands_on_the_correct_side_of_the_split(
        root in absolute_root(),
        source_id in benign_source_id(),
    ) {
        let paths = Paths::under_root(&root);

        for state_path in [
            paths.config_file(),
            paths.sources_dir(),
            paths.source_config_file(&source_id),
            paths.database_file(),
            paths.logs_dir(),
        ] {
            prop_assert!(
                state_path.starts_with(paths.state_root()),
                "{state_path:?} is irreplaceable and must live under the state root"
            );
        }

        for cache_path in [
            paths.index_dir(),
            paths.source_data_dir(&source_id),
            paths.pages_dir(&source_id),
            paths.raw_dir(&source_id),
            paths.assets_dir(&source_id),
        ] {
            prop_assert!(
                cache_path.starts_with(paths.cache_root()),
                "{cache_path:?} is re-fetchable and must live under the cache root"
            );
        }
    }

    /// The tilde bug, generalised. A literal `~` component means something was
    /// built from shell syntax rather than resolved, and it lands in whatever
    /// the process's working directory happens to be — which differs between
    /// the app and the CLI.
    #[test]
    fn no_resolved_path_is_relative_or_contains_a_tilde(
        root in absolute_root(),
        source_id in benign_source_id(),
    ) {
        let paths = Paths::under_root(&root);

        for path in every_path(&paths, &source_id) {
            prop_assert!(path.is_absolute(), "{path:?} must be absolute");
            prop_assert!(
                !path.components().any(|c| c.as_os_str() == "~"),
                "{path:?} contains a literal tilde component"
            );
        }
    }

    /// A benign id never introduces a path component of its own. If it did,
    /// two ids could resolve to the same directory, or one could reach a
    /// sibling source's cache.
    #[test]
    fn a_benign_source_id_is_exactly_one_component(source_id in benign_source_id()) {
        let paths = Paths::under_root("/tmp/tome-properties");
        let data_dir = paths.source_data_dir(&source_id);

        let extra: Vec<_> = data_dir
            .strip_prefix(paths.cache_root().join("data"))
            .expect("source data dir is under <cache>/data")
            .components()
            .collect();

        prop_assert_eq!(extra.len(), 1);
        prop_assert!(matches!(extra[0], Component::Normal(_)));
    }

    /// Distinct sources never share a cache directory.
    #[test]
    fn distinct_source_ids_get_distinct_directories(
        first in benign_source_id(),
        second in benign_source_id(),
    ) {
        prop_assume!(first != second);
        let paths = Paths::under_root("/tmp/tome-properties");

        prop_assert_ne!(paths.pages_dir(&first), paths.pages_dir(&second));
        prop_assert_ne!(paths.source_config_file(&first), paths.source_config_file(&second));
    }

    /// Resolution is pure: same input, same answer, no hidden state. This is
    /// what makes `app_and_cli_resolve_identical_paths` mean anything for two
    /// processes rather than two calls in one.
    #[test]
    fn resolution_is_deterministic(root in absolute_root(), source_id in benign_source_id()) {
        prop_assert_eq!(
            every_path(&Paths::under_root(&root), &source_id),
            every_path(&Paths::under_root(&root), &source_id)
        );
    }
}

proptest! {
    // Touches the filesystem, so run fewer cases than the default 256.
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Directory creation is idempotent and owner-only for any id we accept.
    /// It runs on every launch of both binaries; a second call must never fail,
    /// and a mode drift would leave reading history world-readable.
    #[test]
    fn ensure_created_is_idempotent_and_private(source_id in benign_source_id()) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Paths::under_root(tmp.path());

        paths.ensure_created().expect("first launch");
        paths.ensure_created().expect("second launch");
        paths.ensure_source_dirs(&source_id).expect("source dirs");
        paths.ensure_source_dirs(&source_id).expect("source dirs again");

        for dir in [paths.pages_dir(&source_id), paths.raw_dir(&source_id)] {
            prop_assert!(dir.is_dir(), "{dir:?} was not created");
            prop_assert!(is_owner_only(&dir), "{dir:?} is not 0700");
        }
    }
}

#[cfg(unix)]
fn is_owner_only(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o777 == 0o700)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_owner_only(_path: &Path) -> bool {
    true
}
