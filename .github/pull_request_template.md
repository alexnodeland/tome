## What and why

<!-- What changes, and what problem it solves. Link the issue or ticket ID (e.g. P2-004). -->

## Checklist

- [ ] Tests added or updated for the behaviour that changed
- [ ] `cargo fmt`, `cargo clippy -- -D warnings`, `npm run lint` all clean
- [ ] **Specification updated in this PR** if this adds or changes an HTTP route, MCP tool, CLI
      command, config key, or keyboard shortcut
- [ ] No new copy of a table that another document owns — linked instead
      (see the ownership table in `docs/plans/00-project-overview.md`)
- [ ] `CHANGELOG.md` updated if user-visible

## If this touches a security surface

- [ ] No `unwrap()` on network input, file contents, or another device's data
- [ ] User content (queries, page paths, notes) is not written to logs or diagnostics
- [ ] URL handling passes the SSRF test vectors
- [ ] Sanitizer changes verified against both the XSS corpus **and** the anchor corpus
      (a sanitizer change that breaks heading `id`s silently breaks the TOC)

## Notes for the reviewer

<!-- Anything you are unsure about, or deliberately left out. -->
