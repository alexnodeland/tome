---
description: Fetch or update documentation content in the Tome library
argument-hint: <source-id> | --all
allowed-tools: Bash(tome pull:*), Bash(tome list:*)
---

Update the user's Tome library.

Run:

```
tome pull $ARGUMENTS --json
```

This is `pull`, not `sync` — `sync` means *bookmark* sync in Tome, which is
automatic and not user-invoked.

A pull can take minutes: it is a polite rate-limited crawl of a real
documentation site, and the page count is the site's, not ours. Do not
interrupt it or retry it in parallel.

Report from the JSON: pages stored, and the index counts (`added`, `updated`,
`removed`, `unchanged`) — a re-pull of unchanged content reporting all
`unchanged` is the correct, healthy outcome, not a failure.

Two things in the report deserve to be surfaced rather than skipped past,
because they mean the library is incomplete:

- `hit_page_cap: true` — the site has more pages than were fetched. Raising
  `max_pages` in the source's config file and pulling again is the fix.
- a non-empty `page_errors` — those pages are missing from the library and
  from search.
