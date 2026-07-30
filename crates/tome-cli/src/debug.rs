//! `tome debug` — diagnostics and recovery (P5-004, P5-005).
//!
//! Hidden from the top-level `--help` because nobody should need it on an
//! ordinary day, and specified in `docs/PRD.md` § CLI Specification because
//! everything reachable is.
//!
//! Three subcommands, and the split between them is deliberate:
//!
//! * **`check`** answers "is anything wrong", and never repairs. A diagnostic
//!   that fixes things cannot be run twice to see whether the fix worked.
//! * **`rebuild-index`** repairs the one thing that is *safe* to repair. The
//!   index is derived and lives under the cache root; SPIKE-003 measured a
//!   rebuild at 5–21 s for 100 000 pages against about seven hours to
//!   re-crawl. Nothing else here deletes anything.
//! * **`report`** produces something a person can paste into an issue, with
//!   the reading history taken out. There is no telemetry and never will be,
//!   so this is the only path from a broken machine to a bug report — which
//!   means it has to be worth reading *and* safe to publish.
//!
//! **The redaction rule for `report`:** the home directory, page paths, search
//! queries and note text never appear. `crate::error` already keeps user
//! content out of error messages, so the log lines are safe by construction;
//! the home directory is not, so it is rewritten to `~`.

use anyhow::{Context, Result};
use tome_core::config::SourceConfig;
use tome_core::db::Database;
use tome_core::search::SearchEngine;
use tome_core::Paths;

/// One thing that was checked, and what was found.
struct Finding {
    /// What was checked, in the imperative: "the database opens".
    subject: String,
    ok: bool,
    /// Present when `ok` is false. Says what to do, not what happened.
    remedy: Option<String>,
}

impl Finding {
    fn ok(subject: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            ok: true,
            remedy: None,
        }
    }
    fn bad(subject: impl Into<String>, remedy: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            ok: false,
            remedy: Some(remedy.into()),
        }
    }
}

/// `tome debug check`.
///
/// Exits non-zero when something is wrong, so `tome debug check && …` is
/// usable in a script.
pub(crate) fn check(paths: &Paths, json: bool) -> Result<()> {
    let findings = run_checks(paths);
    let failures = findings.iter().filter(|f| !f.ok).count();

    if json {
        let items: Vec<serde_json::Value> = findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "check": f.subject,
                    "ok": f.ok,
                    "remedy": f.remedy,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({ "checks": items, "problems": failures })
        );
    } else {
        for finding in &findings {
            let mark = if finding.ok { "ok  " } else { "FAIL" };
            println!("{mark}  {}", finding.subject);
            if let Some(remedy) = &finding.remedy {
                println!("      → {remedy}");
            }
        }
        println!();
        if failures == 0 {
            println!("Nothing wrong.");
        } else {
            println!("{failures} problem(s). Each line above says what to do.");
        }
    }

    if failures > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn run_checks(paths: &Paths) -> Vec<Finding> {
    let mut findings = Vec::new();

    if !paths.state_root().exists() {
        // Not a failure: a machine that has pulled nothing is in a valid
        // state, and reporting it as broken sends people looking for a fault
        // that is a first run.
        findings.push(Finding::ok(
            "the library has not been created yet (nothing has been pulled)",
        ));
        return findings;
    }

    findings.push(match writable(paths.state_root()) {
        true => Finding::ok("the library directory is writable"),
        false => Finding::bad(
            "the library directory is not writable",
            format!(
                "check permissions on {}, or set $TOME_HOME elsewhere",
                paths.state_root().display()
            ),
        ),
    });

    // The database. Opening it runs the migrations, so this also reports a
    // schema this build cannot read.
    let database = match Database::open(paths) {
        Ok(db) => {
            findings.push(Finding::ok("the database opens"));
            Some(db)
        }
        Err(e) => {
            findings.push(Finding::bad(
                format!("the database will not open: {e}"),
                format!(
                    "move {} aside and run `tome pull --all` to rebuild it — \
                     bookmarks and annotations are in that file and are not re-fetchable",
                    paths.database_file().display()
                ),
            ));
            None
        }
    };

    // Every source config must parse. `add` round-trips before writing, so a
    // config that fails here was hand-edited or shipped by an older build.
    let mut configs = Vec::new();
    match std::fs::read_dir(paths.sources_dir()) {
        Ok(entries) => {
            let mut bad = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                    continue;
                }
                match SourceConfig::parse_file(&path) {
                    Ok(config) => configs.push(config),
                    Err(e) => bad.push(format!(
                        "{}: {e}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    )),
                }
            }
            if bad.is_empty() {
                findings.push(Finding::ok(format!(
                    "all {} source configuration(s) parse",
                    configs.len()
                )));
            } else {
                findings.push(Finding::bad(
                    format!("{} source configuration(s) do not parse", bad.len()),
                    format!("fix or remove: {}", bad.join("; ")),
                ));
            }
        }
        Err(_) => findings.push(Finding::ok("no source configurations yet")),
    }

    // The index. `open` rather than `open_or_rebuild`: a diagnostic must not
    // delete the thing it is diagnosing.
    match SearchEngine::open(paths) {
        Ok(engine) => {
            findings.push(Finding::ok("the search index opens"));
            if let Some(database) = &database {
                findings.extend(index_agrees_with_database(&engine, database, &configs));
            }
        }
        Err(e) => findings.push(Finding::bad(
            format!("the search index will not open: {e}"),
            "run `tome debug rebuild-index` — the index is derived from local \
             content and needs no network"
                .to_owned(),
        )),
    }

    findings
}

/// Whether every source's page count in the database matches the index.
///
/// This is the failure that has no symptom: an interrupted pull leaves the
/// database ahead of the index, search quietly misses pages that are on disk,
/// and nothing ever says so.
fn index_agrees_with_database(
    engine: &SearchEngine,
    database: &Database,
    configs: &[SourceConfig],
) -> Vec<Finding> {
    let sources = match database.list_sources() {
        Ok(sources) => sources,
        Err(e) => {
            return vec![Finding::bad(
                format!("could not list sources: {e}"),
                "the database may be damaged; `tome debug check` again after \
                 `tome pull --all`"
                    .to_owned(),
            )]
        }
    };

    let mut behind = Vec::new();
    for source in &sources {
        let stored = database.page_count(&source.id).unwrap_or(0);
        let indexed = engine
            .indexed_pages(&source.id)
            .map(|pages| pages.len() as u32)
            .unwrap_or(0);
        if stored != indexed {
            behind.push(format!("{}: {stored} stored, {indexed} indexed", source.id));
        }
    }

    let mut findings = Vec::new();
    if behind.is_empty() {
        findings.push(Finding::ok(format!(
            "the index and the database agree on all {} source(s)",
            sources.len()
        )));
    } else {
        findings.push(Finding::bad(
            format!("the index is out of step for {} source(s)", behind.len()),
            format!(
                "run `tome debug rebuild-index` (no network needed): {}",
                behind.join("; ")
            ),
        ));
    }

    // A configuration with no rows is a source that was added and never
    // pulled. Worth saying, and not a fault.
    let unpulled: Vec<&str> = configs
        .iter()
        .filter(|c| !sources.iter().any(|s| s.id.as_str() == c.id.as_str()))
        .map(|c| c.id.as_str())
        .collect();
    if !unpulled.is_empty() {
        findings.push(Finding::ok(format!(
            "{} source(s) added but never pulled: {}",
            unpulled.len(),
            unpulled.join(", ")
        )));
    }
    findings
}

fn writable(dir: &std::path::Path) -> bool {
    // Asking the filesystem rather than reading the mode bits: ACLs, and a
    // read-only mount, both produce a writable-looking mode.
    let probe = dir.join(".tome-write-probe");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// `tome debug rebuild-index`.
///
/// Discards the index and rebuilds it from the pages already on disk. No
/// network, and nothing irreplaceable is touched: the database, the source
/// configurations and the cached page content are all left alone.
pub(crate) fn rebuild_index(paths: &Paths, json: bool, quiet: bool) -> Result<()> {
    let index_dir = paths.index_dir();
    if index_dir.exists() {
        // Remove rather than reuse. A half-written segment is exactly what
        // would fail to open a second time too, and `index_source` skips
        // pages whose indexed hash already matches — so reusing the directory
        // would rebuild nothing.
        std::fs::remove_dir_all(&index_dir)
            .with_context(|| format!("removing {}", index_dir.display()))?;
    }

    let database = Database::open(paths)?;
    let sources = database.list_sources()?;

    let mut rebuilt = Vec::new();
    for source in &sources {
        if !quiet && !json {
            eprintln!("indexing {}…", source.id);
        }
        let report = tome_core::pipeline::index_source(
            paths,
            &source.id,
            &source.category,
            &mut |_progress| {},
        )
        .with_context(|| format!("indexing {}", source.id))?;
        rebuilt.push((source.id.clone(), report.added + report.updated));
    }

    if json {
        let sources: Vec<serde_json::Value> = rebuilt
            .iter()
            .map(|(id, pages)| serde_json::json!({ "source": id.as_str(), "pages": pages }))
            .collect();
        println!("{}", serde_json::json!({ "rebuilt": sources }));
        return Ok(());
    }

    let total: usize = rebuilt.iter().map(|(_, pages)| *pages).sum();
    if rebuilt.is_empty() {
        println!("There was nothing to index — no source has been pulled yet.");
    } else {
        println!(
            "Rebuilt the index: {total} page(s) across {} source(s).",
            rebuilt.len()
        );
    }
    Ok(())
}

/// `tome debug report` — a redacted diagnostic bundle, on stdout.
///
/// Printed rather than written to a file: the whole point is to paste it into
/// an issue, and a command that says "wrote a file, now go find it" adds a
/// step and an opportunity to attach the wrong one.
pub(crate) fn report(paths: &Paths, lines: usize) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_default();
    // Every path printed below goes through this. The username is the one
    // piece of identifying information that would otherwise be in every
    // single line.
    let redact = |text: &str| -> String {
        if home.is_empty() {
            text.to_owned()
        } else {
            text.replace(&home, "~")
        }
    };

    println!("tome {}", env!("CARGO_PKG_VERSION"));
    println!("bundle {}", tome_core::BUNDLE_ID);
    println!("macos  {}", os_version());
    println!(
        "state  {}",
        redact(&paths.state_root().display().to_string())
    );
    println!(
        "cache  {}",
        redact(&paths.cache_root().display().to_string())
    );
    println!(
        "home   {}",
        if std::env::var_os(tome_core::paths::TOME_HOME_ENV).is_some() {
            "TOME_HOME is set"
        } else {
            "default"
        }
    );
    println!();

    println!("## sources");
    match Database::open(paths).and_then(|db| {
        db.list_sources().map(|sources| {
            sources
                .into_iter()
                .map(|s| {
                    let count = db.page_count(&s.id).unwrap_or(0);
                    (s.id, s.kind, count, s.last_synced)
                })
                .collect::<Vec<_>>()
        })
    }) {
        // Source ids and page counts, never page paths. Which documentation
        // someone has is part of a bug report; which page they were reading
        // is not.
        Ok(sources) if sources.is_empty() => println!("(none)"),
        Ok(sources) => {
            for (id, kind, pages, synced) in sources {
                let synced =
                    synced.map_or("never".to_owned(), |t| t.format("%Y-%m-%d").to_string());
                println!("{id}  {kind:?}  {pages} pages  last pulled {synced}");
            }
        }
        Err(e) => println!("(could not be read: {e})"),
    }
    println!();

    println!("## checks");
    for finding in run_checks(paths) {
        println!(
            "{}  {}",
            if finding.ok { "ok  " } else { "FAIL" },
            redact(&finding.subject)
        );
    }
    println!();

    println!("## log (last {lines} lines)");
    match recent_log_lines(paths, lines) {
        Ok(log) if log.is_empty() => println!("(empty)"),
        Ok(log) => {
            for line in log {
                println!("{}", redact(&line));
            }
        }
        Err(e) => println!("(could not be read: {e})"),
    }
    Ok(())
}

/// The tail of today's log file.
fn recent_log_lines(paths: &Paths, lines: usize) -> Result<Vec<String>> {
    let dir = paths.logs_dir();
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("log"))
                .collect()
        })
        .unwrap_or_default();
    // Names are `tome-YYYY-MM-DD.log`, so lexical order is chronological.
    files.sort();
    let Some(newest) = files.last() else {
        return Ok(Vec::new());
    };
    let body =
        std::fs::read_to_string(newest).with_context(|| format!("reading {}", newest.display()))?;
    let all: Vec<&str> = body.lines().collect();
    Ok(all
        .iter()
        .skip(all.len().saturating_sub(lines))
        .map(|l| (*l).to_owned())
        .collect())
}

fn os_version() -> String {
    std::process::Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}
