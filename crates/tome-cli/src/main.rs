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

mod add;
mod debug;
mod mcp;
mod mcp_tools;
mod remove;
mod serve;
mod token;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tome_core::config::SourceConfig;
use tome_core::db::Database;
use tome_core::model::SourceId;
use tome_core::pipeline::IngestReport;
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
    ///
    /// Fetches the site's homepage, detects the documentation platform, and
    /// writes a source configuration — then pulls. Interactive by default;
    /// pass --yes to confirm nothing.
    Add {
        /// The documentation site's URL.
        target: String,
        /// Skip the confirmation prompt. Required with --json.
        #[arg(long, short)]
        yes: bool,
        /// Display name (also drives the source id). Derived from the URL by
        /// default.
        #[arg(long)]
        name: Option<String>,
        /// Category shown in the library. Defaults to "Uncategorized".
        #[arg(long)]
        category: Option<String>,
        /// Allow http and private hosts — for a server you own (an intranet
        /// mirror). Written into the config as `fetch.allow_insecure`.
        #[arg(long)]
        insecure: bool,
    },
    /// Fetch or update documentation content.
    Pull {
        source: Option<String>,
        #[arg(long)]
        all: bool,
        /// With --all, pull only sources their sync strategy says are due
        /// (P4-018). Without it, --all pulls everything, because a person
        /// typing `tome pull --all` has asked for exactly that.
        #[arg(long, requires = "all")]
        due: bool,
        /// Stop after this many pages, overriding the config. For health
        /// checks — `scripts/verify-registry.sh` uses it — where the question
        /// is "does this scraper still find anything", not "fetch the site".
        #[arg(long)]
        max_pages: Option<u32>,
    },
    /// Search documentation.
    ///
    /// Prefix a term with `@` to search declared symbols only:
    /// `tome search @with_capacity` returns the pages that *declare* it,
    /// rather than every page that mentions it.
    Search {
        query: String,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// List all sources.
    List {
        /// Only sources in this category.
        #[arg(long)]
        category: Option<String>,
    },
    /// Remove a source: its config, its cached pages and assets, its
    /// database rows, and its search index entries.
    Remove {
        source: String,
        /// Skip the confirmation prompt. Required with --json.
        #[arg(long, short)]
        yes: bool,
    },
    /// Show sync and index status, and where the library lives.
    Status {
        /// Print the API bearer token (creates one if none exists yet).
        #[arg(long)]
        show_token: bool,
    },
    /// View or change configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Start the local HTTP API server (off unless you run this).
    Serve {
        /// Port to listen on. 0 picks an ephemeral port, printed on stderr.
        #[arg(long, default_value_t = 7431)]
        port: u16,
        /// Address to bind. Anything but loopback logs a warning — the
        /// bearer token becomes all that protects the library.
        #[arg(long, default_value = "127.0.0.1")]
        bind: std::net::IpAddr,
        /// Origin allowed to read API responses from a browser (repeatable).
        /// No origins means no CORS headers at all. `*` is rejected.
        #[arg(long = "allow-origin")]
        allow_origin: Vec<String>,
    },
    /// Diagnostics and recovery.
    ///
    /// Hidden from the top-level help: nothing here is part of an ordinary
    /// day, and a `debug` command in the main list invites people to reach
    /// for it before the command that would have worked.
    #[command(hide = true)]
    Debug {
        #[command(subcommand)]
        action: DebugAction,
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

#[derive(Subcommand)]
enum DebugAction {
    /// Check the library for problems. Reports; never repairs.
    Check,
    /// Discard the search index and rebuild it from local content.
    ///
    /// No network. The index is derived and lives under the cache root; the
    /// database, the configurations and the cached pages are untouched.
    RebuildIndex,
    /// Print a redacted diagnostic report, for pasting into a bug report.
    ///
    /// No page paths, no search queries, no home directory. There is no
    /// telemetry and never will be, so this is the only path from a broken
    /// machine to something a maintainer can read.
    Report {
        /// How many log lines to include.
        #[arg(long, default_value_t = 50)]
        lines: usize,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Replace the API bearer token; the old one stops working.
    RotateToken,
    /// Delete the API bearer token from the Keychain.
    ///
    /// `brew uninstall --zap` removes files, and the Keychain is not a file,
    /// so this is the only way an uninstall leaves nothing behind. The cask's
    /// caveats name it.
    ForgetToken,
}

fn main() -> Result<()> {
    // stderr, always: stdout belongs to `tome mcp` and to `--json` output.
    // And, since S4-3, to `logs/tome-<date>.log` as well, so that a bug report
    // can say what happened rather than what the reporter remembers. Nothing
    // leaves the machine — `tome debug report` is how a person shares it, and
    // it redacts.
    //
    // Resolving paths before parsing arguments: the log destination cannot
    // depend on the subcommand, or the failure being diagnosed goes unlogged
    // when it happens during parsing. A library that cannot be resolved just
    // means no file half — that error will be reported by the command itself.
    let log_file = Paths::resolve()
        .map(|paths| tome_core::logging::to_stderr_and_file(&paths))
        .ok();
    let filter = || {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,tome=info"))
    };
    match log_file {
        Some(writer) => tracing_subscriber::fmt()
            .with_writer(writer)
            // The file gets the same bytes as the terminal, so escape codes
            // would end up in it. Losing colour on stderr is the cheaper half
            // of that trade.
            .with_ansi(false)
            .with_env_filter(filter())
            .init(),
        None => tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(filter())
            .init(),
    }

    let cli = Cli::parse();
    let json = cli.json;

    match run(cli) {
        Ok(()) => Ok(()),
        Err(e) => {
            if json {
                // Errors as JSON under --json (P4-007): stderr, so a piped
                // stdout never receives half a result and then an error
                // object, and exit 1 so `&&` chains still work.
                eprintln!(
                    "{}",
                    serde_json::json!({ "error": { "message": format!("{e:#}") } })
                );
                std::process::exit(1);
            }
            Err(e)
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let paths = Paths::resolve()?;

    match cli.command {
        Command::Pull {
            source,
            all,
            due,
            max_pages,
        } => pull(
            &paths,
            source.as_deref(),
            all,
            due,
            max_pages,
            cli.quiet,
            cli.json,
        )?,
        Command::List { category } => list(&paths, category.as_deref(), cli.json)?,
        Command::Status { show_token } => status(&paths, show_token, cli.json)?,
        Command::Config { action } => match action {
            ConfigAction::RotateToken => {
                let _new = token::rotate(&paths)?;
                // The token itself is deliberately not printed here — use
                // `tome status --show-token` for that, one place only.
                println!(
                    "API token rotated. A running `tome serve` keeps the old token until restarted."
                );
            }
            ConfigAction::ForgetToken => {
                let existed = token::forget(&paths)?;
                if cli.json {
                    println!("{}", serde_json::json!({ "removed": existed }));
                } else if existed {
                    println!("API token removed. `tome serve` will mint a new one.");
                } else {
                    println!("There was no API token to remove.");
                }
            }
        },
        Command::Add {
            target,
            yes,
            name,
            category,
            insecure,
        } => add::add(
            &paths,
            &target,
            &add::AddOptions {
                yes,
                name: name.as_deref(),
                category: category.as_deref(),
                insecure,
                json: cli.json,
                quiet: cli.quiet,
            },
        )?,
        Command::Remove { source, yes } => remove::remove(&paths, &source, yes, cli.json)?,
        Command::Search {
            query,
            scope,
            limit,
        } => search(&paths, &query, scope.as_deref(), limit, cli.json)?,
        Command::Serve {
            port,
            bind,
            allow_origin,
        } => {
            // `*` is rejected before the server exists: a wildcard on a
            // localhost service holding user data hands every website read
            // access, and no later check can undo having started that way.
            if allow_origin.iter().any(|o| o.trim() == "*") {
                anyhow::bail!(
                    "`--allow-origin *` is not accepted. Name each origin explicitly, \
                     e.g. --allow-origin chrome-extension://<id>."
                );
            }
            serve::run(
                &paths,
                serve::ServeOptions {
                    port,
                    bind,
                    allowed_origins: allow_origin,
                },
            )?;
        }
        Command::Debug { action } => match action {
            DebugAction::Check => debug::check(&paths, cli.json)?,
            DebugAction::RebuildIndex => debug::rebuild_index(&paths, cli.json, cli.quiet)?,
            DebugAction::Report { lines } => debug::report(&paths, lines)?,
        },
        Command::Mcp { http, .. } => {
            if http {
                // Streamable HTTP is the spec's second transport and is
                // deliberately still not implemented: no MCP client Tome
                // targets needs it (Claude Code spawns the process), and an
                // HTTP MCP endpoint has exactly the browser-reachability
                // problem `tome serve` spends its whole middleware stack on.
                // It lands when a client that cannot spawn processes does.
                anyhow::bail!(
                    "`tome mcp --http` is not implemented yet. Use stdio: a client spawns \
                     `tome mcp` itself — see packaging/claude-plugin/.mcp.json for the shape."
                );
            }
            mcp::serve_stdio(&paths, mcp_tools::all())?;
        }
    }

    Ok(())
}

fn status(paths: &Paths, show_token: bool, json: bool) -> Result<()> {
    // Proves the CLI and the app agree on where the library lives.
    let initialised = paths.state_root().exists();

    if show_token {
        // The one place the token is ever printed. Not part of the default
        // output, not in `--json` without asking, never in logs.
        let api_token = token::load_or_create(paths)?;
        if json {
            println!("{}", serde_json::json!({ "token": api_token }));
        } else {
            println!("{api_token}");
        }
        return Ok(());
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "bundle_id": tome_core::BUNDLE_ID,
                "state": paths.state_root().display().to_string(),
                "cache": paths.cache_root().display().to_string(),
                "db": paths.database_file().display().to_string(),
                "index": paths.index_dir().display().to_string(),
                "initialised": initialised,
            })
        );
        return Ok(());
    }
    println!(
        "Tome {} ({})",
        env!("CARGO_PKG_VERSION"),
        tome_core::BUNDLE_ID
    );
    println!("  state:  {}", paths.state_root().display());
    println!("  cache:  {}", paths.cache_root().display());
    println!("  db:     {}", paths.database_file().display());
    println!("  index:  {}", paths.index_dir().display());
    println!(
        "  status: {}",
        if initialised {
            "initialised"
        } else {
            "not yet initialised"
        }
    );
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

fn pull(
    paths: &Paths,
    source: Option<&str>,
    all: bool,
    only_due: bool,
    max_pages: Option<u32>,
    quiet: bool,
    json: bool,
) -> Result<()> {
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

    // `--due` consults each source's sync strategy (P4-018). The database is
    // where `last_synced` lives; with no database nothing has ever synced, so
    // every scheduled source is due.
    let synced_at = paths
        .database_file()
        .exists()
        .then(|| Database::open(paths))
        .transpose()?
        .map(|db| db.list_sources())
        .transpose()?
        .unwrap_or_default();

    let mut pulled = Vec::new();
    let mut skipped = Vec::new();
    for (id, config_path) in selected {
        let mut config = SourceConfig::parse_file(config_path)
            .with_context(|| format!("reading {}", config_path.display()))?;

        // `--max-pages` overrides the config in memory, never on disk: the
        // file a health check reads must stay byte-identical to the one
        // users get, or the check verifies something nobody runs.
        if let Some(cap) = max_pages {
            config.cap_pages(cap);
        }

        if only_due {
            let last = synced_at
                .iter()
                .find(|s| s.id == *id)
                .and_then(|s| s.last_synced);
            let verdict = tome_core::sync::due(
                &config.sync,
                last,
                tome_core::sync::Trigger::Launch,
                chrono::Utc::now(),
            );
            if !verdict.should_fetch() {
                // Said out loud, not silently skipped: "pull --due did
                // nothing" must be distinguishable from "pull --due is
                // broken".
                skipped.push(serde_json::json!({
                    "source": id.as_str(),
                    "reason": format!("{verdict:?}"),
                }));
                if !quiet && !json {
                    eprintln!("Skipping {id}: {verdict:?}");
                }
                continue;
            }
        }

        if !quiet {
            eprintln!("Pulling {id}…");
        }

        let report = pull_source(paths, &config, quiet)?;
        if json {
            pulled.push(serde_json::json!({
                "source": id.as_str(),
                "pull": report_json(&report),
            }));
        } else {
            report_human(id.as_str(), &report);
        }
    }

    if json {
        // One shape, always, like `list --json`: pulling one source still
        // prints an array of one, and `skipped` is present even when empty.
        println!(
            "{}",
            serde_json::json!({ "pulled": pulled, "skipped": skipped })
        );
    }
    Ok(())
}

/// Run the pipeline for one source, with crawl progress on stderr.
///
/// Progress goes to stderr, deliberately: stdout belongs to `--json` output
/// and to `tome mcp`, and a progress line in a piped JSON stream is a parse
/// error at the other end.
fn pull_source(paths: &Paths, config: &SourceConfig, quiet: bool) -> Result<IngestReport> {
    let mut last_reported = 0usize;
    let report = tome_core::pipeline::pull(paths, config, &mut |progress| {
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
    Ok(report)
}

/// One pull's outcome, for humans. Everything that went wrong, said out
/// loud: a pull that reports success while having silently skipped 200 pages
/// is worse than one that says what it missed.
fn report_human(id: &str, report: &IngestReport) {
    println!(
        "{id}: {} pages in {:.1}s",
        report.pages_stored,
        report.elapsed.as_secs_f64()
    );

    if report.hit_page_cap {
        println!("  stopped at the page cap — the source has more pages than were fetched");
    }
    if report.pages_pruned > 0 {
        // Said out loud. Deleting the user's content silently is the thing
        // the whole pruning guard exists to avoid doing by accident; doing it
        // correctly and without mentioning it is only marginally better.
        println!(
            "  {} pages removed — the site no longer has them",
            report.pages_pruned
        );
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
    if let Some(index) = &report.index {
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

/// One pull's outcome, for scripts. The same facts `report_human` prints —
/// the two must not drift, which is why both read the same report.
fn report_json(report: &IngestReport) -> serde_json::Value {
    serde_json::json!({
        "pages": report.pages_stored,
        "seconds": report.elapsed.as_secs_f64(),
        "hit_page_cap": report.hit_page_cap,
        "pages_pruned": report.pages_pruned,
        "page_errors": report.page_errors,
        "asset_errors": report.asset_errors.len(),
        // `null` when indexing did not run — distinct from an index run
        // that changed nothing.
        "index": report.index.as_ref().map(|index| serde_json::json!({
            "added": index.added,
            "updated": index.updated,
            "removed": index.removed,
            "unchanged": index.unchanged,
            "rebuilt": index.rebuilt,
        })),
    })
}

fn list(paths: &Paths, category: Option<&str>, json: bool) -> Result<()> {
    let mut configs = source_configs(paths)?;

    // The filter reads the config files, not the database: a source that has
    // never been pulled still has a category, and the database's copy is a
    // snapshot from pull time.
    if let Some(category) = category {
        configs.retain(|(_, path)| {
            SourceConfig::parse_file(path)
                .map(|config| config.category == category)
                .unwrap_or(false)
        });
    }
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
                    // `null` until the first successful pull.
                    "last_synced": row
                        .and_then(|s| s.last_synced.as_ref())
                        .map(|t| t.to_rfc3339()),
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "sources": sources }));
        return Ok(());
    }

    if configs.is_empty() {
        match category {
            Some(category) => println!("No sources in category {category:?}."),
            None => println!(
                "No sources yet. Configurations go in {}.",
                paths.sources_dir().display()
            ),
        }
        return Ok(());
    }

    for (id, _) in &configs {
        match pulled.iter().find(|s| s.id == *id) {
            Some(source) => println!(
                "{:<24} {:<8} {:<16} {}  ({})",
                id.as_str(),
                database
                    .as_ref()
                    .and_then(|db| db.page_count(id).ok())
                    .unwrap_or(source.page_count),
                source.category,
                source.name,
                source
                    .last_synced
                    .as_ref()
                    .map(|t| synced_ago(t.timestamp()))
                    .unwrap_or_else(|| "never synced".to_owned()),
            ),
            None => println!("{:<24} {:<8} (never pulled)", id.as_str(), "-"),
        }
    }
    Ok(())
}

/// "synced 2 hours ago", from a unix timestamp. Coarse on purpose — this is
/// orientation, not telemetry — and clock skew clamps to "just now" rather
/// than inventing a negative age.
fn synced_ago(then: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(then);
    let secs = (now - then).max(0);
    match secs {
        0..=59 => "synced just now".to_owned(),
        60..=3599 => format!("synced {} min ago", secs / 60),
        3600..=86_399 => format!("synced {}h ago", secs / 3600),
        _ => format!("synced {}d ago", secs / 86_400),
    }
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

    // What the search silently corrected, if anything (P2-009). Reported
    // rather than kept quiet: a search that answers a different question than
    // the one asked, without saying so, leaves the user believing their
    // library contains something it does not.
    let suggestions = engine.suggest(query)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "results": hits.iter().map(|hit| serde_json::json!({
                    "source": hit.source.as_str(),
                    "path": hit.path,
                    "title": hit.title,
                    "score": hit.score,
                    // `null` for a page that documents no single symbol — a
                    // guide or a tutorial. Always present, like `suggestions`.
                    "symbol_kind": hit.symbol_kind.map(|kind| kind.as_str()),
                })).collect::<Vec<_>>(),
                // Always present, even when empty, so `tome search --json | jq`
                // needs no special case. The same rule `tome list --json`
                // follows.
                "suggestions": suggestions.iter().map(|s| serde_json::json!({
                    "typed": s.typed,
                    "meant": s.meant,
                })).collect::<Vec<_>>(),
            })
        );
        return Ok(());
    }

    if !suggestions.is_empty() {
        let corrections: Vec<String> = suggestions
            .iter()
            .map(|s| format!("{} → {}", s.typed, s.meant))
            .collect();
        println!("Did you mean: {}", corrections.join(", "));
        println!();
    }

    if hits.is_empty() {
        println!("No results for {query:?}.");
        return Ok(());
    }
    for hit in &hits {
        // The kind, when the page documents one symbol, goes on the title line
        // — it is what tells a reader that `Vec` is a type and `read_to_string`
        // a function without opening either (P2-015).
        match hit.symbol_kind {
            Some(kind) => println!(
                "{:<24} {}  [{}]",
                hit.source.as_str(),
                hit.title,
                kind.as_str()
            ),
            None => println!("{:<24} {}", hit.source.as_str(), hit.title),
        }
        println!("{:<24} {}", "", hit.path);
    }
    Ok(())
}
