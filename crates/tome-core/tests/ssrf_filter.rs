//! The SSRF filter at the fetcher boundary (S1-5).
//!
//! The address classifier's exhaustive unit tests live in `fetch/ssrf.rs`;
//! these assert the *fetcher* actually refuses forbidden destinations, over
//! a real socket where it can, and that `allow_insecure` widens the policy
//! exactly as far as intended and no further.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::time::Duration;

use tome_core::config::FetchConfig;
use tome_core::fetch::Fetcher;
use tome_core::Error;
use tome_testkit::FixtureServer;
use url::Url;

const MB: u64 = 1024 * 1024;

fn fetcher(allow_insecure: bool) -> Fetcher {
    let config = FetchConfig {
        rate_limit_rps: 1000.0,
        allow_insecure,
        // Short timeout: one test deliberately targets a private address
        // with nothing listening, and the default 30s timeout would make
        // the suite crawl. The filter decisions themselves are instant.
        timeout: Duration::from_millis(400),
        ..FetchConfig::default()
    };
    Fetcher::with_backoff_base(config, Duration::from_millis(5))
}

// ---- literal-IP URLs, rejected before any connection -----------------------

#[test]
fn literal_metadata_and_loopback_ips_are_blocked_by_default() {
    let f = fetcher(false);
    for host in [
        "http://169.254.169.254/latest/meta-data/", // cloud metadata
        "http://127.0.0.1/admin",                   // localhost
        "http://[::1]/admin",                       // v6 loopback
        "http://10.0.0.1/",                         // RFC1918
        "http://[::ffff:127.0.0.1]/",               // v4-mapped loopback
        "http://[::ffff:169.254.169.254]/",         // v4-mapped metadata
    ] {
        let url: Url = host.parse().unwrap();
        let err = f.fetch(&url, MB, None).unwrap_err();
        assert!(
            matches!(err, Error::BlockedByFilter),
            "{host} should be blocked by the filter, got {err:?}"
        );
    }
}

#[test]
fn allow_insecure_opens_loopback_and_private_but_never_link_local() {
    let f = fetcher(true);

    // Link-local (metadata) stays blocked even with allow_insecure — the
    // metadata endpoint is not a documentation host under any config.
    for forbidden in [
        "http://169.254.169.254/",
        "http://[fe80::1]/",
        "http://[::ffff:169.254.169.254]/",
    ] {
        let url: Url = forbidden.parse().unwrap();
        assert!(
            matches!(f.fetch(&url, MB, None).unwrap_err(), Error::BlockedByFilter),
            "{forbidden} must stay blocked even with allow_insecure"
        );
    }

    // Loopback/private no longer fail the *filter* — they fail later (there
    // is nothing listening), which is a different, non-filter error.
    let private: Url = "http://10.255.255.1/".parse().unwrap();
    let err = f.fetch(&private, MB, None).unwrap_err();
    assert!(
        !matches!(err, Error::BlockedByFilter),
        "allow_insecure should let the private address past the filter, got {err:?}"
    );
}

// ---- the fixture server (loopback) -----------------------------------------

#[test]
fn loopback_fixture_is_blocked_by_default_and_reachable_with_allow_insecure() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let url: Url = server.url_for("/index.html").parse().unwrap();

    // Default policy: the loopback fixture is refused before any request.
    let strict = fetcher(false);
    assert!(matches!(
        strict.fetch(&url, MB, None).unwrap_err(),
        Error::BlockedByFilter
    ));
    assert!(
        server.request_count() == 0,
        "a blocked host must never be contacted"
    );

    // allow_insecure: the same fixture is reachable (owned host).
    let relaxed = fetcher(true);
    let outcome = relaxed.fetch(&url, MB, None).unwrap();
    assert!(matches!(
        outcome,
        tome_core::fetch::FetchOutcome::Fetched(_)
    ));
}

// ---- refute-panel regression: proxy env vars must not bypass the filter ----

#[test]
fn a_proxy_environment_variable_does_not_bypass_the_filter() {
    // The confirmed HIGH bypass: ureq defaults to Proxy::try_from_env(), and
    // a proxy connection dials the PROXY, so the destination is never
    // classified. The fetcher disables proxy support; this asserts a set
    // HTTP_PROXY does not change the verdict for a forbidden literal IP.
    //
    // Uses a real env var for the duration of the test. Serialised with the
    // other env-touching test via a shared lock is unnecessary here: the
    // fetcher reads proxy config once at construction, so we set the var,
    // build, then restore.
    let previous = std::env::var_os("HTTP_PROXY");
    // SAFETY: single-threaded within this test; restored below.
    std::env::set_var("HTTP_PROXY", "http://127.0.0.1:3128");

    let f = fetcher(false);
    let url: Url = "http://169.254.169.254/latest/meta-data/".parse().unwrap();
    let result = f.fetch(&url, MB, None);

    match previous {
        Some(v) => std::env::set_var("HTTP_PROXY", v),
        None => std::env::remove_var("HTTP_PROXY"),
    }

    assert!(
        matches!(result, Err(Error::BlockedByFilter)),
        "a proxy env var must not route the metadata endpoint past the filter, got {result:?}"
    );
}

// ---- a public hostname resolving to a blocked address ----------------------

#[test]
fn a_public_name_pointing_at_a_blocked_address_is_refused() {
    // localhost resolves to 127.0.0.1 (and possibly ::1) — a public-looking
    // NAME whose ADDRESS is blocked. This is the DNS-level SSRF case, and
    // the GuardResolver is what catches it: the name resolves, every
    // address is filtered, and the fetch is refused. (A rebinding attacker's
    // second resolution never happens — the connection uses the checked set.)
    let f = fetcher(false);
    let url: Url = "http://localhost/whatever".parse().unwrap();
    let err = f.fetch(&url, MB, None).unwrap_err();
    assert!(
        matches!(err, Error::BlockedByFilter),
        "localhost -> 127.0.0.1 must be refused, got {err:?}"
    );
}
