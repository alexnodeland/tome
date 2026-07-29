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

use tome_core::model::SourceType;
use tome_core::normalize::normalize;
use tome_core::parse::parse_page_with;
use tome_core::scrape::profile_for;
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
            // No content selector, deliberately. The corpus spans six
            // platforms — Sphinx, mdBook, rustdoc, Node, Hugo, and the Go
            // site — and a selector tuned for one of them would be tested
            // against five it does not fit. `None` exercises the automatic
            // content-root discovery (`main`, `[role=main]`, `article`,
            // `body`), which is the path a generic source with no
            // `content_selector` takes, and therefore the one most pages
            // will actually go through.
            //
            // The **platform profile** (S2-11) does come from the case name,
            // because that is what the product does: a source declares its
            // type and the crawler looks the profile up. Testing the profiles
            // against the generic path would test them against nothing.
            let parsed = parse_page_with(
                &case.text(),
                &base_for(&case.name),
                None,
                profile_for(platform_of(&case.name)),
            );
            let normalized = normalize(parsed.body, &base_for(&case.name));
            // Pretty JSON so the diff on a change is line-oriented and
            // reviewable, not one giant line.
            serde_json::to_string_pretty(&normalized).expect("normalized page serializes")
        })
        .expect("running the golden corpus");

    assert!(report.is_ok(), "{report}");
}

/// The platform a corpus case came from, by filename prefix.
///
/// The corpus is named `<platform>-<what-is-interesting>.html` by convention
/// (`corpus/README.md`), so this reads the convention rather than adding a
/// manifest that would have to be kept in step with the directory.
///
/// Cases with no profile — Node, Hugo, go.dev — return `Generic`, which is
/// also what those sources are configured as.
fn platform_of(name: &str) -> SourceType {
    match name.split('-').next() {
        Some("sphinx") => SourceType::ReadTheDocs,
        Some("rustdoc") => SourceType::Rustdoc,
        Some("mdbook") => SourceType::MdBook,
        _ => SourceType::Generic,
    }
}
