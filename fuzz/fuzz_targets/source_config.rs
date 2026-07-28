//! The source config parser must not panic on any input.
//!
//! Config files are hand-edited by users and shipped by the registry; the
//! parser's contract is a helpful `Err`, never a crash. This matters more
//! than usual here because a config that crashes the parser crashes it on
//! every subsequent launch too — the file is already on disk.

#![no_main]

use std::path::Path;

use libfuzzer_sys::fuzz_target;
use tome_core::config::SourceConfig;
use tome_core::model::SourceId;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(id) = SourceId::new("fuzz") else {
        return;
    };
    if let Ok(config) = SourceConfig::parse_str(id, text, Path::new("fuzz.yaml")) {
        // Whatever parses must honour the invariants validation promises.
        assert!(config.fetch.rate_limit_rps > 0.0);
        assert!(config.fetch.rate_limit_rps <= tome_core::config::RATE_LIMIT_CAP_RPS);
        assert!(!config.fetch.timeout.is_zero());
        assert!(!config.name.trim().is_empty());
        if let Some(url) = config.spec.url() {
            assert!(url.scheme() == "https" || config.fetch.allow_insecure);
        }
    }
});
