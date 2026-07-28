//! The database layer (S1-2): round-trip fidelity, cascade behaviour,
//! migrations, and the pragmas that are load-bearing rather than decorative.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use chrono::{DateTime, Utc};
use tome_core::db::Database;
use tome_core::model::{
    Attribution, ContentHash, Icon, Page, PagePath, Schedule, Source, SourceId, SourceType,
    SyncConfig, SyncStrategy,
};
use tome_core::{Error, Paths};

fn sid(id: &str) -> SourceId {
    SourceId::new(id).unwrap()
}

fn open_temp() -> (tempfile::TempDir, Database) {
    let tmp = tempfile::tempdir().unwrap();
    let db = Database::open_at(&tmp.path().join("tome.db")).unwrap();
    (tmp, db)
}

/// A source with every optional field set — round-trip fidelity means
/// nothing if the fixture leaves half the columns NULL.
fn full_source() -> Source {
    let mut source = Source::new(sid("python"), "Python 3.13", SourceType::ReadTheDocs);
    source.url = Some("https://docs.python.org/3/".parse().unwrap());
    source.local_path = Some("/opt/docs".into());
    source.version = Some("3.13".into());
    source.category = "Languages".into();
    source.icon = Some(Icon::Emoji("🐍".into()));
    source.accent_color = Some("#3776AB".into());
    source.attribution = Attribution {
        homepage: Some("https://www.python.org/".parse().unwrap()),
        licence: Some("PSF-2.0".into()),
    };
    source.sync = SyncConfig {
        strategy: SyncStrategy::Scheduled {
            schedule: Schedule::Weekly,
        },
        pin_version: true,
    };
    source.last_synced = Some(fixed("2026-07-28T15:30:00.123456Z"));
    source.created_at = fixed("2026-07-01T09:00:00Z");
    source.page_count = 412;
    source.index_size_bytes = 9_876_543;
    source
}

fn full_page() -> Page {
    let mut page = Page::new(
        sid("python"),
        PagePath::new("library/os.html").unwrap(),
        "os — Miscellaneous operating system interfaces",
        ContentHash::from_digest([0x2A; 32]),
    );
    page.fetched_at = fixed("2026-07-28T15:31:07.5Z");
    page.etag = Some("\"66a1-python\"".into());
    page.last_modified = Some("Mon, 28 Jul 2026 12:00:00 GMT".into());
    page
}

fn fixed(rfc3339: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(rfc3339)
        .unwrap()
        .with_timezone(&Utc)
}

#[test]
fn source_round_trips_every_field() {
    let (_tmp, db) = open_temp();
    let source = full_source();

    db.upsert_source(&source).unwrap();
    let loaded = db
        .get_source(&sid("python"))
        .unwrap()
        .expect("source exists");

    // Struct equality covers every field at once — including the JSON
    // columns and the sub-second timestamps, which are where fidelity
    // usually dies quietly.
    assert_eq!(loaded, source);
}

#[test]
fn page_round_trips_every_field() {
    let (_tmp, db) = open_temp();
    db.upsert_source(&full_source()).unwrap();

    let page = full_page();
    db.upsert_page(&page).unwrap();
    let loaded = db
        .get_page(&sid("python"), &PagePath::new("library/os.html").unwrap())
        .unwrap()
        .expect("page exists");

    assert_eq!(loaded, page);
}

#[test]
fn upsert_source_updates_without_cascading_to_pages() {
    // INSERT OR REPLACE would delete the row first and the delete would
    // cascade — silently emptying the source's pages on every metadata
    // update. This asserts the upsert is a real update.
    let (_tmp, db) = open_temp();
    let mut source = full_source();
    db.upsert_source(&source).unwrap();
    db.upsert_page(&full_page()).unwrap();

    source.name = "Python 3.13.1".into();
    source.page_count = 413;
    db.upsert_source(&source).unwrap();

    assert_eq!(
        db.page_count(&sid("python")).unwrap(),
        1,
        "pages must survive"
    );
    assert_eq!(
        db.get_source(&sid("python")).unwrap().unwrap().name,
        "Python 3.13.1"
    );
}

#[test]
fn upsert_page_replaces_on_the_natural_key() {
    let (_tmp, db) = open_temp();
    db.upsert_source(&full_source()).unwrap();

    let mut page = full_page();
    db.upsert_page(&page).unwrap();
    page.title = "os — updated".into();
    page.content_hash = ContentHash::from_digest([0x2B; 32]);
    db.upsert_page(&page).unwrap();

    assert_eq!(db.page_count(&sid("python")).unwrap(), 1);
    assert_eq!(
        db.get_page(&page.source, &page.path)
            .unwrap()
            .unwrap()
            .title,
        "os — updated"
    );
}

#[test]
fn deleting_a_source_cascades_to_its_pages() {
    let (_tmp, db) = open_temp();
    db.upsert_source(&full_source()).unwrap();
    db.upsert_page(&full_page()).unwrap();

    assert!(db.delete_source(&sid("python")).unwrap());
    assert_eq!(db.page_count(&sid("python")).unwrap(), 0);
    assert!(db.get_source(&sid("python")).unwrap().is_none());
    // Deleting again reports "nothing to delete" rather than erroring.
    assert!(!db.delete_source(&sid("python")).unwrap());
}

#[test]
fn a_page_without_its_source_is_refused() {
    // foreign_keys is OFF by default in SQLite; this test is what notices
    // if the pragma ever stops being applied.
    let (_tmp, db) = open_temp();
    let err = db.upsert_page(&full_page()).unwrap_err();
    assert!(matches!(err, Error::Database { .. }));
}

#[test]
fn list_sources_orders_by_name_case_insensitively() {
    let (_tmp, db) = open_temp();
    for (id, name) in [("b", "beta"), ("a", "Alpha"), ("c", "Charlie")] {
        db.upsert_source(&Source::new(sid(id), name, SourceType::Generic))
            .unwrap();
    }
    let names: Vec<String> = db
        .list_sources()
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, ["Alpha", "beta", "Charlie"]);
}

#[test]
fn list_pages_orders_by_path() {
    let (_tmp, db) = open_temp();
    db.upsert_source(&full_source()).unwrap();
    for path in ["z.html", "a.html", "m/n.html"] {
        let mut page = full_page();
        page.path = PagePath::new(path).unwrap();
        db.upsert_page(&page).unwrap();
    }
    let paths: Vec<String> = db
        .list_pages(&sid("python"))
        .unwrap()
        .into_iter()
        .map(|p| p.path.as_str().to_owned())
        .collect();
    assert_eq!(paths, ["a.html", "m/n.html", "z.html"]);
}

#[test]
fn reopening_does_not_rerun_migrations_and_keeps_data() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("tome.db");

    let db = Database::open_at(&file).unwrap();
    assert_eq!(db.schema_version().unwrap(), 1);
    db.upsert_source(&full_source()).unwrap();
    drop(db);

    let db = Database::open_at(&file).unwrap();
    assert_eq!(db.schema_version().unwrap(), 1);
    assert!(db.get_source(&sid("python")).unwrap().is_some());
}

#[test]
fn open_creates_the_file_where_paths_says_and_owner_only() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = Paths::under_root(tmp.path());
    paths.ensure_created().unwrap();

    let _db = Database::open(&paths).unwrap();
    let file = paths.database_file();
    assert!(
        file.exists(),
        "database file must be at Paths::database_file()"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "reading history must not be world-readable");
    }
}

#[test]
fn corrupted_rows_surface_as_errors_not_hostile_values() {
    // A row edited outside Tome (or corrupted) must fail model validation
    // on read — the DB is not a side door around SourceId/PagePath rules.
    let (tmp, db) = open_temp();
    drop(db);
    let conn = rusqlite::Connection::open(tmp.path().join("tome.db")).unwrap();
    conn.execute(
        "INSERT INTO sources (id, name, kind, category, sync, created_at)
         VALUES ('../etc', 'X', 'generic', 'c', '{\"strategy\":{\"strategy\":\"manual\"},\"pin_version\":false}', '2026-07-28T12:00:00Z')",
        [],
    )
    .unwrap();
    drop(conn);

    let db = Database::open_at(&tmp.path().join("tome.db")).unwrap();
    let result = db.list_sources();
    assert!(matches!(result, Err(Error::InvalidSourceId { .. })));
}
