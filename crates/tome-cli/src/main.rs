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

use anyhow::Result;
use clap::{Parser, Subcommand};
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
        other => {
            let name = match other {
                Command::Add { .. } => "add",
                Command::Pull { .. } => "pull",
                Command::Search { .. } => "search",
                Command::List => "list",
                Command::Remove { .. } => "remove",
                Command::Serve { .. } => "serve",
                Command::Mcp { .. } => "mcp",
                Command::Status => unreachable!("handled above"),
            };
            anyhow::bail!("`tome {name}` is not implemented yet (scaffold only).");
        }
    }

    Ok(())
}
