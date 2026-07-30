//! Reading the source registry (S3-8, used by onboarding in S4-4).
//!
//! The registry is **configuration, never content** — an index of ready-made
//! source configurations, each of which a user's own machine then fetches from
//! the origin. That property is what keeps the legal posture clean
//! (SPIKE-010), and nothing here may trade it for convenience: this module
//! reads YAML off disk and hands back ids and names. It fetches nothing.
//!
//! Why a typed parser here when `tests/registry.rs` deliberately uses loose
//! YAML: that test *defines* the index's shape, and a serde struct would make
//! it pass by construction for fields it never checked. Production code has
//! the opposite need — it wants a missing field to be an error at the edge
//! rather than an empty string three layers in. The two are checked against
//! each other by `the_typed_parser_sees_what_the_shape_test_sees`.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// One entry in `registry/index.yaml`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub category: String,
    /// Where the documentation lives. Shown so a person can judge the source
    /// before installing it.
    pub homepage: String,
    /// The documentation's licence, as published. Recorded because
    /// redistribution rules differ per source even though Tome redistributes
    /// nothing.
    pub licence: String,
    /// Path to the configuration, relative to the registry root.
    pub config: String,
    /// The date the verification job last confirmed this config still produces
    /// pages against the live site. **A stale date is the signal that a
    /// scraper has rotted** (RISK-003) — that is the whole point of the field,
    /// so it is surfaced rather than hidden.
    pub verified: String,
}

/// `registry/index.yaml`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Index {
    #[allow(dead_code)]
    pub version: u32,
    pub updated: String,
    pub sources: Vec<Entry>,
}

impl Index {
    /// Read the index at a registry root.
    pub fn read(root: &Path) -> Result<Self> {
        let file = root.join("index.yaml");
        let text = std::fs::read_to_string(&file).map_err(|source| Error::Config {
            file: file.clone(),
            message: format!("could not be read: {source}"),
        })?;
        let index: Self = serde_yaml_ng::from_str(&text).map_err(|source| Error::Config {
            file,
            message: source.to_string(),
        })?;
        Ok(index)
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.sources.iter().find(|entry| entry.id == id)
    }
}

impl Entry {
    /// The configuration file this entry names, under a registry root.
    ///
    /// The path comes out of a YAML file that ships with the app, not out of
    /// anything a user typed — but it is still joined defensively, because a
    /// `config:` of `../../../etc/passwd` would otherwise read whatever it
    /// named, and "ours today" is not a property that survives the registry
    /// gaining contributors.
    pub fn config_path(&self, root: &Path) -> Result<PathBuf> {
        let relative = Path::new(&self.config);
        if relative.is_absolute()
            || relative
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(Error::Config {
                file: root.join(&self.config),
                message: "a registry `config:` must be a relative path inside the registry"
                    .to_owned(),
            });
        }
        Ok(root.join(relative))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn repo_registry() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registry")
    }

    #[test]
    fn the_typed_parser_sees_what_the_shape_test_sees() {
        // `tests/registry.rs` reads the same file as loose YAML and is the
        // definition of its shape. If this parser and that one ever disagree
        // about how many sources there are, one of them is wrong about the
        // file that ships.
        let index = Index::read(&repo_registry()).expect("the shipped index parses");
        let loose: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            &std::fs::read_to_string(repo_registry().join("index.yaml")).expect("readable"),
        )
        .expect("valid YAML");
        assert_eq!(
            index.sources.len(),
            loose["sources"]
                .as_sequence()
                .expect("sources sequence")
                .len()
        );
        assert!(!index.updated.is_empty());
        for entry in &index.sources {
            assert!(entry.config_path(&repo_registry()).is_ok());
            assert!(!entry.verified.is_empty(), "{}: verified date", entry.id);
        }
    }

    #[test]
    fn a_config_path_cannot_escape_the_registry() {
        let escape = Entry {
            id: "evil".to_owned(),
            name: "Evil".to_owned(),
            category: "x".to_owned(),
            homepage: "https://example.com".to_owned(),
            licence: "MIT".to_owned(),
            config: "../../../etc/passwd".to_owned(),
            verified: "2026-07-30".to_owned(),
        };
        assert!(escape.config_path(Path::new("/registry")).is_err());
    }

    #[test]
    fn get_finds_by_id_and_nothing_else() {
        let index = Index::read(&repo_registry()).expect("index");
        let first = index
            .sources
            .first()
            .expect("at least one source")
            .id
            .clone();
        assert!(index.get(&first).is_some());
        assert!(index.get("no-such-source").is_none());
    }
}
