//! First-run onboarding: install a documentation source in one click (S4-4).
//!
//! P5-006 calls the registry-first path "the single highest-leverage thing in
//! onboarding: the difference between a product and a configuration
//! exercise". Before this, an empty library told the user to write YAML and
//! run a CLI command — which is a fine answer for the person who built it and
//! no answer at all for anyone else.
//!
//! **The registry ships inside the bundle**, as a Tauri resource, rather than
//! being fetched. Two reasons, and the second is the one that decided it:
//!
//! 1. P5-006 requires onboarding to work with no network, explaining what it
//!    needs rather than failing opaquely. A catalogue that has to be
//!    downloaded before it can say "you are offline" cannot do that.
//! 2. The catalogue is *configuration*, and configuration that arrives over
//!    the network at first launch is a supply chain. The copy in the bundle
//!    was reviewed when the release was cut.
//!
//! Installing still fetches, of course — from the documentation's own origin,
//! through the ordinary pipeline, with robots.txt, the rate limit and the
//! SSRF filter all inherited rather than re-implemented.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager};
use tome_core::config::SourceConfig;
use tome_core::pipeline::{self, Progress};
use tome_core::registry::Index;
use tome_core::Paths;

/// Where the registry lives inside the bundle.
///
/// Mirrored by `bundle.resources` in `tauri.conf.json`. If those two ever
/// disagree the catalogue is empty and onboarding silently offers nothing, so
/// `catalogue` reports the resolved path in its error rather than returning an
/// empty list.
const RESOURCE_DIR: &str = "registry";

/// One installable source, as the UI shows it.
#[derive(serde::Serialize)]
pub struct CatalogueEntry {
    id: String,
    name: String,
    category: String,
    homepage: String,
    licence: String,
    /// The date the verification job last confirmed this configuration still
    /// produces pages. Shown, not hidden: a stale date is the only warning a
    /// user gets that a scraper may have rotted.
    verified: String,
    /// Whether this source is already in the library, so the UI can say
    /// "installed" instead of offering to install it twice.
    installed: bool,
}

fn registry_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resolve(RESOURCE_DIR, tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("the bundled registry could not be located: {e}"))
}

/// The bundled catalogue, with everything already installed marked.
#[tauri::command]
pub fn registry_catalogue(app: AppHandle) -> Result<Vec<CatalogueEntry>, String> {
    let root = registry_root(&app)?;
    let index = Index::read(&root).map_err(|e| {
        format!(
            "the bundled registry at {} could not be read: {e}",
            root.display()
        )
    })?;
    let paths = Paths::resolve().map_err(|e| e.to_string())?;

    Ok(index
        .sources
        .into_iter()
        .map(|entry| {
            // "Installed" means a configuration exists, not that it has been
            // pulled: a source added and never pulled must not be offered
            // again, or the second install overwrites the first's config.
            let installed = tome_core::model::SourceId::new(&entry.id)
                .map(|id| paths.source_config_file(&id).exists())
                .unwrap_or(false);
            CatalogueEntry {
                id: entry.id,
                name: entry.name,
                category: entry.category,
                homepage: entry.homepage,
                licence: entry.licence,
                verified: entry.verified,
                installed,
            }
        })
        .collect())
}

/// What an install produced.
#[derive(serde::Serialize, Clone)]
pub struct InstallReport {
    source_id: String,
    pages: usize,
    /// Pages that could not be fetched or parsed. Not fatal, and reported
    /// rather than swallowed — a source that produced 40 of 200 pages looks
    /// like a success from the page count alone.
    page_errors: usize,
    /// True when the crawl stopped at the configured page cap rather than
    /// running out of links, so the UI can say "capped" instead of implying
    /// the source is complete.
    capped: bool,
}

/// Progress, pushed to the UI while a pull runs.
///
/// A first pull takes minutes, which P5-006 requires to be visible. The event
/// carries the source id because a second install can start before the first
/// finishes — the UI would otherwise attribute one source's progress to
/// another.
#[derive(serde::Serialize, Clone)]
struct ProgressEvent {
    source_id: String,
    /// `crawling`, `storing`, or `indexing`.
    phase: &'static str,
    done: usize,
    /// Zero while crawling: the total is not known until the crawl ends, and
    /// an invented denominator produces a progress bar that goes backwards.
    total: usize,
}

/// Install a registry source: write its configuration, then pull it.
///
/// Returns only when the pull has finished. The work runs on a blocking
/// thread — a pull is minutes of synchronous network and disk, and running it
/// on the async runtime's worker would stall every other command including
/// the ones the UI needs to stay responsive.
#[tauri::command]
pub async fn install_registry_source(app: AppHandle, id: String) -> Result<InstallReport, String> {
    tauri::async_runtime::spawn_blocking(move || install_blocking(&app, &id))
        .await
        .map_err(|e| format!("the install task did not finish: {e}"))?
}

fn install_blocking(app: &AppHandle, id: &str) -> Result<InstallReport, String> {
    let root = registry_root(app)?;
    let index = Index::read(&root).map_err(|e| e.to_string())?;
    let entry = index
        .get(id)
        .ok_or_else(|| format!("`{id}` is not in the registry."))?;

    let source = tome_core::model::SourceId::new(&entry.id).map_err(|e| e.to_string())?;
    let paths = Paths::resolve().map_err(|e| e.to_string())?;
    paths.ensure_created().map_err(|e| e.to_string())?;

    // Parse before writing. `tome add` established the rule and it holds here
    // for a different reason: a bundled configuration that this build's parser
    // rejects is a release defect, and it must surface as one rather than as a
    // file that `pull` will reject later with no context.
    let config_source = entry.config_path(&root).map_err(|e| e.to_string())?;
    let config = SourceConfig::parse_file(&config_source).map_err(|e| {
        format!("the bundled configuration for `{id}` is not valid for this version of Tome: {e}")
    })?;

    let destination = paths.source_config_file(&source);
    if !destination.exists() {
        std::fs::copy(&config_source, &destination)
            .map_err(|e| format!("could not write {}: {e}", destination.display()))?;
    }

    let mut on_progress = |progress: Progress| {
        let event = match progress {
            Progress::Crawled { crawled, .. } => ProgressEvent {
                source_id: entry.id.clone(),
                phase: "crawling",
                done: crawled,
                total: 0,
            },
            Progress::Storing { stored, total } => ProgressEvent {
                source_id: entry.id.clone(),
                phase: "storing",
                done: stored,
                total,
            },
            Progress::Indexing { indexed, total } => ProgressEvent {
                source_id: entry.id.clone(),
                phase: "indexing",
                done: indexed,
                total,
            },
        };
        // A failed emit is not a failed install. The window can close mid-pull
        // and the pull should still finish and be usable next launch.
        let _ = app.emit("install-progress", event);
    };

    let report = pipeline::pull(&paths, &config, &mut on_progress).map_err(|e| {
        // Both halves: what happened, and what to do about it. The suggestion
        // is the thing the taxonomy exists to carry (S4-3).
        match e.suggestion() {
            Some(hint) => format!("{e} {hint}"),
            None => e.to_string(),
        }
    })?;

    Ok(InstallReport {
        source_id: entry.id.clone(),
        pages: report.pages_stored,
        page_errors: report.page_errors.len(),
        capped: report.hit_page_cap,
    })
}
