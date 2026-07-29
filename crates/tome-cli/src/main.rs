//! The `tome` command-line interface.
//!
//! Scaffold only — subcommands are stubs. The surface is specified in
//! `docs/PRD.md` § CLI Specification, which is the authoritative list; nothing
//! that is not on it should appear here.
//!
//! Two constraints that shape this binary from the start:
//!
//! * It shares [`tome_core::Paths`] with the desktop app. Same library, same
//!   files. See `docs/decisions/0002-no-app-sandbox.md`.
//! * `tome mcp` speaks JSON-RPC over **stdout**. Nothing else may write there —
//!   a single stray `println!` corrupts the stream and the client disconnects
//!   with an opaque parse error. All diagnostics go to stderr.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tome_core::config::SourceConfig;
use tome_core::db::Database;
use tome_core::model::SourceId;
use tome_core::search::SearchEngine;
use tome_core::Paths;

#[derive(Parser)]
#[command(
    name = "tome",
    version,
    about = "A personal library for technical documentation."
)]
struct Cli {
    /// Output as JSON, for scripting.
    #[arg(long, global = true)]
    json: bool,

    /// Suppress non-essential output.
    #[arg(long, short, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Add a documentation source.
    Add { target: String },
    /// Fetch or update documentation content.
    Pull {
        source: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Search documentation.
    Search {
        query: String,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// List all sources.
    List,
    /// Remove a source.
    Remove { source: String },
    /// Show sync and index status, and where the library lives.
    Status,
    /// Start the local HTTP API server.
    Serve {
        #[arg(long, default_value_t = 7431)]
        port: u16,
    },
    /// Start the MCP server (stdio by default).
    Mcp {
        /// Use Streamable HTTP instead of stdio.
        #[arg(long)]
        http: bool,
        #[arg(long, default_value_t = 7432)]
        port: u16,
    },
}

fn main() -> Result<()> {
    // stderr, always: stdout belongs to `tome mcp` and to `--json` output.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,tome=info")),
        )
        .init();

    let cli = Cli::parse();
    let paths = Paths::resolve()?;

    match cli.command {
        Command::Pull { source, all } => pull(&paths, source.as_deref(), all, cli.quiet)?,
        Command::List => list(&paths, cli.json)?,
        Command::Status => {
            // The one command that does something real so far: it proves the
            // CLI and the app agree on where the library lives.
            println!(
                "Tome {} ({})",
                env!("CARGO_PKG_VERSION"),
                tome_core::BUNDLE_ID
            );
            println!("  state:  {}", paths.state_root().display());
            println!("  cache:  {}", paths.cache_root().display());
            println!("  db:     {}", paths.database_file().display());
            println!("  index:  {}", paths.index_dir().display());
            let exists = paths.state_root().exists();
            println!(
                "  status: {}",
                if exists {
                    "initialised"
                } else {
                    "not yet initialised"
                }
            );
        }
        Command::Add { .. } => {
            // P1-022 owns the interactive add workflow. Until it lands there
            // IS a way to add a source, and saying what it is beats a bare
            // "not implemented" that leaves the reader with nothing to read.
            anyhow::bail!(
                "`tome add` is not implemented yet (P1-022).\n\
                 Until it lands, write the source configuration yourself:\n  \
                 {}/<source-id>.yaml\n\
                 then run `tome pull <source-id>`. The schema is in \
                 docs/PRD.md Appendix A.",
                paths.sources_dir().display()
            );
        }
        Command::Search {
            query,
            scope,
            limit,
        } => search(&paths, &query, scope.as_deref(), limit, cli.json)?,
        other => {
            let name = match other {
                Command::Remove { .. } => "remove",
                Command::Serve { .. } => "serve",
                Command::Mcp { .. } => "mcp",
                Command::Add { .. }
                | Command::Pull { .. }
                | Command::List
                | Command::Search { .. }
                | Command::Status => {
                    unreachable!("handled above")
                }
            };
            anyhow::bail!("`tome {name}` is not implemented yet (scaffold only).");
        }
    }

    Ok(())
}

/// Every source configuration on disk, as `(id, path)`.
///
/// The sources directory is the source of truth for what CAN be pulled; the
/// database records what HAS been. A config with no database row is a source
/// that has never been pulled, which `tome list` shows rather than hiding.
fn source_configs(paths: &Paths) -> Result<Vec<(SourceId, PathBuf)>> {
    let dir = paths.sources_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", dir.display())),
    };

    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "yaml") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        match SourceId::new(stem) {
            Ok(id) => found.push((id, path)),
            // A file whose name is not a valid source id is not a source.
            // Warn rather than fail: one bad file must not block `--all`.
            Err(e) => tracing::warn!("skipping {}: {e}", path.display()),
        }
    }
    found.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    Ok(found)
}

fn pull(paths: &Paths, source: Option<&str>, all: bool, quiet: bool) -> Result<()> {
    // `pull` writes, so it creates the library. `list` deliberately does not.
    paths.ensure_created()?;
    let available = source_configs(paths)?;
    if available.is_empty() {
        anyhow::bail!(
            "No source configurations found in {}.\n\
             Add one before pulling; see docs/PRD.md Appendix A for the schema.",
            paths.sources_dir().display()
        );
    }

    let selected: Vec<_> = match (source, all) {
        (Some(_), true) => anyhow::bail!("Pass a source or --all, not both."),
        (Some(name), false) => {
            let matched: Vec<_> = available
                .iter()
                .filter(|(id, _)| id.as_str() == name)
                .collect();
            if matched.is_empty() {
                let known: Vec<&str> = available.iter().map(|(id, _)| id.as_str()).collect();
                anyhow::bail!("No source named `{name}`. Known: {}", known.join(", "));
            }
            matched
        }
        (None, true) => available.iter().collect(),
        (None, false) => anyhow::bail!("Name a source, or pass --all."),
    };

    for (id, config_path) in selected {
        let config = SourceConfig::parse_file(config_path)
            .with_context(|| format!("reading {}", config_path.display()))?;

        if !quiet {
            eprintln!("Pulling {id}…");
        }

        // Progress goes to stderr, deliberately: stdout belongs to `--json`
        // output and to `tome mcp`, and a progress line in a piped JSON
        // stream is a parse error at the other end.
        let mut last_reported = 0usize;
        let report = tome_core::pipeline::pull(paths, &config, &mut |progress| {
            if quiet {
                return;
            }
            if let tome_core::pipeline::Progress::Crawled {
                crawled, queued, ..
            } = progress
            {
                if crawled > last_reported {
                    last_reported = crawled;
                    eprint!("\r  {crawled} pages fetched, {queued} queued   ");
                }
            }
        })?;
        if !quiet && last_reported > 0 {
            eprintln!();
        }

        println!(
            "{id}: {} pages in {:.1}s",
            report.pages_stored,
            report.elapsed.as_secs_f64()
        );

        // Everything that went wrong, said out loud. A pull that reports
        // success while having silently skipped 200 pages is worse than one
        // that says what it missed.
        if report.hit_page_cap {
            println!("  stopped at the page cap — the source has more pages than were fetched");
        }
        if !report.page_errors.is_empty() {
            println!("  {} pages could not be fetched:", report.page_errors.len());
            for error in report.page_errors.iter().take(10) {
                println!("    {error}");
            }
            if report.page_errors.len() > 10 {
                println!("    … and {} more", report.page_errors.len() - 10);
            }
        }
        if !report.asset_errors.is_empty() {
            println!(
                "  {} assets could not be localized (those images show a placeholder)",
                report.asset_errors.len()
            );
        }
        if let Some(index) = report.index {
            if index.rebuilt {
                println!("  search index was unreadable and has been rebuilt");
            }
            if index.is_noop() {
                println!(
                    "  search index already up to date ({} pages)",
                    index.unchanged
                );
            } else {
                // Named counts rather than a single total: "12 indexed" hides
                // whether a re-pull found real changes or re-did work.
                println!(
                    "  search index: {} added, {} updated, {} removed, {} unchanged",
                    index.added, index.updated, index.removed, index.unchanged
                );
            }
        }
    }

    Ok(())
}

fn list(paths: &Paths, json: bool) -> Result<()> {
    let configs = source_configs(paths)?;
    // A read-only command must not create the library. On a machine where
    // nothing has been pulled yet there is no database, and `tome list`
    // saying "empty" is the correct answer -- not an error, and not a reason
    // to make ~/Library/Application Support/Tome exist.
    let database = paths
        .database_file()
        .exists()
        .then(|| Database::open(paths))
        .transpose()?;
    let pulled = match &database {
        Some(database) => database.list_sources()?,
        None => Vec::new(),
    };

    if json {
        // One shape, always — an empty library prints `{"sources":[]}`, not
        // nothing, so a script can `jq` it without special-casing.
        let sources: Vec<_> = configs
            .iter()
            .map(|(id, _)| {
                let row = pulled.iter().find(|s| s.id == *id);
                serde_json::json!({
                    "id": id.as_str(),
                    "name": row.map(|s| s.name.clone()),
                    "category": row.map(|s| s.category.clone()),
                    // The live row count, the same number the human-readable
                    // output prints. Reading `Source.page_count` here instead
                    // made the two disagree whenever the stored field was
                    // stale.
                    "pages": row
                        .and_then(|s| database.as_ref().and_then(|db| db.page_count(&s.id).ok()))
                        .unwrap_or(0),
                    "pulled": row.is_some(),
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "sources": sources }));
        return Ok(());
    }

    if configs.is_empty() {
        println!(
            "No sources yet. Configurations go in {}.",
            paths.sources_dir().display()
        );
        return Ok(());
    }

    for (id, _) in &configs {
        match pulled.iter().find(|s| s.id == *id) {
            Some(source) => println!(
                "{:<24} {:<8} {}",
                id.as_str(),
                database
                    .as_ref()
                    .and_then(|db| db.page_count(id).ok())
                    .unwrap_or(source.page_count),
                source.name
            ),
            None => println!("{:<24} {:<8} (never pulled)", id.as_str(), "-"),
        }
    }
    Ok(())
}

/// `tome search` (P4-005, brought forward by S2-3).
///
/// S2-3 wires indexing into `pull`; without a way to query it, "search works"
/// would be a claim resting entirely on tests. This is the minimum that makes
/// it checkable by hand. Result snippets (P2-005) and `--scope` accepting a
/// category rather than only a source id are still P4-005's to finish.
fn search(paths: &Paths, query: &str, scope: Option<&str>, limit: usize, json: bool) -> Result<()> {
    // Read-only, so it must not bring a library into existence — same rule as
    // `tome list`. A machine that has pulled nothing has an empty index, and
    // "no results" is the correct answer rather than an error.
    if !paths.index_dir().exists() {
        if json {
            println!("{}", serde_json::json!({ "results": [] }));
        } else {
            println!("No results — nothing has been pulled yet. Try `tome pull <source>`.");
        }
        return Ok(());
    }

    let engine = SearchEngine::open(paths)?;
    // Over-fetch when scoping, because filtering happens after ranking and
    // would otherwise return fewer than `limit` results from a large library.
    // Scoping in the query itself is P2-016; this is the honest stopgap, and
    // it is bounded so a huge limit cannot ask for the whole index.
    let fetch = if scope.is_some() {
        limit.saturating_mul(10).min(1000)
    } else {
        limit
    };

    let hits: Vec<_> = engine
        .search(query, fetch)?
        .into_iter()
        .filter(|hit| scope.is_none_or(|s| hit.source.as_str() == s))
        .take(limit)
        .collect();

    if json {
        println!(
            "{}",
            serde_json::json!({
                "results": hits.iter().map(|hit| serde_json::json!({
                    "source": hit.source.as_str(),
                    "path": hit.path,
                    "title": hit.title,
                    "score": hit.score,
                })).collect::<Vec<_>>()
            })
        );
        return Ok(());
    }

    if hits.is_empty() {
        println!("No results for {query:?}.");
        return Ok(());
    }
    for hit in &hits {
        println!("{:<24} {}", hit.source.as_str(), hit.title);
        println!("{:<24} {}", "", hit.path);
    }
    Ok(())
}
