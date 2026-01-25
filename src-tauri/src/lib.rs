//! Tome - A native macOS documentation reader
//!
//! This library provides the core functionality for the Tome application,
//! including documentation scraping, parsing, storage, and search.
//!
//! # Architecture
//!
//! The library is organized into several modules:
//!
//! - [`commands`] - Tauri command handlers (IPC boundary)
//! - [`config`] - Configuration parsing and validation
//! - [`error`] - Centralized error types
//! - [`parser`] - HTML parsing and normalization
//! - [`scraper`] - HTTP scraping and crawling
//! - [`search`] - Full-text search with Tantivy
//! - [`storage`] - SQLite database and filesystem
//! - [`sync`] - Synchronization orchestration
//!
//! # Module Boundaries
//!
//! Each module has specific import rules to maintain clean architecture:
//!
//! - `commands` can import from all modules (orchestration layer)
//! - `sync` can import from `scraper`, `parser`, `storage`, `search`
//! - `scraper`, `parser`, `search` are independent of each other
//! - `storage` is a pure data layer
//! - `config` and `error` can be imported by all modules

#![deny(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

pub mod commands;
pub mod config;
pub mod error;
pub mod parser;
pub mod scraper;
pub mod search;
pub mod storage;
pub mod sync;

// Re-export commonly used types at the crate root
pub use error::TomeError;

/// Result type alias for Tome operations
pub type Result<T> = std::result::Result<T, TomeError>;

#[cfg(test)]
mod tests {
    // Crate-level tests go here
}
