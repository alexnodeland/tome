//! robots.txt parsing and matching must not panic on any input.
//!
//! robots.txt is attacker-supplied by definition — any host Tome fetches
//! from controls it. The two failure classes this hunts: a panic in the
//! parser (multi-byte truncation, weird lines), and pathological wildcard
//! rules turning the matcher exponential (the input includes both the rule
//! set and a path to match against it, so the matcher runs under the
//! fuzzer's timeout).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tome_core::fetch::robots::RobotsPolicy;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let (body, path) = match text.split_once('\0') {
        Some((body, path)) => (body, path),
        None => (text, "/index.html"),
    };

    let policy = RobotsPolicy::parse(body);
    let _ = policy.allows(path);
    let _ = policy.crawl_delay();

    // Invariants that hold whatever the input:
    assert!(policy.allows("/robots.txt"), "/robots.txt is always fetchable");
    assert!(RobotsPolicy::allow_all().allows(path));
    // disallow_all's rule is `Disallow: /` — a prefix rule, so it binds
    // exactly the well-formed (slash-leading) paths a URL can produce.
    if path.starts_with('/') && path != "/robots.txt" {
        assert!(!RobotsPolicy::disallow_all().allows(path));
    }
});
