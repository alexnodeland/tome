//! Source config parsing and validation (S1-3 / P1-005).
//!
//! Error-message assertions here check for the *field name* being present,
//! not exact copy — the acceptance criterion is "helpful", which means the
//! message must at minimum say which field broke which rule.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;

use tome_core::config::{SourceConfig, SourceSpec, RATE_LIMIT_CAP_RPS, RATE_LIMIT_DEFAULT_RPS};
use tome_core::model::{Icon, Schedule, SourceId, SourceType, SyncStrategy};
use tome_core::Error;

fn parse(yaml: &str) -> Result<SourceConfig, Error> {
    SourceConfig::parse_str(
        SourceId::new("python").unwrap(),
        yaml,
        Path::new("sources/python.yaml"),
    )
}

fn message(result: Result<SourceConfig, Error>) -> String {
    match result {
        Err(Error::Config { message, .. }) => message,
        Err(other) => panic!("expected Error::Config, got {other:?}"),
        Ok(_) => panic!("expected an error, config parsed"),
    }
}

const MINIMAL: &str = r#"
schema_version: 1
name: Python 3.13
source:
  type: readthedocs
  url: https://docs.python.org/3/
"#;

#[test]
fn minimal_config_parses_with_all_defaults() {
    let config = parse(MINIMAL).unwrap();

    assert_eq!(config.name, "Python 3.13");
    assert!(matches!(config.spec, SourceSpec::ReadTheDocs { .. }));
    assert_eq!(config.category, "Uncategorized");
    assert!(config.fetch.respect_robots);
    assert_eq!(config.fetch.rate_limit_rps, RATE_LIMIT_DEFAULT_RPS);
    assert_eq!(config.fetch.timeout.as_secs(), 30);
    assert!(!config.fetch.allow_insecure);
    assert_eq!(config.fetch.max_asset_bytes, 262_144_000);
    assert!(matches!(config.sync.strategy, SyncStrategy::Manual));
    assert!(!config.sync.pin_version);
    assert!(config.attribution.licence.is_none());
}

#[test]
fn full_config_parses_every_field() {
    let config = parse(
        // r## because the YAML contains `"#` (the accent colour), which
        // terminates an r#-delimited raw string.
        r##"
schema_version: 1
name: Python 3.13
source:
  type: generic
  url: https://docs.python.org/3/
  generic:
    entry_points: ["library/", "reference/"]
    max_depth: 3
    max_pages: 800
    include_patterns: ["^library/.*\\.html$"]
    exclude_patterns: ["whatsnew"]
    content_selector: "div.body"
    title_selector: "h1"
    nav_selector: "div.sphinxsidebar"
fetch:
  respect_robots: true
  rate_limit_rps: 1.5
  timeout_seconds: 10
  max_asset_bytes: 1048576
version: "3.13"
category: Languages
display:
  icon: "🐍"
  accent_color: "#3776AB"
attribution:
  homepage: https://www.python.org/
  licence: PSF-2.0
sync:
  strategy: scheduled
  schedule: weekly
  pin_version: true
enrich:
  link_to_source: false
"##,
    )
    .unwrap();

    let SourceSpec::Generic { url, scraper } = &config.spec else {
        panic!("expected generic spec");
    };
    assert_eq!(url.as_str(), "https://docs.python.org/3/");
    assert_eq!(scraper.entry_points.len(), 2);
    assert_eq!(scraper.max_depth, 3);
    assert_eq!(scraper.max_pages, 800);
    assert!(scraper.include_patterns[0].is_match("library/os.html"));
    assert_eq!(scraper.content_selector.as_deref(), Some("div.body"));
    assert_eq!(config.fetch.rate_limit_rps, 1.5);
    assert_eq!(config.version.as_deref(), Some("3.13"));
    assert_eq!(config.category, "Languages");
    assert!(matches!(&config.icon, Some(Icon::Emoji(e)) if e == "🐍"));
    assert_eq!(config.accent_color.as_deref(), Some("#3776AB"));
    assert_eq!(config.attribution.licence.as_deref(), Some("PSF-2.0"));
    assert!(matches!(
        config.sync.strategy,
        SyncStrategy::Scheduled {
            schedule: Schedule::Weekly
        }
    ));
    assert!(config.sync.pin_version);
}

#[test]
fn to_source_carries_everything_across() {
    let source = parse(MINIMAL).unwrap().to_source();
    assert_eq!(source.id.as_str(), "python");
    assert_eq!(source.kind, SourceType::ReadTheDocs);
    assert_eq!(source.url.unwrap().as_str(), "https://docs.python.org/3/");
    assert_eq!(source.page_count, 0);
    assert!(source.last_synced.is_none());
}

// ---- schema and field validation ---------------------------------------

#[test]
fn unknown_fields_are_errors_that_name_the_field() {
    let msg = message(parse(
        "schema_version: 1\nname: X\nsync_stategy: manual\nsource:\n  type: local\n  path: /x\n",
    ));
    assert!(msg.contains("sync_stategy"), "message was: {msg}");
}

#[test]
fn newer_schema_version_is_a_helpful_error() {
    let msg = message(parse(
        "schema_version: 2\nname: X\nsource:\n  type: local\n  path: /x\n",
    ));
    assert!(msg.contains("newer"), "message was: {msg}");
}

#[test]
fn empty_name_is_rejected() {
    let msg = message(parse(
        "schema_version: 1\nname: \"  \"\nsource:\n  type: local\n  path: /x\n",
    ));
    assert!(msg.contains("name"), "message was: {msg}");
}

#[test]
fn unknown_source_type_lists_the_valid_ones() {
    let msg = message(parse(
        "schema_version: 1\nname: X\nsource:\n  type: sphinx\n  url: https://x.test/\n",
    ));
    assert!(
        msg.contains("sphinx") && msg.contains("readthedocs"),
        "message was: {msg}"
    );
}

// ---- URL rules ----------------------------------------------------------

#[test]
fn remote_types_require_a_url() {
    for kind in ["readthedocs", "rustdoc", "mdbook", "generic"] {
        let msg = message(parse(&format!(
            "schema_version: 1\nname: X\nsource:\n  type: {kind}\n"
        )));
        assert!(msg.contains("source.url"), "for {kind}, message was: {msg}");
    }
}

#[test]
fn http_needs_allow_insecure_and_the_message_says_so() {
    let yaml =
        "schema_version: 1\nname: X\nsource:\n  type: rustdoc\n  url: http://intranet/docs\n";
    let msg = message(parse(yaml));
    assert!(msg.contains("allow_insecure"), "message was: {msg}");

    let with_flag = format!("{yaml}fetch:\n  allow_insecure: true\n");
    assert!(parse(&with_flag).is_ok());
}

#[test]
fn non_http_schemes_are_rejected_outright() {
    let msg = message(parse(
        "schema_version: 1\nname: X\nsource:\n  type: rustdoc\n  url: file:///etc/passwd\n",
    ));
    assert!(msg.contains("https"), "message was: {msg}");
}

#[test]
fn fields_from_the_wrong_type_are_errors_not_dead_weight() {
    // A url on a local source means the author thought it did something.
    let msg = message(parse(
        "schema_version: 1\nname: X\nsource:\n  type: local\n  path: /x\n  url: https://x.test/\n",
    ));
    assert!(msg.contains("source.url"), "message was: {msg}");
}

// ---- generic scraper ------------------------------------------------------

#[test]
fn invalid_regex_names_the_pattern() {
    let msg = message(parse(
        "schema_version: 1\nname: X\nsource:\n  type: generic\n  url: https://x.test/\n  generic:\n    include_patterns: [\"[unclosed\"]\n",
    ));
    assert!(msg.contains("[unclosed"), "message was: {msg}");
}

#[test]
fn invalid_css_selector_names_the_selector() {
    let msg = message(parse(
        "schema_version: 1\nname: X\nsource:\n  type: generic\n  url: https://x.test/\n  generic:\n    content_selector: \"div..body\"\n",
    ));
    assert!(msg.contains("div..body"), "message was: {msg}");
}

#[test]
fn zero_depth_and_zero_pages_are_rejected() {
    for field in ["max_depth", "max_pages"] {
        let msg = message(parse(&format!(
            "schema_version: 1\nname: X\nsource:\n  type: generic\n  url: https://x.test/\n  generic:\n    {field}: 0\n"
        )));
        assert!(msg.contains(field), "message was: {msg}");
    }
}

// ---- man ------------------------------------------------------------------

#[test]
fn man_requires_paths_and_valid_sections() {
    let msg = message(parse(
        "schema_version: 1\nname: X\nsource:\n  type: man\n  man:\n    paths: []\n",
    ));
    assert!(msg.contains("paths"), "message was: {msg}");

    let msg = message(parse(
        "schema_version: 1\nname: X\nsource:\n  type: man\n  man:\n    paths: [/usr/share/man]\n    sections: [1, 9]\n",
    ));
    assert!(msg.contains('9'), "message was: {msg}");

    let config = parse(
        "schema_version: 1\nname: X\nsource:\n  type: man\n  man:\n    paths: [/usr/share/man]\n",
    )
    .unwrap();
    let SourceSpec::Man(man) = &config.spec else {
        panic!("expected man spec")
    };
    assert_eq!(man.sections, (1..=8).collect::<Vec<_>>());
}

// ---- fetch ------------------------------------------------------------------

#[test]
fn rate_limit_is_clamped_to_the_published_cap() {
    let config = parse(&format!("{MINIMAL}fetch:\n  rate_limit_rps: 10\n")).unwrap();
    assert_eq!(config.fetch.rate_limit_rps, RATE_LIMIT_CAP_RPS);
}

#[test]
fn nonpositive_rate_and_zero_timeout_are_rejected() {
    let msg = message(parse(&format!("{MINIMAL}fetch:\n  rate_limit_rps: 0\n")));
    assert!(msg.contains("rate_limit_rps"), "message was: {msg}");

    let msg = message(parse(&format!("{MINIMAL}fetch:\n  timeout_seconds: 0\n")));
    assert!(msg.contains("timeout_seconds"), "message was: {msg}");
}

// ---- sync cross-field rules -------------------------------------------------

#[test]
fn schedule_without_scheduled_strategy_is_the_error_the_prd_warns_about() {
    let msg = message(parse(&format!(
        "{MINIMAL}sync:\n  strategy: manual\n  schedule: weekly\n"
    )));
    assert!(msg.contains("schedule"), "message was: {msg}");
}

#[test]
fn scheduled_without_schedule_is_rejected() {
    let msg = message(parse(&format!("{MINIMAL}sync:\n  strategy: scheduled\n")));
    assert!(msg.contains("sync.schedule"), "message was: {msg}");
}

#[test]
fn watch_requires_watch_source_and_example_is_in_the_message() {
    let msg = message(parse(&format!("{MINIMAL}sync:\n  strategy: watch\n")));
    assert!(msg.contains("watch_source"), "message was: {msg}");

    let config = parse(&format!(
        "{MINIMAL}sync:\n  strategy: watch\n  watch_source: \"crates:serde\"\n"
    ))
    .unwrap();
    assert!(matches!(
        config.sync.strategy,
        SyncStrategy::Watch { ref source } if source == "crates:serde"
    ));
}

// ---- display -----------------------------------------------------------------

#[test]
fn accent_color_must_be_six_digit_hex() {
    let msg = message(parse(&format!(
        "{MINIMAL}display:\n  accent_color: \"blue\"\n"
    )));
    assert!(msg.contains("accent_color"), "message was: {msg}");
}

#[test]
fn icon_classification_url_path_emoji() {
    let remote = parse(&format!(
        "{MINIMAL}display:\n  icon: \"https://x.test/icon.png\"\n"
    ))
    .unwrap();
    assert!(matches!(remote.icon, Some(Icon::Remote(_))));

    let file = parse(&format!(
        "{MINIMAL}display:\n  icon: \"icons/python.svg\"\n"
    ))
    .unwrap();
    assert!(matches!(file.icon, Some(Icon::File(_))));

    let emoji = parse(&format!("{MINIMAL}display:\n  icon: \"🐍\"\n")).unwrap();
    assert!(matches!(emoji.icon, Some(Icon::Emoji(_))));
}

// ---- files ---------------------------------------------------------------------

#[test]
fn parse_file_derives_the_id_from_the_stem_and_validates_it() {
    let dir = tempfile::tempdir().unwrap();

    let good = dir.path().join("rust-std.yaml");
    std::fs::write(
        &good,
        "schema_version: 1\nname: Rust\nsource:\n  type: local\n  path: /x\n",
    )
    .unwrap();
    let config = SourceConfig::parse_file(&good).unwrap();
    assert_eq!(config.id.as_str(), "rust-std");

    // The file name IS the identity, so a hostile stem is a config error.
    let bad = dir.path().join(".hidden.yaml");
    std::fs::write(
        &bad,
        "schema_version: 1\nname: X\nsource:\n  type: local\n  path: /x\n",
    )
    .unwrap();
    let msg = match SourceConfig::parse_file(&bad) {
        Err(Error::Config { message, .. }) => message,
        other => panic!("expected config error, got {other:?}"),
    };
    assert!(msg.contains("source id"), "message was: {msg}");
}
