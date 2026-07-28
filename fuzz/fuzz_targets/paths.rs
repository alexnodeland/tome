//! Path resolution must not panic on any input.
//!
//! Source ids come from configuration files, which come from the registry or
//! from a user pasting a URL, and a `Paths` accessor that panics on a strange
//! id takes the app down on launch — after the id has already been persisted,
//! so it takes it down on every subsequent launch too.
//!
//! Note what this target does *not* claim: it does not assert that a hostile
//! id stays inside the cache directory. It does not, today —
//! `pages_dir("../../etc")` escapes lexically, and containment is the job of
//! the path validation in S1. When that lands, the assertion belongs here.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tome_core::Paths;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    let (root, source_id) = match text.split_once('\0') {
        Some((root, id)) => (root, id),
        None => ("/tmp/tome-fuzz", text),
    };

    let paths = Paths::under_root(root);

    // Every accessor, because any of them could be the one that panics.
    let _ = paths.state_root();
    let _ = paths.cache_root();
    let _ = paths.config_file();
    let _ = paths.sources_dir();
    let _ = paths.source_config_file(source_id);
    let _ = paths.database_file();
    let _ = paths.logs_dir();
    let _ = paths.index_dir();
    let _ = paths.source_data_dir(source_id);
    let _ = paths.pages_dir(source_id);
    let _ = paths.raw_dir(source_id);
    let _ = paths.assets_dir(source_id);
});
