# Database Migrations

SQLite schema migrations for Tome.

## Overview

Migrations are SQL files that modify the database schema. They run automatically on application startup in order.

## Naming Convention

```
YYYYMMDD_HHMMSS_description.sql
```

Examples:
- `20260101_000000_initial_schema.sql`
- `20260215_143000_add_collections.sql`
- `20260301_091500_add_sync_queue.sql`

## Writing Migrations

1. Create a new file with the naming convention above
2. Write **UP** migration only (no rollbacks in SQLite)
3. Use `IF NOT EXISTS` / `IF EXISTS` for idempotency
4. Test locally before committing

```sql
-- Migration: Add tags table
-- Description: Allows users to tag bookmarks

CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    color TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS bookmark_tags (
    bookmark_id TEXT NOT NULL REFERENCES bookmarks(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (bookmark_id, tag_id)
);

CREATE INDEX IF NOT EXISTS idx_bookmark_tags_tag ON bookmark_tags(tag_id);
```

## Schema Principles

1. **Use TEXT for IDs** - UUIDs stored as text for portability
2. **Use TEXT for dates** - ISO 8601 format (datetime('now'))
3. **Foreign keys** - Always define with ON DELETE behavior
4. **Indexes** - Add for frequently queried columns
5. **Defaults** - Provide sensible defaults where possible

## Testing Migrations

```bash
# Test migration on fresh database
rm ~/.tome/tome.db
cargo run --manifest-path src-tauri/Cargo.toml

# Verify schema
sqlite3 ~/.tome/tome.db ".schema"
```

## Troubleshooting

If a migration fails:
1. Check the error message in logs
2. Fix the SQL syntax
3. If database is corrupted, delete `~/.tome/tome.db` and restart

**Note:** In production, never modify existing migrations. Create a new migration to fix issues.
