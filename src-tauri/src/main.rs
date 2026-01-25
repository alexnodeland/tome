// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Strict linting - these are checked by clippy but also enforced at compile time
#![deny(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
// Allow some pedantic lints that are too noisy
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod commands;
mod config;
mod error;
mod parser;
mod scraper;
mod search;
mod storage;
mod sync;

use commands::*;
use storage::Database;

/// Application state shared across all Tauri commands
pub struct AppState {
    pub db: Database,
    // Add more shared state as needed:
    // pub fs: storage::FilesystemManager,
    // pub search: search::SearchEngine,
    // pub sync_manager: sync::SyncManager,
}

fn main() {
    // Initialize tracing/logging
    // In development, show all logs. In release, only show info and above.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            EnvFilter::new("tome=debug,tauri=info")
        } else {
            EnvFilter::new("tome=info,tauri=warn")
        }
    });

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    info!("Starting Tome v{}", env!("CARGO_PKG_VERSION"));

    // Build and run the Tauri application
    tauri::Builder::default()
        .setup(|app| {
            info!("Setting up application state");

            // Initialize database
            // In a real app, this would use the proper path from app.path_resolver()
            // let db = Database::open(&app_data_dir.join("tome.db"))?;

            // For now, use a placeholder
            // let state = AppState { db };
            // app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Source management
            // list_sources,
            // get_source,
            // add_source,
            // remove_source,
            // sync_source,
            //
            // Page reading
            // get_page,
            // render_page,
            //
            // Search
            // search,
            //
            // Bookmarks
            // list_bookmarks,
            // create_bookmark,
            // delete_bookmark,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    // Integration tests that test the full application setup
    // would go here. For unit tests, see individual module tests.
}
