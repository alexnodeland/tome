//! The source registry's offline half (S3-8).
//!
//! **This runs in the gate; the live half deliberately does not.** These
//! checks need no network: every registry config parses with the real parser,
//! every index entry points at a file that exists, ids agree with file names,
//! and attribution is present. They catch the failures that are *always*
//! errors — a typo, a stale path, a config the parser rejects — and they catch
//! them in seconds.
//!
//! What they cannot catch is scraper rot (RISK-003): a config that still
//! parses but no longer produces pages because the site was redesigned. That
//! needs the live site, so it lives in `scripts/verify-registry.sh`, is run on
//! a schedule rather than in the gate, and writes back the `verified` dates.
//! Putting it here instead would make the gate fail when someone else's
//! website is down, which teaches everyone to ignore the gate.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use tome_core::config::SourceConfig;
use tome_core::model::SourceId;

fn registry_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../registry")
        .canonicalize()
        .expect("registry directory exists")
}

/// The index, as the loose YAML it is — deliberately not a typed struct: this
/// test is what defines the index's shape, and a serde struct would make the
/// test pass by construction for fields it never checked.
fn index() -> serde_yaml_ng::Value {
    let text = std::fs::read_to_string(registry_dir().join("index.yaml"))
        .expect("registry/index.yaml is readable");
    serde_yaml_ng::from_str(&text).expect("registry/index.yaml is valid YAML")
}

fn entries() -> Vec<serde_yaml_ng::Value> {
    index()["sources"]
        .as_sequence()
        .expect("index has a `sources` sequence")
        .clone()
}

fn field<'a>(entry: &'a serde_yaml_ng::Value, name: &str) -> &'a str {
    entry[name]
        .as_str()
        .unwrap_or_else(|| panic!("registry entry is missing `{name}`: {entry:?}"))
}

#[test]
fn every_registry_config_parses_with_the_real_parser() {
    for entry in entries() {
        let id = field(&entry, "id");
        let config_path = registry_dir().join(field(&entry, "config"));
        assert!(
            config_path.exists(),
            "`{id}` points at {} which does not exist",
            config_path.display()
        );

        let source_id = SourceId::new(id).expect("registry id is a valid source id");
        // The real parser, not a schema copy: a registry config that this
        // rejects is one `tome pull` would reject too, and the registry
        // exists so users never meet that.
        let config = SourceConfig::parse_file(&config_path)
            .unwrap_or_else(|e| panic!("`{id}` does not parse: {e}"));

        assert_eq!(
            config.id, source_id,
            "`{id}`'s config file name must match its registry id — the file name IS the \
             identity everywhere else in Tome"
        );

        // Attribution is not optional here even though the schema allows it
        // to be: SPIKE-010's attribution rules are the legal posture, and a
        // registry entry is the one place Tome asserts what a source is.
        assert!(
            config.attribution.homepage.is_some(),
            "`{id}` has no attribution.homepage"
        );
        assert!(
            config.attribution.licence.is_some(),
            "`{id}` has no attribution.licence"
        );
    }
}

#[test]
fn index_metadata_agrees_with_the_configs() {
    for entry in entries() {
        let id = field(&entry, "id");
        let config = SourceConfig::parse_file(&registry_dir().join(field(&entry, "config")))
            .expect("config parses");

        // Two copies of a fact must not disagree — the defect the 2026-07-28
        // review found across three documents. The index is what a user
        // browses; the config is what runs. If they differ, the browse lies.
        assert_eq!(config.name, field(&entry, "name"), "`{id}`: name differs");
        assert_eq!(
            config.category,
            field(&entry, "category"),
            "`{id}`: category differs"
        );
        assert_eq!(
            config
                .attribution
                .licence
                .as_deref()
                .expect("licence present"),
            field(&entry, "licence"),
            "`{id}`: licence differs"
        );
        assert_eq!(
            config
                .attribution
                .homepage
                .as_ref()
                .expect("homepage present")
                .as_str(),
            field(&entry, "homepage"),
            "`{id}`: homepage differs"
        );
    }
}

#[test]
fn ids_are_unique_and_every_config_file_is_indexed() {
    let indexed: BTreeSet<String> = entries()
        .iter()
        .map(|e| field(e, "id").to_owned())
        .collect();
    assert_eq!(
        indexed.len(),
        entries().len(),
        "duplicate id in registry/index.yaml"
    );

    // The other direction: a config file nobody indexed is invisible to
    // users and unverified by CI — worse than absent, because it looks
    // like coverage.
    let on_disk: BTreeSet<String> = std::fs::read_dir(registry_dir().join("sources"))
        .expect("registry/sources is readable")
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension()? == "yaml").then(|| path.file_stem()?.to_str().map(str::to_owned))?
        })
        .collect();
    assert_eq!(
        on_disk, indexed,
        "registry/sources and registry/index.yaml disagree about which sources exist"
    );
}

#[test]
fn registry_sources_respect_robots_and_the_default_rate_limit() {
    // A registry-shipped configuration cannot override robots.txt — the rule
    // is in CLAUDE.md and it is the difference between a documentation reader
    // and a scraper someone else has to block. It fails here rather than in
    // review, because "respect_robots: false" in a PR is one plausible-looking
    // line.
    for entry in entries() {
        let id = field(&entry, "id");
        let config = SourceConfig::parse_file(&registry_dir().join(field(&entry, "config")))
            .expect("config parses");
        assert!(
            config.fetch.respect_robots,
            "`{id}` disables robots.txt; registry configs may never do that"
        );
        assert!(
            config.fetch.rate_limit_rps <= tome_core::config::RATE_LIMIT_DEFAULT_RPS,
            "`{id}` raises the rate limit above the default; a shipped config crawls \
             other people's servers on behalf of every user who installs it"
        );
        assert!(
            !config.fetch.allow_insecure,
            "`{id}` sets allow_insecure, which is for hosts you own"
        );
    }
}
