# Security Considerations

**Privacy Stance:** No telemetry, no data collection
**Architecture:** Local-first, optional iCloud sync

---

## Threat Model

### Assets to Protect

| Asset | Sensitivity | Location |
|-------|-------------|----------|
| User bookmarks & annotations | Medium | Local SQLite, iCloud |
| Reading history | Low | Local SQLite, iCloud |
| Source configurations | Low | Local YAML files |
| Cached documentation | Low | Local filesystem |
| API tokens (if enabled) | High | Local secure storage |

### Threat Actors

| Actor | Motivation | Capability | Notes |
|-------|------------|------------|-------|
| **A web page in the user's browser** | Read the library, drive the local API, use Tome as an SSRF proxy | **Medium-High** | **The most realistic attacker, and the one the original model omitted.** Any site can `fetch()` a localhost service; with permissive CORS it can read the response too. |
| **Malicious / compromised documentation** | XSS in the reader; prompt injection into agents reading via MCP | Medium | Tome ingests untrusted HTML by design and then feeds it to LLMs |
| **Another local process** | Read the token, the database, the cache | High | Non-sandboxed; same-user processes have file access |
| **Local malware** | Data exfiltration | High | Largely out of scope, but do not make it easy |
| **Network attacker** | MITM on documentation fetches | Medium | TLS, no pinning |
| **Curious neighbor** | Physical access | Low | |

**Prompt injection deserves naming.** Tome exists partly to feed documentation to AI agents. A page
containing "ignore previous instructions and…" reaches the agent as ordinary tool output. Tome
cannot solve this, but it must not amplify it: **write-capable MCP tools are disabled by default**,
tool results are truncated, and the reader never executes anything from ingested content.

### Out of Scope

- Nation-state attackers (disproportionate threat model for this app)
- Attacks requiring kernel/root access
- Side-channel attacks

---

## Security Principles

### 1. No Data Collection

```
┌─────────────────────────────────────────────────┐
│                     Tome                        │
│                                                 │
│  ✗ No analytics                                │
│  ✗ No crash reporting to external services     │
│  ✗ No usage telemetry                          │
│  ✗ No phone-home                               │
│  ✗ No feature flags from server                │
│                                                 │
│  ✓ All data stays on user's machine            │
│  ✓ iCloud sync is user-controlled              │
│  ✓ Crash logs stored locally only              │
│                                                 │
└─────────────────────────────────────────────────┘
```

### 2. Minimal Permissions

**Authoritative entitlements file:
[`05-phase-5-polish-launch.md` P5-010](./05-phase-5-polish-launch.md#p5-010-macos-notarization-setup).**

This document previously contained a *second, different* entitlements file — one requesting iCloud
and no hardened-runtime exceptions, while Phase 5's requested `allow-unsigned-executable-memory`
and `disable-library-validation` and no iCloud. Two conflicting security-critical configurations
in one plan set means whichever is copy-pasted first wins. There is now one.

Summary of the posture:

| | Status |
|---|---|
| Hardened runtime | On, with `allow-jit` as the **only** exception |
| App Sandbox | **Off** — incompatible with sharing a library between the app and an unsandboxed CLI; see `09-non-functional-requirements.md` § Local Security |
| Network client | Requested (fetching documentation) |
| Network server | **Not** requested — the local API binds loopback |
| User-selected files | Requested (local documentation directories) |
| iCloud ubiquity container | Requested (bookmark sync) |
| Keychain access group | Requested (API token) |
| Full disk, Apple Events, camera/mic/location | Never requested |

### 3. Defense in Depth

Multiple layers of protection:

```
Layer 1: App Sandbox (macOS)
    └── Layer 2: Network restrictions (HTTPS only)
        └── Layer 3: Content sanitization (doc rendering)
            └── Layer 4: Input validation (user input)
                └── Layer 5: Secure storage (sensitive data)
```

---

## Network Security

### HTTPS Enforcement

Handled by `validate_source_url` below. **The silent `http:` → `https:` upgrade shown in earlier
drafts was removed**: it is not a security control (it protects nothing an attacker cannot avoid),
it breaks hosts that genuinely have no TLS with a confusing error from the *upgraded* URL, and
`set_scheme(...).ok()` discards the failure so a non-upgradable URL would pass through unchanged
while appearing to have been handled. `http://` is rejected unless the source sets
`fetch.allow_insecure: true`.

### Certificate Validation

- Use system certificate store (no custom CA)
- No certificate pinning (documentation sites change certs)
- Reject self-signed certificates
- Reject expired certificates

```rust
// Use reqwest with default TLS settings
let client = reqwest::Client::builder()
    .https_only(true)  // Enforce HTTPS
    .use_rustls_tls()  // Use rustls for TLS
    .build()?;
```

### Local API Security

**Loopback is not a trust boundary.** Every process on the machine can reach `127.0.0.1`, and so
can every web page the user has open. Binding to loopback prevents *remote* access and nothing
else.

```rust
// Necessary, and nowhere near sufficient.
let addr = SocketAddr::from(([127, 0, 0, 1], 7431));
```

**Security controls (all required, not optional):**

| Control | Detail |
|---------|--------|
| Off by default | The server starts only on explicit user action |
| Loopback binding | Any other bind address requires a flag and logs a warning |
| **Bearer token on every request** | Including loopback. No `is_loopback()` bypass — the original middleware had one, which exempted precisely the attacker in the threat model. |
| **No CORS by default** | Explicit origin allowlist only; `*` rejected at config load. Earlier drafts specified `CorsLayer::permissive()` in Phase 4 while *this document* said "no CORS in default mode" — the code would have won. |
| `Host`/`Origin` validation | Blocks DNS rebinding, which defeats bind-address protection |
| Constant-time token comparison | A naive `==` leaks the secret's prefix by timing to any local process |
| Per-token rate limiting | Bounds damage from a leaked token |
| No user content in logs | Search queries and page paths are reading history |

The same rules apply to the optional MCP Streamable HTTP transport — it is the same exposure with a
different payload.

---

## Content Security

### Documentation Rendering

Fetched documentation could contain malicious content:

**Threats:**
- XSS via `<script>` tags
- External resource loading (tracking pixels)
- Malicious links
- CSS injection

**Mitigations:**

> **The original allowlist broke the product.** It permitted no `id` attribute, which silently
> destroys **every heading anchor, the entire TOC sidebar, and every `#fragment` cross-reference** —
> three headline features, disabled by a security control, in a way that would have been
> misdiagnosed as a TOC bug. It also dropped `img` (so documentation loses every diagram), `br`,
> `hr`, `sup`/`sub` (footnotes), `dl`/`dt`/`dd` (API parameter lists), and `alt` text (an
> accessibility regression). SPIKE-011 validates the corrected list against real pages.

```rust
pub fn sanitize_html(raw: &str, base: &Url) -> String {
    let sanitizer = ammonia::Builder::default()
        .tags(hashset![
            // Structure
            "p", "h1", "h2", "h3", "h4", "h5", "h6", "div", "span", "section", "article",
            "br", "hr", "blockquote", "figure", "figcaption", "details", "summary",
            // Lists -- including definition lists, which API docs use heavily
            "ul", "ol", "li", "dl", "dt", "dd",
            // Inline
            "a", "strong", "em", "b", "i", "u", "s", "mark", "small", "sup", "sub", "abbr",
            // Code
            "code", "pre", "kbd", "samp", "var",
            // Tables
            "table", "thead", "tbody", "tfoot", "tr", "td", "th", "caption", "colgroup", "col",
            // Media -- localized to disk at ingest; see P1-023
            "img", "picture", "source",
        ])
        .tag_attributes(hashmap![
            // `id` is REQUIRED on headings and anchors. Without it the TOC,
            // in-page navigation, and every cross-reference stop working.
            "h1" => hashset!["id"], "h2" => hashset!["id"], "h3" => hashset!["id"],
            "h4" => hashset!["id"], "h5" => hashset!["id"], "h6" => hashset!["id"],
            "section" => hashset!["id"], "div" => hashset!["id", "class"],
            "span" => hashset!["id", "class"],
            "a"   => hashset!["href", "id", "title"],
            "img" => hashset!["src", "alt", "width", "height", "loading"],
            "source" => hashset!["srcset", "type"],
            "code" => hashset!["class"],           // language hints
            "pre"  => hashset!["class"],
            "th"   => hashset!["scope", "colspan", "rowspan"],  // table accessibility
            "td"   => hashset!["colspan", "rowspan"],
            "abbr" => hashset!["title"],
            "li"   => hashset!["id", "value"],
        ])
        // `lang` and `dir` anywhere: screen readers and RTL depend on them.
        .generic_attributes(hashset!["lang", "dir"])

        // http is NOT allowed: mixed content, and by ingest time every asset
        // reference has already been rewritten to a local path.
        .url_schemes(hashset!["https", "mailto", "tel"])
        .url_relative(ammonia::UrlRelative::RewriteWithBase(base.clone()))

        // Style attributes are the classic sanitizer bypass and the CSP forbids
        // inline styles anyway. Everything visual comes from our stylesheet.
        .strip_comments(true)
        .build();

    sanitizer.clean(raw).to_string()
}
```

**Sanitize once, at ingest.** Stored pages are already safe, so rendering is a file read. Doing it
per-render would mean the same untrusted input is re-trusted on every page view, and would put a
security control on the latency-critical path.

**Two things the allowlist alone does not cover:**

- **`id` collisions.** Preserving `id` means untrusted content can declare `id="app-root"` or
  duplicate an id the reader relies on. Namespace ingested ids (`doc-<id>`) and rewrite in-page
  `href="#…"` to match — this keeps anchors working *and* prevents collisions with the shell.
- **SVG.** SVG can carry script and external references. SVG assets are sanitized separately with
  an SVG-specific allowlist, or rasterized. Do not pass SVG through an HTML sanitizer and assume
  it is safe.

### WebView Content Security Policy

The reader is a **sandboxed `<iframe>` without `allow-scripts`**, isolating untrusted
documentation from the app UI and the Tauri IPC bridge. The CSP is applied to that frame:

```
default-src 'none';
script-src 'none';
style-src  'self';          /* no 'unsafe-inline': our stylesheet only  */
img-src    'self' data:;    /* NO https: -- see below                   */
font-src   'self';
connect-src 'none';
frame-src  'none';
form-action 'none';
base-uri   'none';
```

Two deliberate tightenings from the original policy:

- **`img-src` no longer allows `https:`.** The original allowed remote images while the same
  document listed "external resource loading (tracking pixels)" as a threat two paragraphs above —
  the policy permitted exactly the thing it named. Every image is localized at ingest (P1-023), so
  `'self' data:` is sufficient, and it makes the offline guarantee enforceable by the browser
  rather than by hope.
- **`style-src` drops `'unsafe-inline'`.** Inline styles are a CSS-injection and
  data-exfiltration vector (`background: url(...)` in a permissive policy), and highlights use
  classes rather than inline `style` attributes precisely so this can be forbidden.

Set the policy as a **response header** on the frame document, not only as a `<meta>` tag —
`meta`-delivered CSP is applied late and cannot express some directives. The `<meta>` tag stays as
defence in depth.

### Link Handling

```typescript
// Intercept all link clicks
document.addEventListener('click', (e) => {
  const link = e.target.closest('a');
  if (!link) return;

  e.preventDefault();

  const href = link.getAttribute('href');

  if (isInternalLink(href)) {
    // Navigate within Tome
    navigateToPage(href);
  } else if (isExternalLink(href)) {
    // Confirm before opening external links
    if (isSafeExternalUrl(href)) {
      openInBrowser(href);
    } else {
      warnSuspiciousLink(href);
    }
  }
});

function isSafeExternalUrl(url: string): boolean {
  const parsed = new URL(url);
  // Only allow https
  if (parsed.protocol !== 'https:') return false;
  // Block known malicious patterns
  if (parsed.hostname.includes('..')) return false;
  return true;
}
```

---

## Data Security

### Local Storage

**SQLite Database:**
```rust
// Database file permissions (Unix)
use std::os::unix::fs::PermissionsExt;
fs::set_permissions(&db_path, fs::Permissions::from_mode(0o600))?;
```

**Configuration Files:**
- YAML configs: Mode 0600 (owner read/write only)
- No sensitive data in config files

### Secure Storage for Tokens

The API token always exists (authentication is mandatory), so this is not conditional:

```rust
// Use macOS Keychain for sensitive data
use security_framework::keychain::SecKeychain;
use security_framework::passwords::*;

pub fn store_api_token(token: &str) -> Result<()> {
    set_generic_password(
        None,  // Default keychain
        BUNDLE_ID,        // DEC-002 -- a single constant, not a literal repeated
        "api-token",      //           across the codebase
        token.as_bytes(),
    )?;
    Ok(())
}

pub fn get_api_token() -> Result<String> {
    let (password, _) = get_generic_password(
        None,
        BUNDLE_ID,
        "api-token",
    )?;
    String::from_utf8(password).map_err(Into::into)
}
```

### iCloud Data

Data synced to iCloud:
- Encrypted in transit (TLS)
- Encrypted at rest (Apple's encryption)
- Accessible only to user's Apple ID

**We do NOT sync:**
- Documentation content (too large, easily re-fetched)
- Search indexes
- Cached rendered pages

---

## Input Validation

### URL Validation

> **The original filter did not work.** It matched on the hostname *string*, so it missed
> `172.16.0.0/12`, `169.254.0.0/16` (cloud metadata at `169.254.169.254`), all IPv6 (`[::1]`,
> `fc00::/7`), `0.0.0.0`, and alternate encodings (`0x7f.1`, `2130706433`, `127.1`). It did nothing
> about **DNS rebinding** — an attacker-controlled name that passes the string check and then
> resolves to `127.0.0.1`. And its `file://` check was **unreachable**: the scheme check above it
> already returned `Err` for anything that is not http/https.
>
> This matters more than it looks. `POST /api/v1/sources` lets a caller name a URL that Tome then
> fetches and stores — so a weak filter turns Tome into an SSRF proxy into the user's private
> network, with the response readable back through `GET /pages`.

```rust
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub fn validate_source_url(input: &str, allow_insecure: bool) -> Result<ValidatedUrl, ValidationError> {
    let url = Url::parse(input)?;

    match url.scheme() {
        "https" => {}
        "http" if allow_insecure => {}      // explicit per-source opt-in only
        "http" => return Err(ValidationError::InsecureScheme),
        other  => return Err(ValidationError::UnsupportedScheme(other.into())),
    }

    let host = url.host().ok_or(ValidationError::NoHost)?;

    // Resolve, then judge EVERY address the name maps to. Checking the string
    // cannot work: names are attacker-controlled and may resolve anywhere.
    let addrs: Vec<IpAddr> = resolve(&host)?;
    if addrs.is_empty() {
        return Err(ValidationError::Unresolvable);
    }
    for addr in &addrs {
        if is_forbidden(addr) {
            return Err(ValidationError::PrivateAddressBlocked(*addr));
        }
    }

    Ok(ValidatedUrl { url, resolved: addrs })
}

fn is_forbidden(addr: &IpAddr) -> bool {
    match addr {
        IpAddr::V4(v4) => {
            v4.is_loopback()                      // 127/8
                || v4.is_private()                // 10/8, 172.16/12, 192.168/16
                || v4.is_link_local()             // 169.254/16 -- cloud metadata
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()            // 0.0.0.0
                || v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]) // CGNAT 100.64/10
                || v4.is_multicast()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()                      // ::1
                || v6.is_unspecified()
                || v6.is_multicast()
                || (v6.segments()[0] & 0xfe00) == 0xfc00   // fc00::/7 unique local
                || (v6.segments()[0] & 0xffc0) == 0xfe80   // fe80::/10 link local
                // IPv4-mapped (::ffff:127.0.0.1) must be unwrapped and re-checked,
                // or every IPv4 rule above is trivially bypassed.
                || v6.to_ipv4_mapped().map_or(false, |v4| is_forbidden(&IpAddr::V4(v4)))
        }
    }
}
```

**Two things the function alone cannot do, and both are required:**

1. **Pin the connection to the address that was validated.** Otherwise the name is resolved twice —
   once to check, once to connect — and an attacker controlling DNS wins the race (classic
   TOCTOU rebinding). Use a custom resolver that returns only the validated addresses.
2. **Re-validate on every redirect.** A permitted host can `302` to `http://169.254.169.254/`.
   Follow redirects manually, cap them (default 5), and run the full check on each hop.

### Search Query Validation

```rust
pub fn sanitize_search_query(query: &str) -> String {
    // `&query[..query.len().min(1000)]` PANICS: byte slicing a UTF-8 string at an
    // arbitrary index fails unless it lands on a char boundary. Any query with a
    // multi-byte character near the limit crashes the process -- and search input
    // comes straight from the user, the HTTP API, and MCP clients.
    query
        .chars()
        .filter(|c| !c.is_control())
        .take(1000)          // 1000 CHARACTERS, boundary-safe by construction
        .collect()
}
```

Byte-vs-character slicing is a recurring hazard in this codebase — snippet generation, highlight
offsets, and prefix/suffix capture all touch arbitrary user text. Prefer `chars()`/`char_indices()`
and add a fuzz target over non-ASCII input.

### Path Traversal Prevention

> **The original had three bugs, and together they made it both broken and unsafe.**
> `PathBuf::from("~/.tome/…")` does **not** expand `~` — it produces a literal directory named
> `~`. The base was never canonicalized while the candidate was, so `canonical.starts_with(&base)`
> compared an absolute path against `~/...` and could never be true — every request would be
> rejected. And `canonicalize()` fails outright for a path that does not exist yet, turning a
> "not found" into an error that reads like a security violation.

```rust
pub fn validate_page_path(paths: &Paths, source_id: &SourceId, rel: &str)
    -> Result<PathBuf, SecurityError>
{
    // Reject the obvious before touching the filesystem.
    if rel.contains('\0') || Path::new(rel).is_absolute() {
        return Err(SecurityError::PathTraversal);
    }

    // Base comes from the path module (P1-006) and is already absolute and canonical.
    let base = paths.pages_dir(source_id);          // .../Caches/Tome/data/<id>/pages
    let base = base.canonicalize()?;                // canonicalize BOTH sides

    // Reject traversal components explicitly rather than relying on canonicalize,
    // which cannot help when the target does not exist.
    let mut candidate = base.clone();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(c) => candidate.push(c),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(SecurityError::PathTraversal);
            }
        }
    }

    // If it exists, canonicalize to defeat symlinks that escape the base.
    // If it does not, the component walk above already guarantees containment.
    if let Ok(real) = candidate.canonicalize() {
        if !real.starts_with(&base) {
            return Err(SecurityError::PathTraversal);   // symlink escape
        }
        return Ok(real);
    }
    Ok(candidate)
}
```

Note the symlink case is the reason canonicalization is still needed: a crawled site can be stored
under a path that a *user-created* symlink redirects outside the cache.

---

## Dependency Security

### Auditing

**These are now actually wired into CI** (see `10-cicd-devops.md` → the `audit` job, which is a
required status check). They were listed here and in RISK-008 as active mitigations while the
workflows contained none of them — a risk that reads as handled but is not.

```yaml
- uses: rustsec/audit-check@v2            # cargo audit
- uses: EmbarkStudios/cargo-deny-action@v2 # licences, bans, duplicate deps
- run: npm audit --audit-level=high
- uses: gitleaks/gitleaks-action@v2        # secret scanning
```

### Dependency Policy

| Rule | Enforcement |
|------|-------------|
| No known high/critical CVEs | CI blocks merge |
| Prefer well-maintained deps | Manual review |
| Pin major versions | Cargo.toml, package.json |
| Review new dependencies | PR checklist |

### Supply Chain

- Use `cargo-crev` for Rust dependency review (optional)
- Verify npm package signatures
- Lock file committed (`Cargo.lock`, `package-lock.json`)

---

## Secure Development Practices

### Code Review Checklist

Security items for PR review:

- [ ] No hardcoded secrets or credentials
- [ ] User input validated before use
- [ ] URLs validated before fetching
- [ ] File paths validated (no traversal)
- [ ] Error messages don't leak sensitive info
- [ ] New dependencies reviewed for security
- [ ] No `unsafe` Rust without justification
- [ ] HTML output sanitized

### Secrets Management

**Never commit:**
- API keys
- Certificates
- Private keys
- Passwords

**Use:**
- GitHub Secrets for CI
- Environment variables for local dev
- macOS Keychain for runtime secrets

---

## Incident Response

### If a Security Issue is Found

1. **Assess severity** (Critical/High/Medium/Low)
2. **Document** the issue privately
3. **Fix** in private branch
4. **Test** fix thoroughly
5. **Release** patch version
6. **Disclose** responsibly (if applicable)

### Security Contact

- GitHub Security Advisories for private reporting (**enable "Private vulnerability reporting" in
  repository settings — it is off by default, so the stated process does not work until it is on**)
- A `SECURITY.md` at the repository root naming the reporting path and expected response window
- No public issue tracker for security issues
- Credit reporters in release notes unless they decline

---

## Compliance

### Apple Requirements

| Requirement | Status |
|-------------|--------|
| App Sandbox | **Not enabled** — only required for Mac App Store; incompatible with the shared CLI/app library. See `09-non-functional-requirements.md` § Local Security. |
| Hardened Runtime | Configured, but **inert without a signing identity**. Kept so that enabling notarization later is a credentials change, not a config change. |
| Notarization | **Deferred** — [ADR-0006](../decisions/0006-unsigned-distribution.md). Ships unsigned via an own Homebrew tap. |
| Privacy manifest | Required for v1.0 |

> **What shipping unsigned costs, security-wise.** Gatekeeper and notarization are Apple's malware
> scan and tamper-evidence layer; without them, a user has no cryptographic assurance that the
> `.app` they downloaded is the one that was built. The compensating controls are weaker but real:
> the tap pins a `verified:` GitHub URL, releases are built from tagged commits in CI, and the
> repository is public at release so the build is reproducible from source. Users who want a
> stronger guarantee can build from source. This is a deliberate, reversible trade — see the ADR.

### Privacy Manifest

```plist
<!-- PrivacyInfo.xcprivacy -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "...">
<plist version="1.0">
<dict>
    <key>NSPrivacyTracking</key>
    <false/>
    <key>NSPrivacyTrackingDomains</key>
    <array/>
    <key>NSPrivacyCollectedDataTypes</key>
    <array/>
    <key>NSPrivacyAccessedAPITypes</key>
    <array>
        <dict>
            <key>NSPrivacyAccessedAPIType</key>
            <string>NSPrivacyAccessedAPICategoryFileTimestamp</string>
            <key>NSPrivacyAccessedAPITypeReasons</key>
            <array>
                <string>C617.1</string>
            </array>
        </dict>
        <!-- Preferences are read/written throughout; the original manifest
             omitted this category while the plan uses UserDefaults in several
             places. An incomplete manifest is a submission/notarization risk. -->
        <dict>
            <key>NSPrivacyAccessedAPIType</key>
            <string>NSPrivacyAccessedAPICategoryUserDefaults</string>
            <key>NSPrivacyAccessedAPITypeReasons</key>
            <array>
                <string>CA92.1</string>
            </array>
        </dict>
        <!-- Disk space is checked before large crawls and before indexing. -->
        <dict>
            <key>NSPrivacyAccessedAPIType</key>
            <string>NSPrivacyAccessedAPICategoryDiskSpace</string>
            <key>NSPrivacyAccessedAPITypeReasons</key>
            <array>
                <string>E174.1</string>
            </array>
        </dict>
    </array>
</dict>
</plist>
```

---

## Security Testing

### Automated

- `cargo audit` in CI
- `npm audit` in CI
- Clippy security lints

### Manual (Pre-Release)

- [ ] XSS corpus through the sanitizer: zero payloads survive
- [ ] **Anchor corpus through the sanitizer: zero anchors broken** (the security control must not
      break the feature — SPIKE-011)
- [ ] SSRF vector list rejected: `localhost`, `127.0.0.1`, `[::1]`, `::ffff:127.0.0.1`,
      `169.254.169.254`, `10.x`, `172.20.x`, `192.168.x`, `0.0.0.0`, `2130706433`, `0x7f.1`,
      a name that resolves to a private IP, and a redirect chain ending at one
- [ ] Path traversal attempts rejected, including via symlink
- [ ] **A cross-origin `fetch()` from a real browser page cannot read any API response**
- [ ] **An API request with no token is rejected from loopback**
- [ ] Token absent from logs, diagnostics bundles, and crash reports
- [ ] Reader frame: `script` in ingested content does not execute; no remote requests on page view
- [ ] File permissions: `0600` on database and configs, `0700` on directories
- [ ] No network calls on launch beyond what the user configured

---

## Future Considerations

### Post-v1.0

- Security audit by third party (if resources allow)
- Bug bounty program (if user base grows)
- SOC 2 compliance (if enterprise users)
