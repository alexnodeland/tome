//! Centralized error types for the Tome application.
//!
//! All errors in the application should use these types. This provides:
//! - Consistent error handling across modules
//! - Easy conversion to user-friendly messages
//! - Proper error chaining with source errors

use thiserror::Error;

/// Main error type for all Tome operations
#[derive(Error, Debug)]
pub enum TomeError {
    // === Storage Errors ===
    /// Database operation failed
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Filesystem operation failed
    #[error("Filesystem error: {0}")]
    Filesystem(#[from] std::io::Error),

    // === Network Errors ===
    /// HTTP request failed
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// HTTP response had non-success status
    #[error("HTTP error: status {status} for {url}")]
    HttpStatus { status: u16, url: String },

    // === Parse Errors ===
    /// HTML parsing failed
    #[error("HTML parse error: {0}")]
    HtmlParse(String),

    /// CSS selector is invalid
    #[error("Invalid CSS selector: {0}")]
    InvalidSelector(String),

    /// YAML configuration parsing failed
    #[error("Config parse error: {0}")]
    ConfigParse(#[from] serde_yaml::Error),

    /// JSON parsing failed
    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    // === Search Errors ===
    /// Tantivy search error
    #[error("Search error: {0}")]
    Search(#[from] tantivy::TantivyError),

    /// Search query syntax error
    #[error("Invalid search query: {0}")]
    InvalidQuery(String),

    // === Validation Errors ===
    /// Configuration validation failed
    #[error("Validation error: {0}")]
    Validation(String),

    /// Resource not found
    #[error("Not found: {resource}")]
    NotFound { resource: String },

    /// Invalid URL format
    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    // === Sync Errors ===
    /// Sync operation failed
    #[error("Sync error: {0}")]
    Sync(String),

    /// Source type not supported
    #[error("Unsupported source type: {0}")]
    UnsupportedSourceType(String),

    // === Generic Errors ===
    /// Internal error (should not happen)
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Error type for scraping operations
#[derive(Error, Debug)]
pub enum ScrapeError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("HTTP error: status {0}")]
    HttpStatus(reqwest::StatusCode),

    #[error("Invalid selector: {0}")]
    InvalidSelector(String),

    #[error("Content not found: {0}")]
    ContentNotFound(String),

    #[error("Title not found")]
    TitleNotFound,

    #[error("Client configuration error: {0}")]
    ClientConfig(String),
}

/// Error type for parsing operations
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("HTML parse error: {0}")]
    Html(String),

    #[error("Invalid structure: {0}")]
    InvalidStructure(String),
}

/// Error type for storage operations
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Filesystem error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Home directory not found")]
    NoHomeDirectory,

    #[error("Migration error: {0}")]
    Migration(String),
}

/// Error type for search operations
#[derive(Error, Debug)]
pub enum SearchError {
    #[error("Tantivy error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),

    #[error("Query parse error: {0}")]
    QueryParse(#[from] tantivy::query::QueryParserError),

    #[error("Missing field: {0}")]
    MissingField(&'static str),

    #[error("Index not initialized")]
    NotInitialized,
}

/// Error type for configuration operations
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("File read error for {0}: {1}")]
    FileRead(std::path::PathBuf, std::io::Error),

    #[error("Validation error: {0}")]
    Validation(String),
}

/// Error type for sync operations
#[derive(Error, Debug)]
pub enum SyncError {
    #[error("Source not found: {0}")]
    SourceNotFound(String),

    #[error("Scrape error: {0}")]
    Scrape(#[from] ScrapeError),

    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Search error: {0}")]
    Search(#[from] SearchError),

    #[error("Unsupported source type: {0}")]
    UnsupportedSourceType(String),

    #[error("Cancelled")]
    Cancelled,
}

// === Conversion implementations ===

impl From<ScrapeError> for TomeError {
    fn from(e: ScrapeError) -> Self {
        match e {
            ScrapeError::Network(e) => Self::Network(e),
            ScrapeError::HttpStatus(s) => Self::HttpStatus {
                status: s.as_u16(),
                url: String::new(),
            },
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<ParseError> for TomeError {
    fn from(e: ParseError) -> Self {
        Self::HtmlParse(e.to_string())
    }
}

impl From<StorageError> for TomeError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::Database(e) => Self::Database(e),
            StorageError::Io(e) => Self::Filesystem(e),
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<SearchError> for TomeError {
    fn from(e: SearchError) -> Self {
        match e {
            SearchError::Tantivy(e) => Self::Search(e),
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<ConfigError> for TomeError {
    fn from(e: ConfigError) -> Self {
        match e {
            ConfigError::YamlParse(e) => Self::ConfigParse(e),
            e => Self::Validation(e.to_string()),
        }
    }
}

impl From<SyncError> for TomeError {
    fn from(e: SyncError) -> Self {
        Self::Sync(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = TomeError::NotFound {
            resource: "source:abc".into(),
        };
        assert_eq!(error.to_string(), "Not found: source:abc");
    }

    #[test]
    fn test_error_conversion() {
        let storage_err = StorageError::NoHomeDirectory;
        let tome_err: TomeError = storage_err.into();
        assert!(matches!(tome_err, TomeError::Internal(_)));
    }
}
