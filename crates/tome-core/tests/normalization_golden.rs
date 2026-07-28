//! The normalization golden corpus (S1-8).
//!
//! Each `corpus/normalization/input/<name>.html` is parsed and normalized,
//! and its output AST (pretty JSON) is diffed against
//! `corpus/normalization/golden/<name>.json`. This is where "does
//! documentation come out the other side looking right?" is answered by
//! review rather than assertion: a change to the parser or the pipeline that
//! alters any page's output shows up as a reviewable diff, and
//! `TOME_UPDATE_GOLDEN=1` rewrites the goldens (failing that run, so the
//! passing run is the one after `git diff`).
//!
//! Corpus contents and the SPIKE-010 licence gate are documented in
//! `corpus/README.md` and `corpus/normalization/input/SOURCES.md`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use tome_core::normalize::normalize;
use tome_core::parse::parse_page;
use tome_testkit::Golden;
use url::Url;

#[test]
fn normalization_matches_the_golden_corpus() {
    // A stable synthetic base URL per case, so relative-URL resolution is
    // deterministic regardless of where the fixture originally came from.
    let base_for = |name: &str| -> Url {
        format!("https://corpus.test/{name}.html")
            .parse()
            .expect("synthetic base URL")
    };

    let report = Golden::new("corpus/normalization")
        .golden_extension("json")
        .check(|case| {
            let parsed = parse_page(&case.text(), &base_for(&case.name), Some("div.document"));
            let normalized = normalize(parsed.body, &base_for(&case.name));
            // Pretty JSON so the diff on a change is line-oriented and
            // reviewable, not one giant line.
            serde_json::to_string_pretty(&normalized).expect("normalized page serializes")
        })
        .expect("running the golden corpus");

    assert!(report.is_ok(), "{report}");
}
