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

| Actor | Motivation | Capability |
|-------|------------|------------|
| **Malicious website** | XSS via doc content | Low-Medium |
| **Local malware** | Data exfiltration | High |
| **Network attacker** | MITM, data interception | Medium |
| **Curious neighbor** | Physical access | Low |

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

Request only necessary entitlements:

```xml
<!-- entitlements.plist -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "...">
<plist version="1.0">
<dict>
    <!-- Network access for fetching docs -->
    <key>com.apple.security.network.client</key>
    <true/>

    <!-- File access for doc storage -->
    <key>com.apple.security.files.user-selected.read-write</key>
    <true/>

    <!-- iCloud for sync -->
    <key>com.apple.developer.icloud-container-identifiers</key>
    <array>
        <string>iCloud.com.example.tome</string>
    </array>

    <!-- NOT requested: -->
    <!-- com.apple.security.network.server - we don't need incoming -->
    <!-- com.apple.security.files.all - we don't need full disk -->
    <!-- com.apple.security.automation.apple-events - no automation -->
</dict>
</plist>
```

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

```rust
pub fn validate_url(url: &str) -> Result<Url, SecurityError> {
    let parsed = Url::parse(url)?;

    match parsed.scheme() {
        "https" => Ok(parsed),
        "http" => {
            // Auto-upgrade to HTTPS
            let mut upgraded = parsed.clone();
            upgraded.set_scheme("https").ok();
            Ok(upgraded)
        }
        _ => Err(SecurityError::UnsupportedScheme(parsed.scheme().to_string()))
    }
}
```

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

The HTTP API binds to localhost only:

```rust
// API server binds to loopback only
let addr = SocketAddr::from(([127, 0, 0, 1], 7431));
axum::Server::bind(&addr)
    .serve(app.into_make_service())
    .await?;
```

**Security controls:**
- Localhost-only binding (no network exposure)
- Optional token auth for paranoid users
- No CORS in default mode (same-origin)
- Rate limiting to prevent local DoS

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

```rust
pub fn sanitize_html(raw: &str) -> String {
    let sanitizer = ammonia::Builder::default()
        // Allow safe tags
        .tags(hashset!["p", "h1", "h2", "h3", "h4", "h5", "h6",
                       "ul", "ol", "li", "a", "code", "pre",
                       "table", "tr", "td", "th", "thead", "tbody",
                       "strong", "em", "blockquote", "div", "span"])
        // Allow safe attributes
        .tag_attributes(hashmap![
            "a" => hashset!["href"],
            "code" => hashset!["class"],  // for language hints
            "pre" => hashset!["class"],
            "div" => hashset!["class"],
            "span" => hashset!["class"],
        ])
        // Sanitize URLs
        .url_schemes(hashset!["https", "http"])
        // Remove all event handlers
        .strip_comments(true)
        .build();

    sanitizer.clean(raw).to_string()
}
```

### WebView Content Security Policy

```javascript
// Inject CSP meta tag into rendered content
const csp = `
  default-src 'self';
  script-src 'none';
  style-src 'self' 'unsafe-inline';
  img-src 'self' data: https:;
  font-src 'self';
  connect-src 'none';
  frame-src 'none';
`;

document.head.insertAdjacentHTML('beforeend',
  `<meta http-equiv="Content-Security-Policy" content="${csp}">`
);
```

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

If API authentication is enabled, store tokens securely:

```rust
// Use macOS Keychain for sensitive data
use security_framework::keychain::SecKeychain;
use security_framework::passwords::*;

pub fn store_api_token(token: &str) -> Result<()> {
    set_generic_password(
        None,  // Default keychain
        "com.example.tome",
        "api-token",
        token.as_bytes(),
    )?;
    Ok(())
}

pub fn get_api_token() -> Result<String> {
    let (password, _) = get_generic_password(
        None,
        "com.example.tome",
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

```rust
pub fn validate_source_url(input: &str) -> Result<ValidatedUrl, ValidationError> {
    // Parse URL
    let url = Url::parse(input)?;

    // Check scheme
    if url.scheme() != "https" && url.scheme() != "http" {
        return Err(ValidationError::InvalidScheme);
    }

    // Check for suspicious patterns
    if url.host_str().is_none() {
        return Err(ValidationError::NoHost);
    }

    // Block local addresses
    if let Some(host) = url.host_str() {
        if host == "localhost"
            || host == "127.0.0.1"
            || host.starts_with("192.168.")
            || host.starts_with("10.")
        {
            return Err(ValidationError::LocalAddressBlocked);
        }
    }

    // Block file:// URLs
    if url.scheme() == "file" {
        return Err(ValidationError::FileUrlBlocked);
    }

    Ok(ValidatedUrl(url))
}
```

### Search Query Validation

```rust
pub fn sanitize_search_query(query: &str) -> String {
    // Limit length
    let truncated = &query[..query.len().min(1000)];

    // Remove control characters
    truncated
        .chars()
        .filter(|c| !c.is_control())
        .collect()
}
```

### Path Traversal Prevention

```rust
pub fn validate_page_path(source_id: &str, path: &str) -> Result<PathBuf, SecurityError> {
    let base = PathBuf::from(&format!("~/.tome/data/{}/pages", source_id));
    let requested = base.join(path);

    // Canonicalize to resolve .. and symlinks
    let canonical = requested.canonicalize()?;

    // Ensure result is still under base
    if !canonical.starts_with(&base) {
        return Err(SecurityError::PathTraversal);
    }

    Ok(canonical)
}
```

---

## Dependency Security

### Auditing

```yaml
# CI workflow
- name: Cargo Audit
  run: |
    cargo install cargo-audit
    cargo audit --deny warnings

- name: NPM Audit
  run: npm audit --audit-level=high
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

- GitHub Security Advisories for private reporting
- No public issue tracker for security issues

---

## Compliance

### Apple Requirements

| Requirement | Status |
|-------------|--------|
| App Sandbox | Required, enabled |
| Hardened Runtime | Required, enabled |
| Notarization | Required, automated |
| Privacy manifest | Required for v1.0 |

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

- [ ] Test with malicious HTML content
- [ ] Test URL validation edge cases
- [ ] Test path traversal attempts
- [ ] Verify localhost-only API binding
- [ ] Check file permissions on data
- [ ] Verify no network calls without user action

---

## Future Considerations

### Post-v1.0

- Security audit by third party (if resources allow)
- Bug bounty program (if user base grows)
- SOC 2 compliance (if enterprise users)
