# Commands Directory

Tauri command handlers - the IPC boundary between Rust and JavaScript.

## What Belongs Here

- **Command handlers** marked with `#[tauri::command]`
- **Thin wrappers** that delegate to domain modules
- **Error conversion** from domain errors to IPC-friendly strings

## What Does NOT Belong Here

- Business logic (delegate to domain modules)
- Database queries (use `storage/`)
- HTTP requests (use `scraper/`)
- Complex algorithms (use appropriate domain module)

## Naming Conventions

- Command files: `snake_case.rs` (e.g., `source_commands.rs`)
- Command functions: `snake_case` (e.g., `fn list_sources`)
- Keep command names identical to JS `invoke` calls

## Command Pattern

```rust
// source_commands.rs
use tauri::State;
use crate::{
    config::SourceConfig,
    error::TomeError,
    storage::Database,
    AppState,
};
use serde::{Deserialize, Serialize};

/// Response types should be defined here if command-specific
#[derive(Debug, Serialize)]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
    pub page_count: i64,
    pub last_synced: Option<String>,
}

/// List all documentation sources
///
/// # Errors
/// Returns error string if database query fails
#[tauri::command]
pub async fn list_sources(
    state: State<'_, AppState>,
) -> Result<Vec<SourceSummary>, String> {
    state
        .db
        .list_sources()
        .await
        .map(|sources| sources.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

/// Get a single source by ID
///
/// # Arguments
/// * `id` - Source identifier
///
/// # Errors
/// Returns error if source not found or database error
#[tauri::command]
pub async fn get_source(
    id: String,
    state: State<'_, AppState>,
) -> Result<Source, String> {
    state
        .db
        .get_source(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Source not found: {}", id))
}

/// Add a new documentation source
///
/// # Arguments
/// * `config` - Source configuration
///
/// # Errors
/// Returns error if validation fails or database error
#[tauri::command]
pub async fn add_source(
    config: SourceConfig,
    state: State<'_, AppState>,
) -> Result<Source, String> {
    // Validate configuration
    config.validate().map_err(|e| e.to_string())?;

    // Create source from config
    let source = Source::from_config(config);

    // Store in database
    state
        .db
        .insert_source(&source)
        .await
        .map_err(|e| e.to_string())?;

    Ok(source)
}

/// Remove a documentation source
///
/// # Arguments
/// * `id` - Source identifier to remove
///
/// # Errors
/// Returns error if source not found or database error
#[tauri::command]
pub async fn remove_source(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Remove from database (cascades to pages)
    state
        .db
        .delete_source(&id)
        .await
        .map_err(|e| e.to_string())?;

    // Clean up filesystem storage
    state
        .fs
        .remove_source_data(&id)
        .await
        .map_err(|e| e.to_string())?;

    // Remove from search index
    state
        .search
        .remove_source(&id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Trigger sync for a source
///
/// # Arguments
/// * `id` - Source identifier
/// * `force` - Force full re-sync even if unchanged
///
/// # Errors
/// Returns error if sync fails
#[tauri::command]
pub async fn sync_source(
    id: String,
    force: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .sync_manager
        .sync(&id, force.unwrap_or(false))
        .await
        .map_err(|e| e.to_string())
}
```

## Module Registration

```rust
// mod.rs
mod source_commands;
mod page_commands;
mod search_commands;
mod bookmark_commands;

pub use source_commands::*;
pub use page_commands::*;
pub use search_commands::*;
pub use bookmark_commands::*;

// In main.rs, register all commands:
// tauri::Builder::default()
//     .invoke_handler(tauri::generate_handler![
//         list_sources,
//         get_source,
//         add_source,
//         remove_source,
//         sync_source,
//         // ... more commands
//     ])
```

## Testing Pattern

```rust
// tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_state;

    #[tokio::test]
    async fn test_list_sources_empty() {
        let state = create_test_state().await;
        let result = list_sources(state.into()).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_add_and_get_source() {
        let state = create_test_state().await;
        let config = SourceConfig {
            name: "Test".into(),
            // ...
        };

        let added = add_source(config, state.clone().into()).await.unwrap();
        let retrieved = get_source(added.id.clone(), state.into()).await.unwrap();

        assert_eq!(added.id, retrieved.id);
        assert_eq!(added.name, retrieved.name);
    }
}
```

## Architectural Rules

1. Commands **can import from** all other modules (they orchestrate)
2. Commands must be **thin** - maximum 20-30 lines
3. Commands must convert **all errors to String** for IPC
4. Commands must be **async** for non-trivial operations
5. Commands should **not contain business logic** - delegate
6. Document all commands with **doc comments** explaining args and errors
