-- Initial Schema Migration
-- Migration: 20260101_000000_initial_schema.sql
-- Creates the core tables for Tome

-- === Sources Table ===
-- Stores documentation sources (e.g., Rust std, Python docs)
CREATE TABLE IF NOT EXISTS sources (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    source_type TEXT NOT NULL CHECK (source_type IN ('readthedocs', 'rustdoc', 'mdbook', 'man', 'generic', 'local')),
    url TEXT,
    category TEXT,
    icon TEXT,
    accent_color TEXT,
    page_count INTEGER DEFAULT 0,
    sync_strategy TEXT NOT NULL DEFAULT 'manual' CHECK (sync_strategy IN ('manual', 'on_launch', 'scheduled', 'watch')),
    sync_schedule TEXT, -- cron expression for scheduled strategy
    version TEXT,
    pin_version INTEGER DEFAULT 0,
    last_synced_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- === Pages Table ===
-- Stores metadata for individual documentation pages
CREATE TABLE IF NOT EXISTS pages (
    id TEXT PRIMARY KEY NOT NULL, -- format: source_id:path
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    title TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    word_count INTEGER DEFAULT 0,
    last_modified TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(source_id, path)
);

CREATE INDEX IF NOT EXISTS idx_pages_source_id ON pages(source_id);
CREATE INDEX IF NOT EXISTS idx_pages_path ON pages(path);

-- === Bookmarks Table ===
-- User bookmarks for specific pages
CREATE TABLE IF NOT EXISTS bookmarks (
    id TEXT PRIMARY KEY NOT NULL,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    page_path TEXT NOT NULL,
    title TEXT NOT NULL,
    scroll_position REAL DEFAULT 0, -- percentage
    sync_status TEXT NOT NULL DEFAULT 'pending' CHECK (sync_status IN ('pending', 'synced', 'conflict')),
    cloudkit_record_id TEXT, -- iCloud record ID
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(source_id, page_path)
);

CREATE INDEX IF NOT EXISTS idx_bookmarks_source_id ON bookmarks(source_id);

-- === Highlights Table ===
-- Text highlights within pages
CREATE TABLE IF NOT EXISTS highlights (
    id TEXT PRIMARY KEY NOT NULL,
    bookmark_id TEXT NOT NULL REFERENCES bookmarks(id) ON DELETE CASCADE,
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    text_content TEXT NOT NULL, -- the highlighted text
    color TEXT NOT NULL DEFAULT 'yellow' CHECK (color IN ('yellow', 'green', 'blue', 'pink', 'purple')),
    note TEXT, -- optional annotation
    sync_status TEXT NOT NULL DEFAULT 'pending' CHECK (sync_status IN ('pending', 'synced', 'conflict')),
    cloudkit_record_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_highlights_bookmark_id ON highlights(bookmark_id);

-- === Collections Table ===
-- User-created bookmark collections
CREATE TABLE IF NOT EXISTS collections (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    icon TEXT,
    color TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    sync_status TEXT NOT NULL DEFAULT 'pending' CHECK (sync_status IN ('pending', 'synced', 'conflict')),
    cloudkit_record_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- === Bookmark-Collections Junction Table ===
CREATE TABLE IF NOT EXISTS bookmark_collections (
    bookmark_id TEXT NOT NULL REFERENCES bookmarks(id) ON DELETE CASCADE,
    collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (bookmark_id, collection_id)
);

-- === Reading Positions Table ===
-- Remembers last read position for each page
CREATE TABLE IF NOT EXISTS reading_positions (
    source_id TEXT NOT NULL,
    page_path TEXT NOT NULL,
    scroll_position REAL NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (source_id, page_path),
    FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE CASCADE
);

-- === Sync Queue Table ===
-- Queue for offline changes to sync to iCloud
CREATE TABLE IF NOT EXISTS sync_queue (
    id TEXT PRIMARY KEY NOT NULL,
    change_type TEXT NOT NULL CHECK (change_type IN ('create', 'update', 'delete')),
    entity_type TEXT NOT NULL CHECK (entity_type IN ('bookmark', 'highlight', 'collection')),
    entity_id TEXT NOT NULL,
    payload TEXT, -- JSON serialized change data
    retry_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_sync_queue_entity ON sync_queue(entity_type, entity_id);

-- === App Settings Table ===
-- Key-value store for application settings
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Insert default settings
INSERT OR IGNORE INTO settings (key, value) VALUES
    ('theme', '"system"'),
    ('font_size', '17'),
    ('line_height', '1.6'),
    ('icloud_sync_enabled', 'false'),
    ('api_enabled', 'false'),
    ('api_port', '7431');
