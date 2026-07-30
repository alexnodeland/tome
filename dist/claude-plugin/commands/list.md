---
description: List the documentation sources in the Tome library
allowed-tools: mcp__plugin_tome_tome__tome_list_sources
---

List what is in the user's Tome library using the `tome_list_sources` MCP tool.

Report each source's id, name, page count and category as a short table. The
**id** is the part that matters operationally — it is what `--scope` and
`tome pull` take.

If a source is listed as configured but never pulled, say so and offer to run
`/tome:pull <id>`: it has no content and will not appear in search.
