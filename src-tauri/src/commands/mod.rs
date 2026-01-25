//! Tauri command handlers.
//!
//! This module contains all the IPC command handlers that bridge
//! the Rust backend with the JavaScript frontend.
//!
//! ## Design Principles
//!
//! 1. Commands are thin wrappers - business logic lives in domain modules
//! 2. All errors are converted to String for IPC compatibility
//! 3. Commands use `State<AppState>` for shared application state
//! 4. Async commands are preferred for any I/O operations
//!
//! ## Adding a New Command
//!
//! 1. Create a function with `#[tauri::command]` attribute
//! 2. Add to `generate_handler![]` in main.rs
//! 3. Create corresponding TypeScript wrapper in `src/lib/services/`

// Placeholder modules - uncomment and implement as needed
// mod source_commands;
// mod page_commands;
// mod search_commands;
// mod bookmark_commands;

// Re-export all commands
// pub use source_commands::*;
// pub use page_commands::*;
// pub use search_commands::*;
// pub use bookmark_commands::*;

// Example command for testing the setup
use tauri::command;

/// Health check command for testing IPC
#[command]
pub fn health_check() -> String {
    "Tome backend is running".to_string()
}

/// Get application version
#[command]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check() {
        let result = health_check();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_get_version() {
        let version = get_version();
        assert!(!version.is_empty());
    }
}
