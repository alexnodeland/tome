# Storage Directory

Database and filesystem operations.

## What Belongs Here

- **SQLite database** operations and migrations
- **Filesystem** management for document storage
- **Repository patterns** for data access
- **Migration** definitions and runner

## What Does NOT Belong Here

- Business logic (use domain modules)
- HTTP operations (use `scraper/`)
- Search indexing (use `search/`)
- Command handlers (use `commands/`)

## Module Structure

```
storage/
├── mod.rs              # Module exports
├── database.rs         # SQLite connection management
├── migrations/         # SQL migration files
│   ├── mod.rs
│   ├── 001_initial.sql
│   └── 002_bookmarks.sql
├── repositories/       # Data access patterns
│   ├── mod.rs
│   ├── sources.rs
│   ├── pages.rs
│   └── bookmarks.rs
├── filesystem.rs       # File storage management
└── tests.rs            # Unit tests
```

## Database Schema

```sql
-- migrations/001_initial.sql

-- Sources table: documentation origins
CREATE TABLE IF NOT EXISTS sources (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source_type TEXT NOT NULL,
    url TEXT,
    local_path TEXT,
    version TEXT,
    category TEXT DEFAULT 'Uncategorized',
    icon TEXT,
    accent_color TEXT,
    sync_strategy TEXT DEFAULT 'manual',
    sync_schedule TEXT,
    pin_version INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    last_synced_at TEXT,
    page_count INTEGER DEFAULT 0,
    index_size_bytes INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_sources_category ON sources(category);

-- Pages table: individual documentation pages
CREATE TABLE IF NOT EXISTS pages (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    title TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    last_modified TEXT NOT NULL,
    UNIQUE(source_id, path)
);

CREATE INDEX IF NOT EXISTS idx_pages_source ON pages(source_id);
CREATE INDEX IF NOT EXISTS idx_pages_path ON pages(source_id, path);
```

## Repository Pattern

```rust
// repositories/sources.rs
use rusqlite::{params, Connection, Result as SqlResult};
use crate::error::StorageError;

pub struct SourceRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SourceRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// List all sources
    pub fn list(&self) -> Result<Vec<Source>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, source_type, url, category, page_count, last_synced_at
             FROM sources
             ORDER BY category, name"
        )?;

        let sources = stmt.query_map([], |row| {
            Ok(Source {
                id: row.get(0)?,
                name: row.get(1)?,
                source_type: row.get(2)?,
                url: row.get(3)?,
                category: row.get(4)?,
                page_count: row.get(5)?,
                last_synced_at: row.get(6)?,
            })
        })?;

        sources.collect::<SqlResult<Vec<_>>>().map_err(Into::into)
    }

    /// Get source by ID
    pub fn get(&self, id: &str) -> Result<Option<Source>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, source_type, url, category, page_count, last_synced_at
             FROM sources
             WHERE id = ?"
        )?;

        let source = stmt.query_row(params![id], |row| {
            Ok(Source {
                id: row.get(0)?,
                name: row.get(1)?,
                source_type: row.get(2)?,
                url: row.get(3)?,
                category: row.get(4)?,
                page_count: row.get(5)?,
                last_synced_at: row.get(6)?,
            })
        });

        match source {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Insert a new source
    pub fn insert(&self, source: &Source) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO sources (id, name, source_type, url, category, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                source.id,
                source.name,
                source.source_type,
                source.url,
                source.category,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Update source metadata
    pub fn update(&self, id: &str, updates: &SourceUpdate) -> Result<(), StorageError> {
        let mut sql = String::from("UPDATE sources SET ");
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];
        let mut first = true;

        if let Some(name) = &updates.name {
            if !first { sql.push_str(", "); }
            sql.push_str("name = ?");
            params.push(Box::new(name.clone()));
            first = false;
        }

        if let Some(page_count) = updates.page_count {
            if !first { sql.push_str(", "); }
            sql.push_str("page_count = ?");
            params.push(Box::new(page_count));
            first = false;
        }

        if let Some(last_synced) = &updates.last_synced_at {
            if !first { sql.push_str(", "); }
            sql.push_str("last_synced_at = ?");
            params.push(Box::new(last_synced.clone()));
        }

        sql.push_str(" WHERE id = ?");
        params.push(Box::new(id.to_string()));

        self.conn.execute(&sql, rusqlite::params_from_iter(params.iter()))?;
        Ok(())
    }

    /// Delete a source and all its pages
    pub fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let count = self.conn.execute(
            "DELETE FROM sources WHERE id = ?",
            params![id],
        )?;
        Ok(count > 0)
    }
}
```

## Filesystem Manager

```rust
// filesystem.rs
use std::path::{Path, PathBuf};
use tokio::fs;
use crate::error::StorageError;

/// Manages the ~/.tome directory structure
pub struct FilesystemManager {
    base_path: PathBuf,
}

impl FilesystemManager {
    /// Create manager with default path (~/.tome)
    pub fn new() -> Result<Self, StorageError> {
        let base_path = dirs::home_dir()
            .ok_or(StorageError::NoHomeDirectory)?
            .join(".tome");

        Ok(Self { base_path })
    }

    /// Create with custom base path (for testing)
    pub fn with_path(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Initialize directory structure
    pub async fn init(&self) -> Result<(), StorageError> {
        // Create all required directories
        for dir in &["sources", "data", "index"] {
            fs::create_dir_all(self.base_path.join(dir)).await?;
        }
        Ok(())
    }

    /// Get path to source data directory
    pub fn source_data_path(&self, source_id: &str) -> PathBuf {
        self.base_path.join("data").join(source_id)
    }

    /// Get path to a specific page file
    pub fn page_path(&self, source_id: &str, page_path: &str) -> PathBuf {
        self.source_data_path(source_id)
            .join("pages")
            .join(format!("{}.html", sanitize_path(page_path)))
    }

    /// Store page content
    pub async fn store_page(
        &self,
        source_id: &str,
        page_path: &str,
        content: &str,
    ) -> Result<(), StorageError> {
        let path = self.page_path(source_id, page_path);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&path, content).await?;
        Ok(())
    }

    /// Load page content
    pub async fn load_page(
        &self,
        source_id: &str,
        page_path: &str,
    ) -> Result<String, StorageError> {
        let path = self.page_path(source_id, page_path);
        fs::read_to_string(&path).await.map_err(Into::into)
    }

    /// Remove all data for a source
    pub async fn remove_source_data(&self, source_id: &str) -> Result<(), StorageError> {
        let path = self.source_data_path(source_id);
        if path.exists() {
            fs::remove_dir_all(&path).await?;
        }
        Ok(())
    }
}

/// Sanitize a path for use as filename
fn sanitize_path(path: &str) -> String {
    path.replace('/', "_")
        .replace('\\', "_")
        .replace(':', "_")
        .replace('?', "_")
}
```

## Testing Pattern

```rust
// tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_db() -> (Connection, TempDir) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = Connection::open(&db_path).unwrap();

        // Run migrations
        Database::run_migrations(&conn).unwrap();

        (conn, dir)
    }

    #[test]
    fn test_source_crud() {
        let (conn, _dir) = create_test_db();
        let repo = SourceRepository::new(&conn);

        // Create
        let source = Source {
            id: "test-1".into(),
            name: "Test Source".into(),
            source_type: "generic".into(),
            url: Some("https://example.com".into()),
            category: "Testing".into(),
            page_count: 0,
            last_synced_at: None,
        };

        repo.insert(&source).unwrap();

        // Read
        let retrieved = repo.get("test-1").unwrap().unwrap();
        assert_eq!(retrieved.name, "Test Source");

        // Update
        repo.update("test-1", &SourceUpdate {
            page_count: Some(42),
            ..Default::default()
        }).unwrap();

        let updated = repo.get("test-1").unwrap().unwrap();
        assert_eq!(updated.page_count, 42);

        // Delete
        assert!(repo.delete("test-1").unwrap());
        assert!(repo.get("test-1").unwrap().is_none());
    }

    #[tokio::test]
    async fn test_filesystem_manager() {
        let dir = TempDir::new().unwrap();
        let fs = FilesystemManager::with_path(dir.path().to_path_buf());

        fs.init().await.unwrap();

        // Store and load page
        fs.store_page("source-1", "/docs/intro", "<h1>Hello</h1>").await.unwrap();
        let content = fs.load_page("source-1", "/docs/intro").await.unwrap();

        assert_eq!(content, "<h1>Hello</h1>");
    }
}
```

## Architectural Rules

1. Storage **cannot import from** `commands/` (data layer only)
2. Storage **can import from** `config/`, `error.rs`
3. All database operations must use **prepared statements**
4. All filesystem operations must be **async**
5. Use **transactions** for multi-step operations
6. Always use **tempdir** for tests (never touch real ~/.tome)
