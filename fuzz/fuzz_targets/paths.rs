//! Path resolution must not panic on any input — and, since the accessors
//! moved to `&SourceId`, whatever constructs must stay contained.
//!
//! Source ids come from configuration files, which come from the registry or
//! from a user pasting a URL, and a `Paths` accessor that panics on a strange
//! id takes the app down on launch — after the id has already been persisted,
//! so it takes it down on every subsequent launch too.
//!
//! The S0 version of this target carried a note that it did *not* assert a
//! hostile id stays inside the cache directory, because it didn't hold: the
//! accessors took `&str` and `pages_dir("../../etc")` escaped lexically. The
//! accessors now take `SourceId`, so the assertion this target was waiting
//! for is here: an id either fails construction or resolves to exactly one
//! path component under the right root.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tome_core::model::SourceId;
use tome_core::Paths;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    let (root, raw_id) = match text.split_once('\0') {
        Some((root, id)) => (root, id),
        None => ("/tmp/tome-fuzz", text),
    };

    let paths = Paths::under_root(root);

    // Accessors that take no id: must never panic, whatever the root.
    let _ = paths.state_root();
    let _ = paths.cache_root();
    let _ = paths.config_file();
    let _ = paths.sources_dir();
    let _ = paths.database_file();
    let _ = paths.logs_dir();
    let _ = paths.index_dir();

    // Id-taking accessors are only reachable through SourceId. Construction
    // must never panic; every id that constructs must stay contained.
    let Ok(id) = SourceId::new(raw_id) else {
        return;
    };

    let config = paths.source_config_file(&id);
    assert!(config.starts_with(paths.sources_dir()));

    let data_dir = paths.source_data_dir(&id);
    assert!(data_dir.starts_with(paths.cache_root()));
    // Exactly one component below <cache>/data — an id that added or removed
    // components could collide with a sibling source or climb the tree.
    let below: Vec<_> = data_dir
        .strip_prefix(paths.cache_root().join("data"))
        .expect("data dir must sit under <cache>/data")
        .components()
        .collect();
    assert_eq!(below.len(), 1);

    for dir in [paths.pages_dir(&id), paths.raw_dir(&id), paths.assets_dir(&id)] {
        assert!(dir.starts_with(&data_dir));
    }
});
