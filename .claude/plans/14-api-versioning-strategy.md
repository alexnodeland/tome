# API Versioning Strategy

This document covers versioning for Tome's external interfaces: Local HTTP API, MCP Server, and CLI.

---

## Versioning Philosophy

1. **Stability over features** - Don't break existing integrations
2. **Deprecation with notice** - Minimum 2 minor versions before removal
3. **Semantic versioning** - Breaking changes only in major versions
4. **Graceful degradation** - Old clients should work with helpful errors

---

## Semantic Versioning

Tome follows [SemVer 2.0](https://semver.org/):

```
MAJOR.MINOR.PATCH

Example: 1.2.3
         │ │ └── Patch: Bug fixes, no API changes
         │ └──── Minor: New features, backwards compatible
         └────── Major: Breaking changes
```

### Version Progression

```
v1.0.0 → v1.0.1 (bug fix)
       → v1.1.0 (new endpoint)
       → v1.2.0 (new optional field)
       → v2.0.0 (breaking change)
```

---

## HTTP API Versioning

### URL-Based Versioning

```
Base URL: http://localhost:7431/api/v1/
```

**Rationale:** URL versioning is explicit, easy to understand, and works with any HTTP client.

### Version in URL

```
GET /api/v1/search?q=iterator
GET /api/v1/sources
GET /api/v1/sources/{id}/pages
```

### When to Bump API Version

| Change Type | API Version | Example |
|-------------|-------------|---------|
| Bug fix | Patch (1.0.0 → 1.0.1) | Fix search returning duplicates |
| New endpoint | Minor (1.0 → 1.1) | Add `/api/v1/collections` |
| New optional field | Minor | Add `created_at` to response |
| New required field | Major | Require `source_type` in request |
| Remove field | Major | Remove `legacy_id` from response |
| Change field type | Major | Change `count` from string to int |
| Remove endpoint | Major | Remove `/api/v1/deprecated` |

### Multiple API Versions

Support N-1 versions concurrently:

```rust
// Router setup
let app = Router::new()
    .nest("/api/v1", v1_routes())
    .nest("/api/v2", v2_routes())  // When v2 is released
    .layer(/* ... */);
```

### Deprecation Headers

When an endpoint is deprecated:

```http
HTTP/1.1 200 OK
Deprecation: true
Sunset: Sat, 01 Jun 2027 00:00:00 GMT
Link: </api/v2/search>; rel="successor-version"

{
  "results": [...]
}
```

### Version Negotiation

Default to latest stable:

```rust
// If no version specified, redirect to v1
pub async fn api_root() -> Redirect {
    Redirect::permanent("/api/v1/")
}
```

### API Version Lifecycle

| State | Duration | Support Level |
|-------|----------|---------------|
| **Current** | Indefinite | Full support |
| **Deprecated** | 6 months minimum | Bug fixes only |
| **Sunset** | After deprecation | No support, returns 410 Gone |

---

## MCP Protocol Versioning

### Protocol Version Negotiation

MCP uses version negotiation during initialization:

```json
// Client → Server
{
  "jsonrpc": "2.0",
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": { ... }
  },
  "id": 1
}

// Server → Client
{
  "jsonrpc": "2.0",
  "result": {
    "protocolVersion": "2024-11-05",
    "capabilities": { ... },
    "serverInfo": {
      "name": "tome",
      "version": "1.0.0"
    }
  },
  "id": 1
}
```

### Supported Protocol Versions

```rust
const SUPPORTED_MCP_VERSIONS: &[&str] = &[
    "2024-11-05",  // Current
    "2024-09-01",  // Previous
];

pub fn negotiate_version(client_version: &str) -> Option<&'static str> {
    SUPPORTED_MCP_VERSIONS
        .iter()
        .find(|&&v| v == client_version)
        .copied()
}
```

### Tool Versioning

Individual tools can have versions:

```json
{
  "tools": [
    {
      "name": "tome_search",
      "description": "Search documentation",
      "version": "1.0.0",
      "inputSchema": { ... }
    }
  ]
}
```

### MCP Breaking Changes

| Change | Handling |
|--------|----------|
| New tool | Add to tools list, no version bump |
| New parameter (optional) | Add with default, no version bump |
| New parameter (required) | New tool version or new tool name |
| Remove tool | Deprecate first, remove in next major |
| Change parameter type | New tool version |

---

## CLI Versioning

### Command Compatibility

CLI follows the same SemVer as the app:

```bash
tome --version
# tome 1.0.0
```

### Output Format Stability

**JSON output (`--json`) is part of the API contract:**

```bash
# This output format is stable within a major version
tome list --json
```

```json
{
  "sources": [
    {
      "id": "abc123",
      "name": "Rust std",
      "version": "1.0.0",
      "page_count": 4521
    }
  ]
}
```

### CLI Deprecation

Deprecated commands show warnings:

```bash
$ tome old-command
Warning: 'old-command' is deprecated and will be removed in v2.0.
Use 'new-command' instead.

# Still executes for backwards compatibility
```

### Exit Codes

Stable across versions:

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | Resource not found |
| 4 | Network error |
| 5 | Configuration error |

---

## Configuration File Versioning

### Schema Version

```yaml
# ~/.tome/sources/example.yaml
schema_version: 1
name: Example Docs
source:
  type: generic
  url: https://example.com/docs
```

### Schema Migration

```rust
pub fn load_source_config(path: &Path) -> Result<SourceConfig> {
    let raw: RawConfig = serde_yaml::from_reader(File::open(path)?)?;

    let config = match raw.schema_version {
        1 => parse_v1(raw),
        2 => parse_v2(raw),
        _ => return Err(ConfigError::UnsupportedVersion(raw.schema_version)),
    };

    // Auto-upgrade to latest schema on save
    if raw.schema_version < CURRENT_SCHEMA_VERSION {
        tracing::info!("Upgrading config schema from v{} to v{}", raw.schema_version, CURRENT_SCHEMA_VERSION);
        save_source_config(path, &config)?;
    }

    Ok(config)
}
```

---

## Database Schema Versioning

### Migration System

```rust
// src-tauri/src/migrations/mod.rs
pub const MIGRATIONS: &[Migration] = &[
    Migration::new(1, "create_sources_table", include_str!("001_sources.sql")),
    Migration::new(2, "create_pages_table", include_str!("002_pages.sql")),
    Migration::new(3, "add_sync_status", include_str!("003_sync_status.sql")),
];

pub async fn run_migrations(db: &SqlitePool) -> Result<()> {
    let current = get_schema_version(db).await?;

    for migration in MIGRATIONS.iter().filter(|m| m.version > current) {
        tracing::info!("Running migration {}: {}", migration.version, migration.name);
        sqlx::query(migration.sql).execute(db).await?;
        set_schema_version(db, migration.version).await?;
    }

    Ok(())
}
```

### Rollback Support

```sql
-- Each migration has an up and down
-- migrations/003_sync_status.up.sql
ALTER TABLE bookmarks ADD COLUMN sync_status TEXT DEFAULT 'pending';

-- migrations/003_sync_status.down.sql
ALTER TABLE bookmarks DROP COLUMN sync_status;
```

---

## Breaking Change Policy

### Communication

1. **Changelog** - Document all changes
2. **Deprecation warnings** - In responses and logs
3. **Migration guide** - For major versions

### Changelog Format

```markdown
# Changelog

## [2.0.0] - 2027-01-15

### Breaking Changes
- Removed `/api/v1/legacy` endpoint. Use `/api/v2/modern` instead.
- Changed `source.type` from string to enum.

### Migration Guide
See [MIGRATION-v2.md](./MIGRATION-v2.md)

## [1.2.0] - 2026-06-01

### Added
- New `collections` endpoint
- `created_at` field in bookmark responses

### Deprecated
- `/api/v1/legacy` endpoint (use `/api/v2/modern`)
```

### Migration Guides

```markdown
# Migrating from v1 to v2

## API Changes

### Search Endpoint

**v1:**
```json
GET /api/v1/search?q=iterator&limit=10
```

**v2:**
```json
GET /api/v2/search
{
  "query": "iterator",
  "options": { "limit": 10 }
}
```

## Configuration Changes

The `sync.strategy` field now requires an object instead of string:

**v1:**
```yaml
sync:
  strategy: weekly
```

**v2:**
```yaml
sync:
  strategy:
    type: scheduled
    interval: weekly
```
```

---

## Version Discovery

### API Version Endpoint

```http
GET /api/version

{
  "api_version": "1.2.0",
  "app_version": "1.2.0",
  "supported_api_versions": ["v1"],
  "deprecated_api_versions": [],
  "mcp_protocol_versions": ["2024-11-05", "2024-09-01"]
}
```

### CLI Version

```bash
tome --version
tome 1.2.0

tome version --json
{
  "app_version": "1.2.0",
  "api_version": "1.2.0",
  "rust_version": "1.75.0",
  "build_date": "2026-06-01"
}
```

---

## Compatibility Matrix

| Tome Version | API v1 | API v2 | MCP 2024-11 | MCP 2024-09 |
|--------------|--------|--------|-------------|-------------|
| 1.0.x | ✓ | - | ✓ | ✓ |
| 1.1.x | ✓ | - | ✓ | ✓ |
| 1.2.x | ✓ | - | ✓ | ✓ |
| 2.0.x | ✓* | ✓ | ✓ | ✓ |
| 2.1.x | ✓* | ✓ | ✓ | - |

\* Deprecated, will be removed in 3.0
