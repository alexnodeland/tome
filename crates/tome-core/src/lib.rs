//! Shared core for Tome.
//!
//! This crate is depended on by **both** the desktop app (`src-tauri`) and the
//! `tome` CLI. That is deliberate and load-bearing: the app, the CLI, and the
//! MCP server must observe exactly the same library on disk. See
//! `docs/decisions/0002-no-app-sandbox.md` — the reason Tome ships without App
//! Sandbox is that a sandboxed app would resolve a *different* data directory
//! than a Homebrew-installed CLI, and the two would silently diverge.
//!
//! Nothing outside [`paths`] may construct a data path.

pub mod config;
pub mod crawl;
pub mod db;
pub mod error;
pub mod fetch;
pub mod model;
pub mod parse;
pub mod paths;

pub use error::{Error, Result};
pub use paths::Paths;

pub use fetch::ssrf;

/// Reverse-DNS bundle identifier. See `docs/decisions/0004-bundle-identifier.md`.
///
/// Single source of truth: the Keychain service name, the iCloud container, the
/// preferences domain, and the Homebrew `zap` list are all derived from this.
pub const BUNDLE_ID: &str = "com.alexnodeland.tome";

/// Human-facing application name, used for on-disk directory names.
pub const APP_NAME: &str = "Tome";
