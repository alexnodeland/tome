//! The SQLite layer (implementation plan S1-2, tickets P1-004/P1-021).
//!
//! One database at [`Paths::database_file`](crate::Paths::database_file) —
//! `~/Library/Application Support/Tome/tome.db` — holding source and page
//! *metadata*. Page content lives on the filesystem under the cache root;
//! see `model/page.rs` for the split and why.
//!
//! Decisions worth knowing before touching this:
//!
//! - **STRICT tables.** SQLite's default column affinity silently stores a
//!   string in an INTEGER column; STRICT makes that the error it should be.
//!   Requires SQLite ≥ 3.37, which is why `rusqlite` is built with
//!   `bundled` rather than linking whatever macOS shipped.
//! - **`icon` and `sync` are stored as JSON in the frozen serde shape**
//!   (pinned by `tests/model_serde_shape.rs`). Flattening `sync` into
//!   columns would reintroduce in storage the invalid combinations the
//!   `SyncStrategy` enum makes unrepresentable, and neither field is
//!   queried by parts.
//! - **No connection pool, deliberately** — P1-004 says "connection pooling
//!   configured", and that criterion is met by not needing one: this is a
//!   local desktop database, WAL mode lets a reader (the CLI) coexist with
//!   the app's writer, and `busy_timeout` absorbs the rare collision. Each
//!   process opens one [`Database`]. A pool is a server pattern; carrying
//!   one here would be cargo cult.
//! - **`pages` is `WITHOUT ROWID`** with `(source_id, path)` as the primary
//!   key — the natural key from the frozen model. P1-004's sketch also had
//!   `CREATE INDEX idx_pages_source`; that index is *not* created, because
//!   the composite primary key already serves every prefix lookup on
//!   `source_id`, and a redundant index is write overhead pretending to be
//!   diligence.
//! - **`SyncState` is deliberately absent.** P1-004 listed it, but bookmark
//!   sync is deferred by ADR-0005; a speculative table shipped years early
//!   would only have to be migrated away. It arrives with S3.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{Error, Result};
use crate::model::{
    Attribution, ContentHash, Icon, Page, PagePath, Source, SourceId, SourceType, SyncConfig,
};
use crate::paths::Paths;

/// Schema migrations, applied in order; `PRAGMA user_version` records the
/// last one applied. Append-only: a released migration is never edited,
/// because a database that already ran it will not run it again.
const MIGRATIONS: &[&str] = &[
    // 0001 — sources and pages.
    "
    CREATE TABLE sources (
        id                    TEXT PRIMARY KEY,
        name                  TEXT NOT NULL,
        kind                  TEXT NOT NULL,
        url                   TEXT,
        local_path            TEXT,
        version               TEXT,
        category              TEXT NOT NULL,
        icon                  TEXT,
        accent_color          TEXT,
        attribution_homepage  TEXT,
        attribution_licence   TEXT,
        sync                  TEXT NOT NULL,
        created_at            TEXT NOT NULL,
        last_synced           TEXT,
        page_count            INTEGER NOT NULL DEFAULT 0,
        index_size_bytes      INTEGER NOT NULL DEFAULT 0
    ) STRICT;

    CREATE INDEX idx_sources_category ON sources(category);

    CREATE TABLE pages (
        source_id     TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
        path          TEXT NOT NULL,
        title         TEXT NOT NULL,
        content_hash  TEXT NOT NULL,
        fetched_at    TEXT NOT NULL,
        etag          TEXT,
        last_modified TEXT,
        PRIMARY KEY (source_id, path)
    ) STRICT, WITHOUT ROWID;
    ",
    // 0002 — navigation order.
    //
    // Pages were listed alphabetically by path, so the Cargo Book opened on
    // CHANGELOG.html and the Python tutorial on appendix.html — both of them
    // the first file by name and neither the first page of the document. The
    // crawler already visits pages in navigation order (it discovers links
    // from the whole document, and a documentation site advertises its pages
    // through its own contents list), and that order was being thrown away.
    //
    // Existing rows get 0, which sorts them together and falls back to path
    // — the previous behaviour, so an un-resynced library is unchanged rather
    // than scrambled.
    "
    ALTER TABLE pages ADD COLUMN ordinal INTEGER NOT NULL DEFAULT 0;
    ",
];

/// One open handle to the Tome database. Open one per process; see the
/// module docs for why there is no pool.
pub struct Database {
    conn: Connection,
}

/// The schema version a freshly-opened database ends up at.
///
/// Derived from [`MIGRATIONS`] rather than written down, so that adding a
/// migration cannot leave a stale literal behind — which is exactly what a
/// test asserting `== 1` did the first time a second migration appeared.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

impl Database {
    /// Open (creating and migrating if needed) the database for a library.
    pub fn open(paths: &Paths) -> Result<Self> {
        let file = paths.database_file();
        let db = Self::open_at(&file)?;
        // Reading history is in here; default umask would leave it
        // world-readable on a shared machine.
        #[cfg(unix)]
        crate::paths::restrict_file(&file)?;
        Ok(db)
    }

    /// Open at an explicit path. Tests use this with a tempdir; production
    /// code goes through [`Self::open`] so the location always comes from
    /// [`Paths`].
    pub fn open_at(file: &Path) -> Result<Self> {
        let conn = Connection::open(file).map_err(db_err)?;
        // WAL: the app writes while the CLI reads. NORMAL is durable enough
        // under WAL (a power cut loses at most the last transaction, never
        // consistency). busy_timeout absorbs writer collisions instead of
        // surfacing SQLITE_BUSY to the user.
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(db_err)?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(db_err)?;
        conn.pragma_update(None, "busy_timeout", 5000)
            .map_err(db_err)?;
        // OFF by default in SQLite for historical reasons; not optional here.
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(db_err)?;

        let mut db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&mut self) -> Result<()> {
        let applied: u32 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(db_err)?;

        for (index, sql) in MIGRATIONS.iter().enumerate().skip(applied as usize) {
            let version = index + 1;
            let tx = self.conn.transaction().map_err(db_err)?;
            tx.execute_batch(sql).map_err(db_err)?;
            // pragma_update can't take a bound parameter; the value is a
            // small integer we control.
            tx.pragma_update(None, "user_version", version as u32)
                .map_err(db_err)?;
            tx.commit().map_err(db_err)?;
            tracing::info!(version, "applied database migration");
        }
        Ok(())
    }

    /// The schema version currently applied, for diagnostics.
    pub fn schema_version(&self) -> Result<u32> {
        self.conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(db_err)
    }

    // ---- sources ---------------------------------------------------------

    /// Insert or fully replace a source. Replacement does NOT cascade to
    /// pages — `INSERT OR REPLACE` would (it deletes the row first, and the
    /// delete cascades), which is why this is an upsert with
    /// `ON CONFLICT DO UPDATE` instead. That distinction has eaten data in
    /// other codebases; do not "simplify" it back.
    pub fn upsert_source(&self, source: &Source) -> Result<()> {
        self.conn
            .execute(
                "
                INSERT INTO sources (
                    id, name, kind, url, local_path, version, category, icon,
                    accent_color, attribution_homepage, attribution_licence,
                    sync, created_at, last_synced, page_count, index_size_bytes
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    kind = excluded.kind,
                    url = excluded.url,
                    local_path = excluded.local_path,
                    version = excluded.version,
                    category = excluded.category,
                    icon = excluded.icon,
                    accent_color = excluded.accent_color,
                    attribution_homepage = excluded.attribution_homepage,
                    attribution_licence = excluded.attribution_licence,
                    sync = excluded.sync,
                    created_at = excluded.created_at,
                    last_synced = excluded.last_synced,
                    page_count = excluded.page_count,
                    index_size_bytes = excluded.index_size_bytes
                ",
                params![
                    source.id.as_str(),
                    source.name,
                    source.kind.as_str(),
                    source.url.as_ref().map(Url::as_str),
                    source
                        .local_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                    source.version,
                    source.category,
                    source.icon.as_ref().map(to_json).transpose()?,
                    source.accent_color,
                    source.attribution.homepage.as_ref().map(Url::as_str),
                    source.attribution.licence,
                    to_json(&source.sync)?,
                    source.created_at.to_rfc3339(),
                    source.last_synced.map(|t| t.to_rfc3339()),
                    source.page_count,
                    // SQLite INTEGER is i64; saturate rather than wrap for a
                    // size that will never legitimately reach 8 EB anyway.
                    i64::try_from(source.index_size_bytes).unwrap_or(i64::MAX),
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn get_source(&self, id: &SourceId) -> Result<Option<Source>> {
        self.conn
            .query_row(
                "SELECT * FROM sources WHERE id = ?1",
                params![id.as_str()],
                source_from_row,
            )
            .optional()
            .map_err(db_err)?
            .transpose()
    }

    /// All sources, ordered by name for stable display.
    pub fn list_sources(&self) -> Result<Vec<Source>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM sources ORDER BY name COLLATE NOCASE, id")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], source_from_row)
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        rows.into_iter().collect()
    }

    /// Delete a source; its pages go with it (FK cascade).
    pub fn delete_source(&self, id: &SourceId) -> Result<bool> {
        let deleted = self
            .conn
            .execute("DELETE FROM sources WHERE id = ?1", params![id.as_str()])
            .map_err(db_err)?;
        Ok(deleted > 0)
    }

    // ---- pages -----------------------------------------------------------

    /// Forget one page.
    ///
    /// **`pull` deliberately does not call this**, and that is worth stating
    /// where the method lives. It is tempting to prune every page a crawl did
    /// not revisit, but a crawl stops early for reasons that have nothing to
    /// do with the site: it hit `max_pages`, the network dropped, the laptop
    /// closed. Treating "not seen this run" as "deleted upstream" would then
    /// delete a user's library a few hundred pages at a time, and the
    /// content is the expensive half — hours of polite crawling — while the
    /// index that reads it is seconds to rebuild.
    ///
    /// **The policy is now agreed and is not yet implemented** (owner,
    /// 2026-07-29): prune pages not seen this run, but *only* when the crawl
    /// completed with **no errors and without hitting the page cap**. Any
    /// doubt at all and nothing is deleted; the next clean run will catch up.
    /// Whoever implements it owes a test that a capped or errored crawl
    /// deletes nothing — the guard is the entire point, and it is the half
    /// that fails silently.
    ///
    /// Until then this exists for explicit removal and for
    /// [`crate::pipeline::index_source`], which reconciles whatever the
    /// database actually holds.
    pub fn delete_page(&self, source: &SourceId, path: &PagePath) -> Result<bool> {
        let deleted = self
            .conn
            .execute(
                "DELETE FROM pages WHERE source_id = ?1 AND path = ?2",
                params![source.as_str(), path.as_str()],
            )
            .map_err(db_err)?;
        Ok(deleted > 0)
    }

    /// Store a page's metadata.
    ///
    /// `ordinal` is its position in the source's navigation order, which the
    /// crawler knows and [`Page`] deliberately does not: page *identity* is
    /// `(source, path)` (see `model/mod.rs`), and where a page sits in a
    /// contents list is a property of the source, not of the page. Keeping it
    /// in the row rather than in the model also leaves the frozen serde shape
    /// untouched.
    pub fn upsert_page(&self, page: &Page, ordinal: u32) -> Result<()> {
        self.conn
            .execute(
                "
                INSERT INTO pages (
                    source_id, path, title, content_hash, fetched_at, etag,
                    last_modified, ordinal
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(source_id, path) DO UPDATE SET
                    title = excluded.title,
                    content_hash = excluded.content_hash,
                    fetched_at = excluded.fetched_at,
                    etag = excluded.etag,
                    last_modified = excluded.last_modified,
                    ordinal = excluded.ordinal
                ",
                params![
                    page.source.as_str(),
                    page.path.as_str(),
                    page.title,
                    page.content_hash.as_str(),
                    page.fetched_at.to_rfc3339(),
                    page.etag,
                    page.last_modified,
                    ordinal,
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn get_page(&self, source: &SourceId, path: &PagePath) -> Result<Option<Page>> {
        self.conn
            .query_row(
                "SELECT * FROM pages WHERE source_id = ?1 AND path = ?2",
                params![source.as_str(), path.as_str()],
                page_from_row,
            )
            .optional()
            .map_err(db_err)?
            .transpose()
    }

    /// Every page of one source, in **navigation order** — the order the
    /// crawler met them, which for a documentation site is the order its own
    /// contents list advertises. Path is the tiebreak, so a library written
    /// before ordinals existed still lists deterministically.
    pub fn list_pages(&self, source: &SourceId) -> Result<Vec<Page>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM pages WHERE source_id = ?1 ORDER BY ordinal, path")
            .map_err(db_err)?;
        let rows = stmt
            .query_map(params![source.as_str()], page_from_row)
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        rows.into_iter().collect()
    }

    pub fn page_count(&self, source: &SourceId) -> Result<u32> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM pages WHERE source_id = ?1",
                params![source.as_str()],
                |row| row.get(0),
            )
            .map_err(db_err)
    }
}

// ---------------------------------------------------------------------------
// Row mapping. Every fallible conversion routes through FromSql-compatible
// closures returning rusqlite errors, so `query_row` composes; the model
// validation still runs (SourceId/PagePath/ContentHash re-validate on read,
// which means a corrupted or hand-edited row surfaces as an error, not as a
// hostile value flowing onwards).
// ---------------------------------------------------------------------------

use chrono::{DateTime, Utc};
use url::Url;

fn source_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<Source>> {
    // Two-layer Result: rusqlite's for column access (composes with
    // query_row/optional), ours for model validation of the values.
    let id: String = row.get("id")?;
    let name: String = row.get("name")?;
    let kind: String = row.get("kind")?;
    let url: Option<String> = row.get("url")?;
    let local_path: Option<String> = row.get("local_path")?;
    let version: Option<String> = row.get("version")?;
    let category: String = row.get("category")?;
    let icon: Option<String> = row.get("icon")?;
    let accent_color: Option<String> = row.get("accent_color")?;
    let attribution_homepage: Option<String> = row.get("attribution_homepage")?;
    let attribution_licence: Option<String> = row.get("attribution_licence")?;
    let sync: String = row.get("sync")?;
    let created_at: String = row.get("created_at")?;
    let last_synced: Option<String> = row.get("last_synced")?;
    let page_count: u32 = row.get("page_count")?;
    let index_size_bytes: i64 = row.get("index_size_bytes")?;

    Ok((|| {
        let mut source = Source::new(SourceId::new(id)?, name, parse_kind(&kind)?);
        source.url = url.map(|u| parse_url(&u)).transpose()?;
        source.local_path = local_path.map(Into::into);
        source.version = version;
        source.category = category;
        source.icon = icon.map(|i| from_json::<Icon>(&i)).transpose()?;
        source.accent_color = accent_color;
        source.attribution = Attribution {
            homepage: attribution_homepage.map(|u| parse_url(&u)).transpose()?,
            licence: attribution_licence,
        };
        source.sync = from_json::<SyncConfig>(&sync)?;
        source.created_at = parse_time(&created_at)?;
        source.last_synced = last_synced.map(|t| parse_time(&t)).transpose()?;
        source.page_count = page_count;
        // A negative size can only mean a hand-edited row; clamp to zero
        // rather than failing the whole listing over a display statistic.
        source.index_size_bytes = u64::try_from(index_size_bytes).unwrap_or(0);
        Ok(source)
    })())
}

fn page_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<Page>> {
    let source_id: String = row.get("source_id")?;
    let path: String = row.get("path")?;
    let title: String = row.get("title")?;
    let content_hash: String = row.get("content_hash")?;
    let fetched_at: String = row.get("fetched_at")?;
    let etag: Option<String> = row.get("etag")?;
    let last_modified: Option<String> = row.get("last_modified")?;

    Ok((|| {
        let mut page = Page::new(
            SourceId::new(source_id)?,
            PagePath::new(path)?,
            title,
            ContentHash::new(content_hash)?,
        );
        page.fetched_at = parse_time(&fetched_at)?;
        page.etag = etag;
        page.last_modified = last_modified;
        Ok(page)
    })())
}

fn parse_kind(kind: &str) -> Result<SourceType> {
    match kind {
        "readthedocs" => Ok(SourceType::ReadTheDocs),
        "rustdoc" => Ok(SourceType::Rustdoc),
        "mdbook" => Ok(SourceType::MdBook),
        "man" => Ok(SourceType::Man),
        "generic" => Ok(SourceType::Generic),
        "local" => Ok(SourceType::Local),
        "docset" => Ok(SourceType::Docset),
        other => Err(Error::Database {
            message: format!("unknown source kind {other:?} in the database"),
        }),
    }
}

fn parse_url(text: &str) -> Result<Url> {
    Url::parse(text).map_err(|e| Error::Database {
        message: format!("invalid URL in the database: {e}"),
    })
}

fn parse_time(text: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| Error::Database {
            message: format!("invalid timestamp in the database: {e}"),
        })
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|e| Error::Database {
        message: format!("could not encode a record: {e}"),
    })
}

fn from_json<T: serde::de::DeserializeOwned>(text: &str) -> Result<T> {
    serde_json::from_str(text).map_err(|e| Error::Database {
        message: format!("could not decode a stored record: {e}"),
    })
}

fn db_err(e: rusqlite::Error) -> Error {
    Error::Database {
        message: e.to_string(),
    }
}
