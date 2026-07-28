# Security Policy

## Status

Tome is pre-implementation — there is no released software to attack yet. This policy exists so
that the reporting path is in place before it is needed, and so contributors know how the project
intends to behave.

## Reporting a vulnerability

**Please do not open a public issue.**

Use **GitHub Security Advisories** → the *Security* tab → *Report a vulnerability*. This creates a
private thread visible only to maintainers.

If that is unavailable, open a public issue containing only "requesting a private channel for a
security report" and no details.

### What to expect

This is a small open-source project with no paid support and no service-level agreement. As a
statement of intent, not a commitment:

| | Target |
|---|---|
| Acknowledgement | within a few days |
| Initial assessment | within a week |
| Fix for a confirmed critical issue | prioritized over all other work |
| Public disclosure | after a fix ships, coordinated with you |

Reporters are credited in release notes unless they prefer otherwise.

## Scope

**In scope** once code exists:

- Bypassing authentication on the local HTTP API or the MCP HTTP transport
- Anything letting a web page or another local process read library data or drive the API
- Server-side request forgery via source configuration (reaching loopback, link-local, or private
  addresses)
- Sanitizer bypass allowing script execution in the reader
- Path traversal out of the data directory
- Secrets (the API bearer token) appearing in logs, diagnostics bundles, or crash reports
- Sync operations that lose or corrupt user data
- Supply-chain issues in the release pipeline

**Out of scope:**

- Attacks requiring root or physical access
- Anything requiring the user to disable a documented safety control (for example setting
  `fetch.respect_robots: false` or binding the API off-loopback)
- Denial of service against the user's own machine by their own configuration
- Vulnerabilities in third-party documentation sites
- Missing hardening with no demonstrated impact

## Security posture

Documented in [`docs/plans/12-security-considerations.md`](docs/plans/12-security-considerations.md).
The commitments that most shape the design:

- **Loopback is not a trust boundary.** The local API requires a bearer token on every request,
  including from `127.0.0.1`, and emits no CORS headers by default. Every process on a machine —
  and every web page in the user's browser — can reach a localhost port.
- **Ingested documentation is untrusted input**, sanitized to an allowlist at ingest and rendered in
  a script-disabled frame under a strict content-security policy.
- **Every fetched URL is validated after DNS resolution and re-validated on each redirect**, so that
  `POST /sources` cannot be used to reach the user's private network.
- **No telemetry**, so a vulnerability cannot exfiltrate through an analytics channel that does not
  exist.
- **Agent-facing MCP tools that mutate state are disabled by default**, because documentation Tome
  ingests is untrusted text that agents will read.
