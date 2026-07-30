//! `tome remove` (P4-006).
//!
//! Removal touches four places — search index, database, cached content, and
//! the config file — and the order is deliberate: **the config file goes
//! last.** Every earlier step can fail and leave a state `tome remove` can be
//! run against again; deleting the config first would orphan the data with no
//! command left that knows the source ever existed.

use std::io::{IsTerminal, Write as _};

use anyhow::{bail, Context, Result};
use tome_core::db::Database;
use tome_core::model::SourceId;
use tome_core::search::SearchEngine;
use tome_core::Paths;

pub(crate) fn remove(paths: &Paths, name: &str, yes: bool, json: bool) -> Result<()> {
    let available = crate::source_configs(paths)?;
    let Some((id, config_path)) = available.iter().find(|(id, _)| id.as_str() == name) else {
        let known: Vec<&str> = available.iter().map(|(id, _)| id.as_str()).collect();
        if known.is_empty() {
            bail!("No source named `{name}` — there are no sources configured.");
        }
        bail!("No source named `{name}`. Known: {}", known.join(", "));
    };

    // What is about to go, said before asking. "This will remove old-lib"
    // is not informed consent if the user thought old-lib was 4 pages and it
    // is 4 000.
    let pages = page_count(paths, id);

    if !yes {
        if json {
            bail!("--json is non-interactive; pass --yes as well.");
        }
        if !std::io::stdin().is_terminal() {
            bail!("stdin is not a terminal; pass --yes to remove without confirmation.");
        }
        match pages {
            Some(pages) => eprintln!(
                "This will remove `{}` — {pages} pages and all cached data.",
                id.as_str()
            ),
            None => eprintln!("This will remove `{}` and all cached data.", id.as_str()),
        }
        eprint!("Continue? [y/N] ");
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        // Destructive, so the default is No — the opposite of `tome add`.
        if !matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            eprintln!("Not removed.");
            return Ok(());
        }
    }

    // 1. Search index. An unreadable index must not block removal — the
    //    index is seconds to rebuild, and the next pull's `open_or_rebuild`
    //    will — but silently skipping it would leave ghost results, so the
    //    skip is said out loud.
    if paths.index_dir().exists() {
        match SearchEngine::open(paths) {
            Ok(engine) => {
                let mut session = engine.session()?;
                session.delete_source(id)?;
                session.commit()?;
            }
            Err(e) => {
                tracing::warn!(
                    "search index could not be opened ({e}); it will be rebuilt on the next pull"
                );
            }
        }
    }

    // 2. Database. Pages go with the source (FK cascade).
    if paths.database_file().exists() {
        Database::open(paths)?.delete_source(id)?;
    }

    // 3. Cached content: pages, raw HTML, assets.
    let data_dir = paths.source_data_dir(id);
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir)
            .with_context(|| format!("removing {}", data_dir.display()))?;
    }

    // 4. The config file, last.
    std::fs::remove_file(config_path)
        .with_context(|| format!("removing {}", config_path.display()))?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "removed": id.as_str(),
                // `null` when there was no database to count from — distinct
                // from 0, which would claim the source was empty.
                "pages": pages,
            })
        );
    } else {
        match pages {
            Some(pages) => println!("Removed {} ({pages} pages).", id.as_str()),
            None => println!("Removed {}.", id.as_str()),
        }
    }
    Ok(())
}

/// How many pages the database holds for this source, if it can say.
fn page_count(paths: &Paths, id: &SourceId) -> Option<u32> {
    if !paths.database_file().exists() {
        return None;
    }
    let db = Database::open(paths).ok()?;
    db.page_count(id).ok()
}
