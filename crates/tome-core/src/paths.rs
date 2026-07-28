//! Path resolution — the single source of truth for every location Tome uses.
//!
//! **Nothing else in the codebase may construct a data path.** Every directory
//! and file comes from here.
//!
//! That rule exists because the planning documents previously named four
//! different data locations (`~/.tome/`, `~/Library/Application Support/Tome`,
//! `~/Library/Caches/<bundle-id>`, and `dirs::data_dir()/tome/`) across four
//! documents, and several code samples passed a literal `~` to APIs that do not
//! expand it — which silently creates a directory named `~` in the process's
//! working directory. Centralising resolution makes both classes of bug
//! impossible rather than merely discouraged.
//!
//! # Layout
//!
//! ```text
//! ~/Library/Application Support/Tome/   state — irreplaceable, back this up
//! ├── config.yaml
//! ├── sources/                          source configurations (YAML)
//! ├── tome.db                           bookmarks, annotations, metadata
//! └── logs/
//!
//! ~/Library/Caches/Tome/                re-fetchable — safe to delete
//! ├── data/<source-id>/
//! │   ├── pages/                        normalized, sanitized HTML
//! │   ├── raw/                          original fetched bytes
//! │   └── assets/                       content-addressed
//! └── index/                            Tantivy index
//! ```
//!
//! Splitting state from cache is what lets macOS evict the cache under disk
//! pressure, lets `brew uninstall --zap` be correct, and lets us tell a user
//! which single directory to back up.
//!
//! # `TOME_HOME`
//!
//! If `TOME_HOME` is set, **both** roots live under it (`<root>/state`,
//! `<root>/cache`). This is the escape hatch for users who want everything in
//! one place, and it is how the tests get a temporary root.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::model::SourceId;
use crate::APP_NAME;

/// Environment variable that overrides both roots.
pub const TOME_HOME_ENV: &str = "TOME_HOME";

/// Resolved locations for one Tome library.
///
/// Cheap to clone; holds two owned roots and derives everything else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    state_root: PathBuf,
    cache_root: PathBuf,
}

impl Paths {
    /// Resolve from the environment.
    ///
    /// Honours `TOME_HOME` if set; otherwise uses the macOS conventional
    /// locations. **The app, the CLI, and the MCP server all call this**, which
    /// is what guarantees they observe the same library — asserted by
    /// [`tests::app_and_cli_resolve_identical_paths`].
    pub fn resolve() -> Result<Self> {
        match std::env::var_os(TOME_HOME_ENV) {
            Some(raw) if !raw.is_empty() => Ok(Self::under_root(PathBuf::from(raw))),
            _ => Self::platform_default(),
        }
    }

    /// Conventional per-platform locations, ignoring `TOME_HOME`.
    pub fn platform_default() -> Result<Self> {
        // `BaseDirs` expands the home directory properly. A literal
        // `PathBuf::from("~/…")` would NOT — `~` is shell syntax, not a path
        // component, and every filesystem API treats it as a directory name.
        let base = directories::BaseDirs::new().ok_or(Error::NoHomeDirectory)?;

        Ok(Self {
            // macOS: ~/Library/Application Support
            state_root: base.data_dir().join(APP_NAME),
            // macOS: ~/Library/Caches
            cache_root: base.cache_dir().join(APP_NAME),
        })
    }

    /// Put both roots under a single directory. Used for `TOME_HOME` and tests.
    pub fn under_root(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            state_root: root.join("state"),
            cache_root: root.join("cache"),
        }
    }

    // ---- state: irreplaceable, back this up ---------------------------------

    /// Root of everything that cannot be regenerated.
    pub fn state_root(&self) -> &Path {
        &self.state_root
    }

    /// Global configuration file.
    pub fn config_file(&self) -> PathBuf {
        self.state_root.join("config.yaml")
    }

    /// Directory holding one YAML file per documentation source.
    pub fn sources_dir(&self) -> PathBuf {
        self.state_root.join("sources")
    }

    /// Configuration file for a single source.
    ///
    /// Takes [`SourceId`], not `&str` — that is the containment story. A
    /// `SourceId` admits no path separator, no NUL, and no leading dot, so
    /// every accessor below joins exactly one well-formed component and
    /// cannot be steered outside its root. The S0 scaffold took `&str` here
    /// and documented `pages_dir("../../etc")` escaping as a known gap; the
    /// fix was to make the hostile value unrepresentable, not to sprinkle
    /// checks at each call site.
    pub fn source_config_file(&self, source_id: &SourceId) -> PathBuf {
        self.sources_dir().join(format!("{source_id}.yaml"))
    }

    /// SQLite database: bookmarks, annotations, page metadata, sync state.
    pub fn database_file(&self) -> PathBuf {
        self.state_root.join("tome.db")
    }

    /// Rotated log files. Written only when debug mode is enabled.
    pub fn logs_dir(&self) -> PathBuf {
        self.state_root.join("logs")
    }

    // ---- cache: re-fetchable, safe to delete --------------------------------

    /// Root of everything that can be rebuilt by re-fetching.
    pub fn cache_root(&self) -> &Path {
        &self.cache_root
    }

    /// Tantivy search index. Rebuildable, hence cache rather than state.
    pub fn index_dir(&self) -> PathBuf {
        self.cache_root.join("index")
    }

    /// Cached content for one source. See [`Self::source_config_file`] for
    /// why this takes [`SourceId`].
    pub fn source_data_dir(&self, source_id: &SourceId) -> PathBuf {
        self.cache_root.join("data").join(source_id.as_str())
    }

    /// Normalized, sanitized, highlight-annotated HTML — what the reader loads.
    pub fn pages_dir(&self, source_id: &SourceId) -> PathBuf {
        self.source_data_dir(source_id).join("pages")
    }

    /// Original fetched bytes, kept so content can be re-normalized without
    /// re-crawling when the pipeline changes.
    pub fn raw_dir(&self, source_id: &SourceId) -> PathBuf {
        self.source_data_dir(source_id).join("raw")
    }

    /// Content-addressed assets (`<sha256>.<ext>`), deduplicated across pages.
    pub fn assets_dir(&self, source_id: &SourceId) -> PathBuf {
        self.source_data_dir(source_id).join("assets")
    }

    // ---- creation -----------------------------------------------------------

    /// Create every directory Tome needs, with restrictive permissions.
    ///
    /// Idempotent: safe to call on every launch, from every binary.
    pub fn ensure_created(&self) -> Result<()> {
        for dir in [
            self.state_root.clone(),
            self.sources_dir(),
            self.logs_dir(),
            self.cache_root.clone(),
            self.index_dir(),
            self.cache_root.join("data"),
        ] {
            create_private_dir(&dir)?;
        }
        Ok(())
    }

    /// Create the per-source cache directories for one source.
    pub fn ensure_source_dirs(&self, source_id: &SourceId) -> Result<()> {
        for dir in [
            self.pages_dir(source_id),
            self.raw_dir(source_id),
            self.assets_dir(source_id),
        ] {
            create_private_dir(&dir)?;
        }
        Ok(())
    }
}

/// Create a directory owner-only (`0700`).
///
/// The library records what a person reads. Default umask would leave it
/// group- and world-readable on a shared machine.
fn create_private_dir(path: &Path) -> Result<()> {
    if let Err(source) = std::fs::create_dir_all(path) {
        return Err(match source.kind() {
            std::io::ErrorKind::PermissionDenied => Error::PermissionDenied {
                path: path.to_path_buf(),
            },
            _ => Error::CreateDirectory {
                path: path.to_path_buf(),
                source,
            },
        });
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }

    Ok(())
}

/// Restrict a file to `0600`. For `tome.db` and source configuration files.
#[cfg(unix)]
pub fn restrict_file(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // Panicking on setup failure is correct in tests.
mod tests {
    use super::*;

    fn sid(id: &str) -> SourceId {
        SourceId::new(id).unwrap()
    }

    #[test]
    fn under_root_separates_state_from_cache() {
        let p = Paths::under_root("/tmp/tome-test");

        assert_eq!(p.state_root(), Path::new("/tmp/tome-test/state"));
        assert_eq!(p.cache_root(), Path::new("/tmp/tome-test/cache"));

        // The split is the point: losing the cache must never lose state.
        assert!(!p.database_file().starts_with(p.cache_root()));
        assert!(!p.index_dir().starts_with(p.state_root()));
    }

    #[test]
    fn platform_default_uses_macos_conventions() {
        let p = Paths::platform_default().unwrap();

        assert!(
            p.state_root().ends_with("Tome"),
            "state root should end with the app name, got {:?}",
            p.state_root()
        );

        #[cfg(target_os = "macos")]
        {
            assert!(p
                .state_root()
                .to_string_lossy()
                .contains("Application Support"));
            assert!(p.cache_root().to_string_lossy().contains("Caches"));
        }
    }

    /// The tilde bug the planning documents were riddled with.
    ///
    /// `PathBuf::from("~/…")` produces a *relative* path whose first component
    /// is the literal character `~`. Anything built that way lands in the
    /// process's working directory — which differs between the app and the CLI.
    #[test]
    fn no_resolved_path_contains_a_literal_tilde() {
        let p = Paths::platform_default().unwrap();

        let all = [
            p.state_root().to_path_buf(),
            p.cache_root().to_path_buf(),
            p.config_file(),
            p.sources_dir(),
            p.database_file(),
            p.logs_dir(),
            p.index_dir(),
            p.pages_dir(&sid("rust-std")),
            p.raw_dir(&sid("rust-std")),
            p.assets_dir(&sid("rust-std")),
        ];

        for path in all {
            assert!(
                path.is_absolute(),
                "every resolved path must be absolute, got {path:?}"
            );
            assert!(
                !path.components().any(|c| c.as_os_str() == "~"),
                "path contains a literal `~` component: {path:?}"
            );
        }
    }

    /// The invariant that ADR-0002 exists to protect.
    ///
    /// The desktop app and the `tome` CLI are separate processes reaching the
    /// same library. If they ever resolve different roots, bookmarks added in
    /// the app are invisible to the CLI and to the MCP server, and the entire
    /// integration story silently breaks. Both call `Paths::resolve()`; this
    /// asserts that call is deterministic.
    #[test]
    fn app_and_cli_resolve_identical_paths() {
        let as_app = Paths::resolve().unwrap();
        let as_cli = Paths::resolve().unwrap();
        assert_eq!(as_app, as_cli);
    }

    #[test]
    fn tome_home_override_is_honoured() {
        let tmp = tempfile::tempdir().unwrap();
        let expected = Paths::under_root(tmp.path());

        // Scoped so the variable does not leak into sibling tests.
        temp_env(TOME_HOME_ENV, Some(tmp.path().as_os_str()), || {
            assert_eq!(Paths::resolve().unwrap(), expected);
        });
    }

    #[test]
    fn empty_tome_home_falls_back_to_platform_default() {
        temp_env(TOME_HOME_ENV, Some("".as_ref()), || {
            let resolved = Paths::resolve().unwrap();
            assert_eq!(resolved, Paths::platform_default().unwrap());
        });
    }

    #[test]
    fn ensure_created_makes_every_directory_owner_only() {
        let tmp = tempfile::tempdir().unwrap();
        let p = Paths::under_root(tmp.path());

        p.ensure_created().unwrap();
        p.ensure_source_dirs(&sid("rust-std")).unwrap();

        for dir in [
            p.state_root().to_path_buf(),
            p.sources_dir(),
            p.logs_dir(),
            p.cache_root().to_path_buf(),
            p.index_dir(),
            p.pages_dir(&sid("rust-std")),
            p.assets_dir(&sid("rust-std")),
        ] {
            assert!(dir.is_dir(), "expected directory: {dir:?}");

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o700, "{dir:?} should be owner-only, got {mode:o}");
            }
        }
    }

    #[test]
    fn ensure_created_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let p = Paths::under_root(tmp.path());

        p.ensure_created().unwrap();
        p.ensure_created().unwrap(); // must not error on the second launch
    }

    #[test]
    fn hostile_source_ids_cannot_reach_the_accessors_at_all() {
        // The S0 version of this test called `pages_dir("../../etc")` and
        // asserted the result did not land in /etc — the best a &str
        // signature allowed. The accessors now take &SourceId, and the
        // hostile values fail *construction*, which is the stronger claim:
        // there is no call site to get wrong.
        for hostile in ["../../etc", "..", "a/b", "a\\b", ".ssh", "~root", ""] {
            assert!(
                SourceId::new(hostile).is_err(),
                "must not construct: {hostile:?}"
            );
        }

        // And every id that does construct stays inside the cache root.
        let p = Paths::under_root("/tmp/tome-test");
        let id = sid("rust-std");
        assert!(p.pages_dir(&id).starts_with(p.cache_root()));
        assert_eq!(p.source_data_dir(&id), p.cache_root().join("data/rust-std"));
    }

    /// Set an environment variable for the duration of a closure.
    ///
    /// `std::env::set_var` is process-global and Rust runs tests in threads, so
    /// this is only sound because the env-touching tests are serialised by
    /// taking the same lock.
    fn temp_env(key: &str, value: Option<&std::ffi::OsStr>, f: impl FnOnce()) {
        use std::sync::{Mutex, OnceLock};
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = LOCK.get_or_init(|| Mutex::new(())).lock();

        let previous = std::env::var_os(key);
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }

        f();

        match previous {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}
