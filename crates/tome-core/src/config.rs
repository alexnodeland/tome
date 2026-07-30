//! Source configuration parsing and validation (implementation plan S1-3).
//!
//! Reads the YAML schema from `docs/PRD.md` Appendix A — one file per source
//! under [`Paths::sources_dir`](crate::Paths::sources_dir), the file stem
//! being the [`SourceId`]. Parsing is two layers on purpose:
//!
//! 1. **Raw serde structs** mirror the YAML exactly, with
//!    `deny_unknown_fields` everywhere. A typo (`sync_stategy:`) is an error
//!    naming the unknown field, not a silently ignored setting — for a file
//!    users hand-edit, that is the single most useful validation there is.
//! 2. **[`SourceConfig`]** is what the rest of the codebase sees: variants
//!    per source type so that "a rustdoc source without a URL" is
//!    unrepresentable, compiled regexes, parsed URLs, validated selectors,
//!    and every default applied.
//!
//! Every error is [`Error::Config`] with the file named and a message that
//! says which field and what rule — P1-005's acceptance criteria call for
//! helpful messages, and "invalid config" is not one.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use url::Url;

use crate::error::{Error, Result};
use crate::model::{
    Attribution, Icon, Schedule, Source, SourceId, SourceType, SyncConfig, SyncStrategy,
};

/// Schema version this build reads and writes. See
/// `docs/plans/14-api-versioning-strategy.md` for the migration policy.
pub const SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// The validated configuration — what everything downstream consumes.
// ---------------------------------------------------------------------------

/// A parsed, validated source configuration.
#[derive(Debug, Clone)]
pub struct SourceConfig {
    pub id: SourceId,
    pub name: String,
    pub spec: SourceSpec,
    pub fetch: FetchConfig,
    pub version: Option<String>,
    pub category: String,
    pub icon: Option<Icon>,
    pub accent_color: Option<String>,
    pub attribution: Attribution,
    pub sync: SyncConfig,
    /// A runtime cap on pages crawled, overriding whatever the spec's own
    /// limits are. **Not part of the YAML schema** — the parser always leaves
    /// it `None`; only a caller sets it.
    ///
    /// It exists for health checks (`scripts/verify-registry.sh` asks "does
    /// this scraper still find anything?", not "fetch the site"), and it is a
    /// runtime override precisely so the config file the check reads stays
    /// byte-identical to the one users get. A check that edits the file
    /// verifies something nobody runs.
    ///
    /// It applies to *every* source type. The generic scraper's own
    /// `max_pages` is a user-facing setting; this is not.
    pub max_pages_override: Option<u32>,
}

impl SourceConfig {
    /// Cap the crawl at `pages`, whatever the config says.
    pub fn cap_pages(&mut self, pages: u32) {
        self.max_pages_override = Some(pages);
    }
}

/// The per-type half of a config. An enum rather than optional fields so the
/// invalid combinations (`type: rustdoc` with no `url`, `type: local` with a
/// `url`) are unrepresentable after validation instead of checked at use.
#[derive(Debug, Clone)]
pub enum SourceSpec {
    ReadTheDocs {
        url: Url,
    },
    Rustdoc {
        url: Url,
    },
    MdBook {
        url: Url,
    },
    Man(ManConfig),
    Generic {
        url: Url,
        scraper: GenericScraperConfig,
    },
    Local {
        path: PathBuf,
    },
    Docset {
        path: PathBuf,
    },
}

impl SourceSpec {
    pub fn source_type(&self) -> SourceType {
        match self {
            Self::ReadTheDocs { .. } => SourceType::ReadTheDocs,
            Self::Rustdoc { .. } => SourceType::Rustdoc,
            Self::MdBook { .. } => SourceType::MdBook,
            Self::Man(_) => SourceType::Man,
            Self::Generic { .. } => SourceType::Generic,
            Self::Local { .. } => SourceType::Local,
            Self::Docset { .. } => SourceType::Docset,
        }
    }

    /// The crawl entry point, for the remote variants.
    pub fn url(&self) -> Option<&Url> {
        match self {
            Self::ReadTheDocs { url }
            | Self::Rustdoc { url }
            | Self::MdBook { url }
            | Self::Generic { url, .. } => Some(url),
            Self::Man(_) | Self::Local { .. } | Self::Docset { .. } => None,
        }
    }

    /// Generic-scraper crawl entry paths (relative to [`Self::url`]). Empty
    /// for every non-generic type; the crawler falls back to the base URL.
    pub fn generic_entry_points(&self) -> &[String] {
        match self {
            Self::Generic { scraper, .. } => &scraper.entry_points,
            _ => &[],
        }
    }

    /// Compiled include patterns, if this is a generic source with any.
    pub fn generic_include(&self) -> Option<&Vec<regex::Regex>> {
        match self {
            Self::Generic { scraper, .. } if !scraper.include_patterns.is_empty() => {
                Some(&scraper.include_patterns)
            }
            _ => None,
        }
    }

    /// Compiled exclude patterns, if this is a generic source with any.
    pub fn generic_exclude(&self) -> Option<&Vec<regex::Regex>> {
        match self {
            Self::Generic { scraper, .. } if !scraper.exclude_patterns.is_empty() => {
                Some(&scraper.exclude_patterns)
            }
            _ => None,
        }
    }
}

/// Generic scraper settings, validated: patterns compile, selectors parse.
#[derive(Debug, Clone)]
pub struct GenericScraperConfig {
    /// Starting paths relative to the entry URL. Empty means "the URL itself".
    pub entry_points: Vec<String>,
    pub max_depth: u32,
    /// Hard cap on pages fetched — bounds a runaway crawl.
    pub max_pages: u32,
    pub include_patterns: Vec<regex::Regex>,
    pub exclude_patterns: Vec<regex::Regex>,
    pub content_selector: Option<String>,
    pub title_selector: Option<String>,
    pub nav_selector: Option<String>,
}

impl Default for GenericScraperConfig {
    fn default() -> Self {
        Self {
            entry_points: Vec::new(),
            max_depth: 4,
            max_pages: 5000,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            content_selector: None,
            title_selector: None,
            nav_selector: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ManConfig {
    /// Man page directories. Empty is invalid — a man source with nowhere to
    /// look is a configuration mistake, not a default.
    pub paths: Vec<PathBuf>,
    /// Sections to index; defaults to all of 1–8.
    pub sections: Vec<u8>,
}

/// Fetch etiquette. Defaults are the non-negotiable safe values from
/// `docs/PRD.md` § Crawl etiquette; the cap on `rate_limit_rps` exists
/// because Read the Docs publishes "< 4 requests per second" as its ceiling
/// (SPIKE-010) and no configuration may cross the strictest published limit.
#[derive(Debug, Clone)]
pub struct FetchConfig {
    pub respect_robots: bool,
    pub rate_limit_rps: f64,
    pub timeout: Duration,
    pub allow_insecure: bool,
    pub max_asset_bytes: u64,
}

pub const RATE_LIMIT_DEFAULT_RPS: f64 = 2.0;
pub const RATE_LIMIT_CAP_RPS: f64 = 4.0;

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            respect_robots: true,
            rate_limit_rps: RATE_LIMIT_DEFAULT_RPS,
            timeout: Duration::from_secs(30),
            allow_insecure: false,
            max_asset_bytes: 262_144_000, // 250 MB
        }
    }
}

// ---------------------------------------------------------------------------
// Raw layer: the YAML as written. deny_unknown_fields throughout.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    schema_version: u32,
    name: String,
    source: RawSource,
    #[serde(default)]
    fetch: Option<RawFetch>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    display: Option<RawDisplay>,
    #[serde(default)]
    attribution: Option<RawAttribution>,
    #[serde(default)]
    sync: Option<RawSync>,
    // Parsed so a config carrying it is not rejected, deliberately unused:
    // enrichment is post-v1 and unfunded (PRD § Non-Goals).
    #[serde(default)]
    #[allow(dead_code)]
    enrich: Option<RawEnrich>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSource {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    path: Option<PathBuf>,
    #[serde(default)]
    generic: Option<RawGeneric>,
    #[serde(default)]
    man: Option<RawMan>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGeneric {
    #[serde(default)]
    entry_points: Vec<String>,
    #[serde(default)]
    max_depth: Option<u32>,
    #[serde(default)]
    max_pages: Option<u32>,
    #[serde(default)]
    include_patterns: Vec<String>,
    #[serde(default)]
    exclude_patterns: Vec<String>,
    #[serde(default)]
    content_selector: Option<String>,
    #[serde(default)]
    title_selector: Option<String>,
    #[serde(default)]
    nav_selector: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMan {
    #[serde(default)]
    paths: Vec<PathBuf>,
    #[serde(default)]
    sections: Option<Vec<u8>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFetch {
    #[serde(default)]
    respect_robots: Option<bool>,
    #[serde(default)]
    rate_limit_rps: Option<f64>,
    #[serde(default)]
    timeout_seconds: Option<u64>,
    #[serde(default)]
    allow_insecure: Option<bool>,
    #[serde(default)]
    max_asset_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDisplay {
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    accent_color: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAttribution {
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    licence: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSync {
    #[serde(default)]
    strategy: Option<String>,
    #[serde(default)]
    schedule: Option<String>,
    #[serde(default)]
    watch_source: Option<String>,
    #[serde(default)]
    pin_version: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEnrich {
    #[serde(default)]
    #[allow(dead_code)]
    link_to_source: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    tldr_summaries: Option<bool>,
}

// ---------------------------------------------------------------------------
// Parsing and validation.
// ---------------------------------------------------------------------------

impl SourceConfig {
    /// Parse a config file. The file stem is the source id — the file name
    /// IS the identity (see `model/mod.rs`), so an unpronounceable stem is a
    /// config error, not a fallback.
    pub fn parse_file(path: &Path) -> Result<Self> {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| config_error(path, "file name must be valid UTF-8"))?;
        let id = SourceId::new(stem)
            .map_err(|e| config_error(path, format!("file name is not a valid source id: {e}")))?;
        let yaml = std::fs::read_to_string(path)
            .map_err(|e| config_error(path, format!("could not read the file: {e}")))?;
        Self::parse_str(id, &yaml, path)
    }

    /// Parse from a string. `file` is used only for error messages.
    pub fn parse_str(id: SourceId, yaml: &str, file: &Path) -> Result<Self> {
        let raw: RawConfig = serde_yaml_ng::from_str(yaml)
            .map_err(|e| config_error(file, format!("not a valid source config: {e}")))?;
        validate(id, raw, file)
    }

    /// The domain [`Source`] this config describes, for a source being added
    /// now. `page_count`/`index_size_bytes`/`last_synced` start at zero —
    /// ingest owns them.
    pub fn to_source(&self) -> Source {
        let mut source = Source::new(self.id.clone(), self.name.clone(), self.spec.source_type());
        source.url = self.spec.url().cloned();
        source.local_path = match &self.spec {
            SourceSpec::Local { path } | SourceSpec::Docset { path } => Some(path.clone()),
            _ => None,
        };
        source.version = self.version.clone();
        source.category = self.category.clone();
        source.icon = self.icon.clone();
        source.accent_color = self.accent_color.clone();
        source.attribution = self.attribution.clone();
        source.sync = self.sync.clone();
        source
    }
}

fn config_error(file: &Path, message: impl Into<String>) -> Error {
    Error::Config {
        file: file.to_path_buf(),
        message: message.into(),
    }
}

fn validate(id: SourceId, raw: RawConfig, file: &Path) -> Result<SourceConfig> {
    let err = |message: String| config_error(file, message);

    if raw.schema_version != SCHEMA_VERSION {
        return Err(err(format!(
            "schema_version {} is not supported by this build (expected {SCHEMA_VERSION}). \
             A larger version means the file was written by a newer Tome.",
            raw.schema_version
        )));
    }
    if raw.name.trim().is_empty() {
        return Err(err("name must not be empty".into()));
    }

    let fetch = validate_fetch(raw.fetch, file)?;
    let spec = validate_spec(raw.source, &fetch, file)?;
    let sync = validate_sync(raw.sync, file)?;

    let (icon, accent_color) = match raw.display {
        Some(display) => (
            display.icon.map(parse_icon),
            display
                .accent_color
                .map(|c| validate_accent_color(&c, file))
                .transpose()?,
        ),
        None => (None, None),
    };

    let attribution = match raw.attribution {
        Some(a) => Attribution {
            homepage: a
                .homepage
                .map(|h| {
                    Url::parse(&h)
                        .map_err(|e| err(format!("attribution.homepage is not a valid URL: {e}")))
                })
                .transpose()?,
            licence: a.licence,
        },
        None => Attribution::default(),
    };

    Ok(SourceConfig {
        id,
        name: raw.name,
        spec,
        fetch,
        version: raw.version,
        category: raw
            .category
            .unwrap_or_else(|| Source::DEFAULT_CATEGORY.to_owned()),
        icon,
        accent_color,
        attribution,
        sync,
        // Never set by the parser: it is a runtime override, not a schema
        // field. See the field's documentation.
        max_pages_override: None,
    })
}

fn validate_spec(raw: RawSource, fetch: &FetchConfig, file: &Path) -> Result<SourceSpec> {
    let err = |message: String| config_error(file, message);
    let kind = raw.kind.as_str();

    // Cross-field noise first: fields that cannot belong to this type are
    // errors, not dead weight. A `url` on a `local` source means the author
    // thought it did something.
    let require_url = |url: &Option<String>| -> Result<Url> {
        let text = url.as_ref().ok_or_else(|| {
            err(format!(
                "source.url is required when source.type is {kind:?}"
            ))
        })?;
        let parsed =
            Url::parse(text).map_err(|e| err(format!("source.url is not a valid URL: {e}")))?;
        match parsed.scheme() {
            "https" => Ok(parsed),
            "http" if fetch.allow_insecure => Ok(parsed),
            "http" => Err(err(
                "source.url uses http. Set fetch.allow_insecure: true if this host is \
                 yours (an intranet mirror); documentation on the public internet \
                 should be https."
                    .into(),
            )),
            other => Err(err(format!(
                "source.url must be https (got {other:?}); Tome fetches over HTTP(S) only"
            ))),
        }
    };
    let forbid = |field: &str, present: bool| -> Result<()> {
        if present {
            Err(err(format!(
                "source.{field} does not apply when source.type is {kind:?}"
            )))
        } else {
            Ok(())
        }
    };

    match kind {
        "readthedocs" | "rustdoc" | "mdbook" => {
            forbid("path", raw.path.is_some())?;
            forbid("generic", raw.generic.is_some())?;
            forbid("man", raw.man.is_some())?;
            let url = require_url(&raw.url)?;
            Ok(match kind {
                "readthedocs" => SourceSpec::ReadTheDocs { url },
                "rustdoc" => SourceSpec::Rustdoc { url },
                _ => SourceSpec::MdBook { url },
            })
        }
        "generic" => {
            forbid("path", raw.path.is_some())?;
            forbid("man", raw.man.is_some())?;
            let url = require_url(&raw.url)?;
            let scraper = validate_generic(raw.generic.unwrap_or_else(default_raw_generic), file)?;
            Ok(SourceSpec::Generic { url, scraper })
        }
        "man" => {
            forbid("url", raw.url.is_some())?;
            forbid("path", raw.path.is_some())?;
            forbid("generic", raw.generic.is_some())?;
            let man = raw
                .man
                .ok_or_else(|| err("source.man is required when source.type is \"man\"".into()))?;
            if man.paths.is_empty() {
                return Err(err(
                    "source.man.paths must name at least one directory".into()
                ));
            }
            let sections = man.sections.unwrap_or_else(|| (1..=8).collect());
            if let Some(bad) = sections.iter().find(|s| !(1..=8).contains(*s)) {
                return Err(err(format!(
                    "source.man.sections contains {bad}; man sections are 1-8"
                )));
            }
            Ok(SourceSpec::Man(ManConfig {
                paths: man.paths,
                sections,
            }))
        }
        "local" | "docset" => {
            forbid("url", raw.url.is_some())?;
            forbid("generic", raw.generic.is_some())?;
            forbid("man", raw.man.is_some())?;
            let path = raw.path.ok_or_else(|| {
                err(format!(
                    "source.path is required when source.type is {kind:?}"
                ))
            })?;
            Ok(if kind == "local" {
                SourceSpec::Local { path }
            } else {
                SourceSpec::Docset { path }
            })
        }
        other => Err(err(format!(
            "source.type {other:?} is not one of: readthedocs, rustdoc, mdbook, man, \
             generic, local, docset"
        ))),
    }
}

fn default_raw_generic() -> RawGeneric {
    RawGeneric {
        entry_points: Vec::new(),
        max_depth: None,
        max_pages: None,
        include_patterns: Vec::new(),
        exclude_patterns: Vec::new(),
        content_selector: None,
        title_selector: None,
        nav_selector: None,
    }
}

fn validate_generic(raw: RawGeneric, file: &Path) -> Result<GenericScraperConfig> {
    let err = |message: String| config_error(file, message);
    let defaults = GenericScraperConfig::default();

    let compile = |patterns: Vec<String>, field: &str| -> Result<Vec<regex::Regex>> {
        patterns
            .into_iter()
            .map(|p| {
                regex::Regex::new(&p).map_err(|e| {
                    err(format!(
                        "source.generic.{field} pattern {p:?} is not a valid regex: {e}"
                    ))
                })
            })
            .collect()
    };
    let selector = |value: Option<String>, field: &str| -> Result<Option<String>> {
        value
            .map(|s| {
                // scraper's parser is the same one the crawler will use, so
                // "validates here" and "works there" cannot drift apart.
                scraper::Selector::parse(&s).map_err(|e| {
                    err(format!(
                        "source.generic.{field} {s:?} is not a valid CSS selector: {e}"
                    ))
                })?;
                Ok(s)
            })
            .transpose()
    };

    let max_depth = raw.max_depth.unwrap_or(defaults.max_depth);
    if max_depth == 0 {
        return Err(err("source.generic.max_depth must be at least 1".into()));
    }
    let max_pages = raw.max_pages.unwrap_or(defaults.max_pages);
    if max_pages == 0 {
        return Err(err("source.generic.max_pages must be at least 1".into()));
    }

    Ok(GenericScraperConfig {
        entry_points: raw.entry_points,
        max_depth,
        max_pages,
        include_patterns: compile(raw.include_patterns, "include_patterns")?,
        exclude_patterns: compile(raw.exclude_patterns, "exclude_patterns")?,
        content_selector: selector(raw.content_selector, "content_selector")?,
        title_selector: selector(raw.title_selector, "title_selector")?,
        nav_selector: selector(raw.nav_selector, "nav_selector")?,
    })
}

fn validate_fetch(raw: Option<RawFetch>, file: &Path) -> Result<FetchConfig> {
    let err = |message: String| config_error(file, message);
    let defaults = FetchConfig::default();
    let Some(raw) = raw else {
        return Ok(defaults);
    };

    let rate = raw.rate_limit_rps.unwrap_or(defaults.rate_limit_rps);
    if !rate.is_finite() || rate <= 0.0 {
        return Err(err("fetch.rate_limit_rps must be a positive number".into()));
    }
    // Clamped, not rejected: the cap is not negotiable (SPIKE-010 — Read the
    // Docs publishes < 4 req/s), but a config asking for 10 wants "fast",
    // and "as fast as allowed" honours the intent while the warn says why.
    let rate = if rate > RATE_LIMIT_CAP_RPS {
        tracing::warn!(
            requested = rate,
            cap = RATE_LIMIT_CAP_RPS,
            "fetch.rate_limit_rps capped; hosts publish limits and Tome stays under them"
        );
        RATE_LIMIT_CAP_RPS
    } else {
        rate
    };

    let timeout = raw
        .timeout_seconds
        .map(Duration::from_secs)
        .unwrap_or(defaults.timeout);
    if timeout.is_zero() {
        return Err(err("fetch.timeout_seconds must be at least 1".into()));
    }

    Ok(FetchConfig {
        respect_robots: raw.respect_robots.unwrap_or(defaults.respect_robots),
        rate_limit_rps: rate,
        timeout,
        allow_insecure: raw.allow_insecure.unwrap_or(defaults.allow_insecure),
        max_asset_bytes: raw.max_asset_bytes.unwrap_or(defaults.max_asset_bytes),
    })
}

fn validate_sync(raw: Option<RawSync>, file: &Path) -> Result<SyncConfig> {
    let err = |message: String| config_error(file, message);
    let Some(raw) = raw else {
        return Ok(SyncConfig::default());
    };

    let strategy_name = raw.strategy.as_deref().unwrap_or("manual");
    let strategy = match strategy_name {
        "manual" | "on_launch" => {
            if let Some(schedule) = &raw.schedule {
                // The PRD calls this confusion out by name: `weekly` is a
                // schedule value, not a strategy value, and silently ignoring
                // it here would tell the author their source syncs weekly
                // when it never will.
                return Err(err(format!(
                    "sync.schedule ({schedule:?}) only applies when sync.strategy is \
                     \"scheduled\" (got {strategy_name:?})"
                )));
            }
            if raw.watch_source.is_some() {
                return Err(err(format!(
                    "sync.watch_source only applies when sync.strategy is \"watch\" \
                     (got {strategy_name:?})"
                )));
            }
            if strategy_name == "manual" {
                SyncStrategy::Manual
            } else {
                SyncStrategy::OnLaunch
            }
        }
        "scheduled" => {
            if raw.watch_source.is_some() {
                return Err(err(
                    "sync.watch_source only applies when sync.strategy is \"watch\"".into(),
                ));
            }
            let schedule = match raw.schedule.as_deref() {
                Some("daily") => Schedule::Daily,
                Some("weekly") => Schedule::Weekly,
                Some("monthly") => Schedule::Monthly,
                Some(other) => {
                    return Err(err(format!(
                        "sync.schedule {other:?} is not one of: daily, weekly, monthly"
                    )))
                }
                None => {
                    return Err(err(
                        "sync.schedule is required when sync.strategy is \"scheduled\"".into(),
                    ))
                }
            };
            SyncStrategy::Scheduled { schedule }
        }
        "watch" => {
            if raw.schedule.is_some() {
                return Err(err(
                    "sync.schedule only applies when sync.strategy is \"scheduled\"".into(),
                ));
            }
            let source = raw.watch_source.ok_or_else(|| {
                err(
                    "sync.watch_source is required when sync.strategy is \"watch\" \
                     (e.g. \"crates:serde\")"
                        .into(),
                )
            })?;
            SyncStrategy::Watch { source }
        }
        other => {
            return Err(err(format!(
                "sync.strategy {other:?} is not one of: manual, on_launch, scheduled, watch"
            )))
        }
    };

    Ok(SyncConfig {
        strategy,
        pin_version: raw.pin_version.unwrap_or(false),
    })
}

fn validate_accent_color(color: &str, file: &Path) -> Result<String> {
    let hex = color.strip_prefix('#').unwrap_or("");
    if hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(color.to_owned())
    } else {
        Err(config_error(
            file,
            format!("display.accent_color {color:?} must be a hex colour like \"#5E6AD2\""),
        ))
    }
}

/// The config schema says an icon is "URL, local path, or emoji". Classified
/// rather than guessed: a parseable http(s) URL is remote, anything with a
/// path separator is a file, the rest is treated as emoji/text.
fn parse_icon(icon: String) -> Icon {
    if let Ok(url) = Url::parse(&icon) {
        if matches!(url.scheme(), "http" | "https") {
            return Icon::Remote(url);
        }
    }
    if icon.contains('/') {
        Icon::File(PathBuf::from(icon))
    } else {
        Icon::Emoji(icon)
    }
}
