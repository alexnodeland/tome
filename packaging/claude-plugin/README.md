# Tome — Claude Code plugin

Gives Claude Code your offline documentation library: search it, open pages,
look up symbols, without a network round trip and without a docs site's
JavaScript.

## Install

Tome's CLI must be on `PATH` first — the plugin is a thin wrapper over it.
`brew install --cask alexnodeland/tap/tome` puts `tome` there (the cask
symlinks `Tome.app/Contents/MacOS/tome`), or build from source and put the
binary on `PATH` yourself.

Then, from this repository:

```
/plugin marketplace add alexnodeland/tome
/plugin install tome
```

Or point Claude Code at a checkout directly:

```
claude plugin validate packaging/claude-plugin   # optional, checks the manifest
```

Verify with `/tome:list`. If the MCP tools do not appear, `tome` is not on
Claude Code's `PATH` — check with `which tome` in the same shell you launch
Claude Code from.

## What it adds

**MCP tools** (the ones Claude reaches for on its own):

| Tool | What it does |
|---|---|
| `tome_search` | Search every source, or one via `scope`. `@name` searches declared symbols only |
| `tome_get_page` | Read a page as markdown. `section` reads one heading's subtree |
| `tome_list_sources` | What is in the library |
| `tome_get_toc` | One source's pages, in navigation order |
| `tome_lookup_symbol` | Pages that *declare* a symbol, not every page mentioning it |

**Slash commands** (the ones you invoke):

| Command | What it does |
|---|---|
| `/tome:add <url>` | Detect the platform, write a config, pull |
| `/tome:search <query>` | Search, then read the best result and answer from it |
| `/tome:list` | What is in the library |
| `/tome:pull <id> \| --all` | Fetch or update content |
| `/tome:remove <id>` | Remove a source, after confirming |

## Example workflows

**Answer from real documentation instead of memory.** Ask a question about a
library you have pulled; Claude searches, opens the page, and cites
`source_id: page_path`. The citation is the point — you can open the same page
in Tome.

**Look up an API without leaving the terminal.**

```
> what does Vec::with_capacity actually allocate?
```

Claude calls `tome_lookup_symbol` with `with_capacity`, gets the page that
*declares* it rather than the 321 pages that mention `Vec`, and reads it.

**Add docs for something you are about to use.**

```
/tome:add https://docs.pola.rs/
```

Detection picks the scraper, the initial pull runs, and the source is
searchable when it finishes.

## Error handling

Tool errors come back as text that names the remedy, and they are worth
relaying rather than working around:

| What you see | What it means |
|---|---|
| "nothing has been pulled yet" | The library is empty. `/tome:add` a source |
| "no page … in source" | The path is wrong; `tome_search` returns paths that exist |
| "no section … on this page" | The error lists the page's real anchors |
| "No page declares `x`" | `tome_search` finds pages that merely mention it |
| `[truncated: showing N of M KiB]` | The page is long; call again with `section` — the notice lists them |

Nothing here mutates the library except `/tome:add`, `/tome:pull` and
`/tome:remove`, all of which are explicit slash commands. **The MCP tools are
read-only by design**: the documentation Tome ingests is untrusted text that
agents read, so a prompt injection in a scraped page must not be able to reach
a write.

## Requirements

macOS, and `tome` on `PATH`. The MCP server is stdio — Claude Code spawns
`tome mcp` per session; there is no port, no socket and nothing to leave
running.
