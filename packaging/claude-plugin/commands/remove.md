---
description: Remove a documentation source from the Tome library
argument-hint: <source-id>
allowed-tools: Bash(tome remove:*), Bash(tome list:*)
---

Remove the source the user named from their Tome library.

**Confirm with the user first, in your own message, before running anything.**
This deletes the source's cached content, database rows, search index entries
and config file. The content is not recoverable without re-crawling the site,
which is minutes of polite fetching.

Once they have confirmed:

```
tome remove <source-id> --yes --json
```

`--yes` is required here because this session is not a terminal — the
interactive confirmation cannot be answered. That makes *your* confirmation the
only one there is, which is why it comes first.

If the id is unknown the error names the ids that exist; use `/tome:list` to
show the user rather than guessing at a correction.
