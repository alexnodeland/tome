---
description: Add a documentation source to the Tome library and pull it
argument-hint: <url> [--name <id>] [--category <name>]
allowed-tools: Bash(tome add:*), Bash(tome list:*)
---

Add the documentation site the user named to their Tome library.

Run:

```
tome add $ARGUMENTS --yes --json
```

`--yes` is required: this session is not a terminal, so the interactive
confirmation cannot be answered and the command would refuse without it.

The JSON reports what platform was detected (`detected.platform`,
`detected.confident`), where the config was written, and what the initial pull
fetched. Tell the user:

- the source id (they will use it as `scope` when searching),
- how many pages were pulled,
- and, **only if `detected.confident` is false**, that detection was unsure and
  the generic scraper was used — which works, but may keep page furniture a
  platform profile would strip.

If the command fails, the error message says what to do; relay it rather than
retrying with different flags. Two common ones: a URL already configured under
another id (nothing to do — it is already there), and an `http://` URL, which
needs `--insecure` and only for a host the user owns.
