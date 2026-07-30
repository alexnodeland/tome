---
description: Search the Tome documentation library
argument-hint: <query> [--scope <source-id>]
allowed-tools: mcp__plugin_tome_tome__tome_search, mcp__plugin_tome_tome__tome_get_page
---

Search the user's Tome library for: **$ARGUMENTS**

Use the `tome_search` MCP tool, not the CLI — it returns the same ranking and
saves a process spawn.

Then **read the best result** with `tome_get_page` rather than answering from
titles alone: a title match is not an answer, and the whole point of a local
library is that opening the page is free.

Answer the user's question from the page, and cite it as
`source_id: page_path` so they can open it themselves.

If the search reports a correction (`(searched "x" for "y")`), say so — the
answer is to a slightly different question than the one asked.
