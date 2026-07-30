//! `tome add` (P4-003) — and the first user-facing consumer of platform
//! detection (P2-014).
//!
//! The flow is: classify the homepage, propose a config, confirm, write the
//! YAML, pull. Two properties matter more than the ergonomics:
//!
//! * **Detection decides "act, or ask", never "act wrongly".** A confident
//!   detection picks the platform's scraper profile; anything below
//!   [`detect::AUTO_ACCEPT`] falls back to the generic scraper, which is
//!   correct-but-plain for every platform. The cost of being confidently
//!   wrong is a library full of mis-parsed pages that look fine until read.
//! * **The written file is the contract, not this code.** The YAML goes
//!   through [`SourceConfig::parse_file`] before anything fetches — the same
//!   parser every other path uses — so `tome add` cannot write a config that
//!   `tome pull` would then reject.

use std::io::{IsTerminal, Write as _};

use anyhow::{bail, Context, Result};
use tome_core::config::{FetchConfig, GenericScraperConfig, SourceConfig};
use tome_core::detect::{self, Detection, Platform};
use tome_core::fetch::Fetcher;
use tome_core::model::SourceId;
use tome_core::Paths;
use url::Url;

pub(crate) struct AddOptions<'a> {
    pub yes: bool,
    pub name: Option<&'a str>,
    pub category: Option<&'a str>,
    pub insecure: bool,
    pub json: bool,
    pub quiet: bool,
}

pub(crate) fn add(paths: &Paths, target: &str, opts: &AddOptions<'_>) -> Result<()> {
    // A filesystem path is a `local`/`docset` source, and their ingestion
    // paths do not exist yet (see `pipeline::pull`). Saying so up front beats
    // writing a config that every pull would then report as unpullable.
    if target.starts_with('/') || target.starts_with('.') || std::path::Path::new(target).exists() {
        bail!(
            "`{target}` looks like a local path. Local directories and docsets \
             cannot be ingested yet — only web documentation. Pass a URL."
        );
    }

    let url = Url::parse(target).with_context(|| format!("`{target}` is not a valid URL"))?;
    match url.scheme() {
        "https" => {}
        "http" if opts.insecure => {}
        "http" => bail!(
            "{url} uses http. If this host is yours (an intranet mirror or a \
             local server), pass --insecure; documentation on the public \
             internet should be https."
        ),
        other => bail!("Tome fetches over HTTP(S) only (got {other:?})."),
    }

    // Interactivity is settled before any network traffic: a script that
    // forgot --yes should fail in a millisecond, not after a fetch.
    if !opts.yes {
        if opts.json {
            bail!("--json is non-interactive; pass --yes as well.");
        }
        if !std::io::stdin().is_terminal() {
            bail!("stdin is not a terminal; pass --yes to add without confirmation.");
        }
    }

    let id = match opts.name {
        Some(name) => id_from_text(name)
            .with_context(|| format!("could not derive a source id from name {name:?}"))?,
        None => {
            derive_id(&url).context("could not derive a source id from the URL; pass --name")?
        }
    };
    let name = opts
        .name
        .map(str::to_owned)
        .unwrap_or_else(|| id.as_str().to_owned());

    // Duplicates, by id and by URL. The id check is what makes `tome add`
    // idempotent-ish (the second run tells you what to do instead); the URL
    // check catches the same site added under two spellings.
    let config_file = paths.source_config_file(&id);
    if config_file.exists() {
        bail!(
            "Source `{}` already exists ({}). To refresh it, run `tome pull {}`.",
            id.as_str(),
            config_file.display(),
            id.as_str()
        );
    }
    for (existing_id, existing_path) in crate::source_configs(paths)? {
        let Ok(existing) = SourceConfig::parse_file(&existing_path) else {
            continue; // an invalid config is its owner's problem, not add's
        };
        if let Some(existing_url) = existing.spec.url() {
            if existing_url.as_str().trim_end_matches('/') == url.as_str().trim_end_matches('/') {
                bail!(
                    "{} is already configured as source `{}`.",
                    url,
                    existing_id.as_str()
                );
            }
        }
    }

    if !opts.quiet {
        eprintln!("Analyzing {url}…");
    }

    // The fetch goes through the ordinary Fetcher, so robots.txt, the rate
    // limit and the SSRF guard all apply — detection is the first thing Tome
    // does to a host a user names, which makes it exactly the wrong place to
    // skip any of them (`detect::detect_site`).
    let fetch_config = FetchConfig {
        allow_insecure: opts.insecure,
        ..FetchConfig::default()
    };
    let detection = detect::detect_site(&Fetcher::new(fetch_config), &url)
        .with_context(|| format!("could not fetch {url}"))?;

    let kind = if detection.is_confident() {
        config_type(detection.platform)
    } else {
        // Not confident means "ask", and the safe answer to act on is the
        // generic scraper: correct for every platform, merely unprofiled.
        "generic"
    };

    if !opts.quiet && !opts.json {
        describe(&detection, kind);
        eprintln!("Name: {name}");
        eprintln!("Config: {}", config_file.display());
    }

    if !opts.yes && !confirm("Add this source?")? {
        eprintln!("Not added.");
        return Ok(());
    }

    paths.ensure_created()?;
    std::fs::create_dir_all(paths.sources_dir())?;
    let yaml = render_config(&name, kind, &url, opts.category, opts.insecure);
    std::fs::write(&config_file, &yaml)
        .with_context(|| format!("writing {}", config_file.display()))?;

    // Round-trip through the real parser before pulling. If this fails, the
    // bug is here, not in the user's input — remove the file rather than
    // leaving a config the rest of the CLI will keep rejecting.
    let config = match SourceConfig::parse_file(&config_file) {
        Ok(config) => config,
        Err(e) => {
            let _ = std::fs::remove_file(&config_file);
            return Err(e).context(
                "`tome add` wrote a config its own parser rejects — this is a bug in tome",
            );
        }
    };

    if !opts.quiet {
        eprintln!("Created {}", config_file.display());
        eprintln!("Fetching documentation…");
    }

    let report = crate::pull_source(paths, &config, opts.quiet)?;

    if opts.json {
        println!(
            "{}",
            serde_json::json!({
                "id": id.as_str(),
                "name": name,
                "config": config_file.display().to_string(),
                "detected": {
                    "platform": detection.platform.as_str(),
                    "confidence": detection.confidence,
                    "confident": detection.is_confident(),
                },
                "type": kind,
                "pull": crate::report_json(&report),
            })
        );
    } else {
        crate::report_human(id.as_str(), &report);
        println!("Done. `{}` is now available in Tome.", id.as_str());
    }
    Ok(())
}

/// The config `source.type` a detected platform maps to.
///
/// Sphinx maps to `readthedocs` because that is the config schema's name for
/// the Sphinx scraper (ReadTheDocs is Sphinx, hosted — `detect::Platform`
/// says why there is no separate label). The platforms with no profile of
/// their own crawl generically; the profiles only ever *remove* furniture, so
/// generic is the safe direction to fall.
fn config_type(platform: Platform) -> &'static str {
    match platform {
        Platform::Sphinx => "readthedocs",
        Platform::Rustdoc => "rustdoc",
        Platform::MdBook => "mdbook",
        Platform::GitBook | Platform::Docusaurus | Platform::MkDocs | Platform::Generic => {
            "generic"
        }
    }
}

fn describe(detection: &Detection, kind: &str) {
    if detection.is_confident() {
        eprintln!(
            "Detected: {} (confidence {:.2}) → source type `{kind}`",
            detection.platform.as_str(),
            detection.confidence,
        );
    } else {
        eprintln!(
            "Detection is not confident (best guess: {} at {:.2}); \
             the generic scraper will be used.",
            detection.platform.as_str(),
            detection.confidence,
        );
    }
}

/// The YAML `tome add` writes. Scalars go through [`yaml_scalar`] — a name
/// with a `:` in it must not become a mapping key.
fn render_config(
    name: &str,
    kind: &str,
    url: &Url,
    category: Option<&str>,
    insecure: bool,
) -> String {
    let mut yaml = String::new();
    yaml.push_str("# Written by `tome add`. Edit freely; the schema is docs/PRD.md Appendix A.\n");
    yaml.push_str("schema_version: 1\n");
    yaml.push_str(&format!("name: {}\n", yaml_scalar(name)));
    yaml.push_str("source:\n");
    yaml.push_str(&format!("  type: {kind}\n"));
    yaml.push_str(&format!("  url: {}\n", yaml_scalar(url.as_str())));
    if kind == "generic" {
        // The parser would default these identically; they are written out so
        // the crawl's bounds are a visible knob rather than invisible
        // behaviour (P1-022 refuses unbounded crawls for the same reason).
        let defaults = GenericScraperConfig::default();
        yaml.push_str("  generic:\n");
        yaml.push_str(&format!("    max_depth: {}\n", defaults.max_depth));
        yaml.push_str(&format!("    max_pages: {}\n", defaults.max_pages));
    }
    if let Some(category) = category {
        yaml.push_str(&format!("category: {}\n", yaml_scalar(category)));
    }
    if insecure {
        yaml.push_str("fetch:\n  allow_insecure: true\n");
    }
    yaml
}

/// Quote a string for YAML. JSON string syntax is valid YAML, and
/// `serde_json` already knows how to escape it — no hand-rolled quoting.
fn yaml_scalar(text: &str) -> String {
    serde_json::Value::String(text.to_owned()).to_string()
}

/// Derive a source id from a URL: meaningful host labels plus path segments.
///
/// `https://docs.python.org/3/` → `python-3`,
/// `https://doc.rust-lang.org/std/` → `rust-lang-std`. The rules are
/// deliberately dumb — drop `www`, drop a leading `docs`/`doc` when something
/// meaningful remains, drop the TLD — because the id is a suggestion the user
/// sees and confirms, not an identity that has to be right.
fn derive_id(url: &Url) -> Result<SourceId> {
    let host = url.host_str().unwrap_or("source");
    let mut labels: Vec<&str> = host.split('.').collect();
    if labels.first().is_some_and(|l| *l == "www") {
        labels.remove(0);
    }
    if labels.len() >= 3 && matches!(labels[0], "docs" | "doc") {
        labels.remove(0);
    }
    if labels.len() >= 2 {
        labels.pop(); // the TLD says nothing about the content
    }

    let mut parts: Vec<String> = labels.into_iter().map(sanitize).collect();
    for segment in url.path_segments().into_iter().flatten() {
        if segment.is_empty() || segment.contains('.') {
            continue; // `index.html` is not part of anyone's name
        }
        parts.push(sanitize(segment));
    }
    id_from_text(&parts.join("-"))
}

fn id_from_text(text: &str) -> Result<SourceId> {
    let sanitized = sanitize(text);
    let trimmed = sanitized.trim_matches('-');
    let capped = if trimmed.len() > 64 {
        trimmed[..64].trim_end_matches('-')
    } else {
        trimmed
    };
    Ok(SourceId::new(capped)?)
}

/// Lowercase; anything a `SourceId` cannot hold becomes `-`.
fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| {
            let c = c.to_ascii_lowercase();
            if c.is_ascii_alphanumeric() || ".-_+".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Ask on stderr, so a piped stdout stays parseable.
fn confirm(question: &str) -> Result<bool> {
    eprint!("{question} [Y/n] ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let answer = line.trim().to_ascii_lowercase();
    Ok(answer.is_empty() || answer == "y" || answer == "yes")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn id_for(url: &str) -> String {
        derive_id(&Url::parse(url).expect("valid url"))
            .expect("derivable")
            .as_str()
            .to_owned()
    }

    #[test]
    fn ids_drop_noise_and_keep_meaning() {
        assert_eq!(id_for("https://docs.python.org/3/"), "python-3");
        assert_eq!(id_for("https://doc.rust-lang.org/std/"), "rust-lang-std");
        assert_eq!(id_for("https://www.example.com/"), "example");
        // Two labels: dropping `docs` would leave only the TLD.
        assert_eq!(id_for("https://docs.rs/"), "docs");
    }

    #[test]
    fn every_platform_maps_to_a_valid_config_type() {
        // The strings here are `source.type` values; a typo would surface as
        // a config the parser rejects *after* `tome add` claimed success.
        for platform in Platform::ALL {
            let kind = config_type(platform);
            assert!(
                ["readthedocs", "rustdoc", "mdbook", "generic"].contains(&kind),
                "{kind:?} is not a config source.type"
            );
        }
    }

    #[test]
    fn rendered_config_parses_with_the_real_parser() {
        // Hostile-ish name: a colon and quotes must not break the YAML.
        let url = Url::parse("https://docs.example.org/v2/").expect("valid url");
        let yaml = render_config(
            "Widget: the \"docs\"",
            "readthedocs",
            &url,
            Some("C: drive"),
            false,
        );
        let id = SourceId::new("widget").expect("valid id");
        let parsed = SourceConfig::parse_str(id, &yaml, std::path::Path::new("widget.yaml"))
            .expect("tome add must not write a config the parser rejects");
        assert_eq!(parsed.name, "Widget: the \"docs\"");
        assert_eq!(parsed.category, "C: drive");
    }

    #[test]
    fn generic_config_carries_explicit_caps() {
        let url = Url::parse("https://example.org/").expect("valid url");
        let yaml = render_config("Example", "generic", &url, None, true);
        assert!(yaml.contains("max_depth:"), "caps must be visible: {yaml}");
        assert!(yaml.contains("max_pages:"), "caps must be visible: {yaml}");
        assert!(yaml.contains("allow_insecure: true"));
        let id = SourceId::new("example").expect("valid id");
        SourceConfig::parse_str(id, &yaml, std::path::Path::new("example.yaml"))
            .expect("generic config parses");
    }
}
