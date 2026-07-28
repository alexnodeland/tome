# Rollback & Recovery Procedures

This document covers procedures for recovering from failures, rolling back problematic releases, and handling data recovery scenarios.

---

## Release Rollback

### When to Rollback

Rollback a release when:
- Critical bug affects core functionality
- Data corruption or loss occurring
- Security vulnerability discovered
- Build was incorrectly signed/notarized

### Rollback Decision Matrix

| Severity | Users Affected | Action |
|----------|----------------|--------|
| Critical | > 10% | Immediate rollback |
| Critical | < 10% | Hotfix within 24h |
| High | Any | Hotfix within 48h |
| Medium | Any | Fix in next release |
| Low | Any | Fix when convenient |

### Rollback Procedure

```bash
# 1. Identify the last known good version
GOOD_VERSION="v1.0.1"

# 2. Create rollback release
git checkout $GOOD_VERSION
git checkout -b rollback/from-v1.0.2

# 3. Bump patch version (v1.0.3 as rollback)
# Edit version in Cargo.toml, package.json, tauri.conf.json

# 4. Update changelog
echo "## [1.0.3] - $(date +%Y-%m-%d)

### Rollback
- Rolled back to v1.0.1 due to critical bug in v1.0.2
- Issue: [link to issue]
" >> CHANGELOG.md

# 5. Commit and tag
git add -A
git commit -m "chore: rollback to v1.0.1 (release as v1.0.3)"
git tag v1.0.3
git push origin rollback/from-v1.0.2 --tags

# 6. CI will build and publish
# 7. Update Homebrew cask to point to v1.0.3
```

### Communication During Rollback

**GitHub Release Notes:**
```markdown
# Tome v1.0.3 (Rollback)

⚠️ **This is a rollback release**

We discovered a critical issue in v1.0.2 that could cause [description].

This release reverts to the stable v1.0.1 codebase.

## What Happened
[Brief explanation]

## What to Do
Update to v1.0.3 as soon as possible.

## Affected Users
If you experienced [symptoms], please [recovery steps].

We apologize for the inconvenience and are working on a proper fix.
```

---

## User Rollback

### How Users Can Rollback

If a user wants to downgrade:

1. **Download previous version** from GitHub Releases
2. **Quit Tome**
3. **Delete current app** from /Applications
4. **Install previous version** from DMG
5. **Launch** - Tome should work with existing data

### Version Compatibility

| Scenario | Compatibility |
|----------|---------------|
| v1.0.1 → v1.0.2 | ✓ Forward migrations run automatically |
| v1.0.2 → v1.0.1 | ⚠️ **Only if no migration ran between them** — see below |
| v1.x → v2.x | ⚠️ Migration required |
| v2.x → v1.x | ✗ Not supported |

> "Usually compatible" is too generous, and in a recovery document that is dangerous. Whether a
> downgrade is safe depends on whether a schema migration ran, which is not knowable from the
> version numbers. **The app records the schema version in the database and refuses to open one
> newer than it understands**, telling the user to reinstall the newer version rather than opening
> it and corrupting data. The real downgrade path is the automatic pre-migration backup
> (`tome.db.pre-<version>`), not a down migration.

### Database Migration Rollback

If a database migration breaks things:

```rust
// Each migration has a down migration
pub async fn rollback_migration(db: &SqlitePool, version: u32) -> Result<()> {
    let migration = get_migration(version)?;

    tracing::warn!("Rolling back migration {}: {}", version, migration.name);

    sqlx::query(migration.down_sql).execute(db).await?;
    set_schema_version(db, version - 1).await?;

    Ok(())
}
```

**User-initiated rollback:**
```bash
# Not normally exposed, but available for emergencies
tome debug rollback-migration --version 5
```

---

## Data Recovery

### Backup Locations

**Authoritative layout: [PRD § File System Layout](../PRD.md#file-system-layout).** This document
previously described a third, different set of paths — the plan set contained `~/.tome/`,
`~/Library/Application Support/Tome`, `~/Library/Caches/com.example.tome`, and
`dirs::data_dir()/tome/` across four documents. In a recovery procedure a wrong path is worse than
no path: it sends someone deleting the wrong directory during an incident.

```
~/Library/Application Support/Tome/     # BACK THIS UP — irreplaceable
├── config.yaml
├── sources/                            # source configs (YAML)
├── tome.db                             # bookmarks, annotations, metadata
└── logs/

~/Library/Caches/Tome/                  # safe to delete — rebuildable
├── data/<source-id>/{pages,raw,assets}
└── index/

~/Library/Mobile Documents/iCloud~<bundle-id>/Documents/   # synced
└── devices/<device-id>/{manifest.json,ops-*.jsonl}
```

**Only the first directory matters for backup.** Everything under Caches can be re-fetched, and
saying so plainly is what stops someone backing up 40 GB of cached documentation.

### What Can Be Recovered

| Data | Recovery Method | Likelihood |
|------|-----------------|------------|
| Bookmarks/Annotations | iCloud sync | High |
| Reading positions | iCloud sync | High |
| Source configs | Filesystem backup | Medium |
| Documentation cache | Re-fetch from source | High |
| Search index | Rebuild from cache | High |

### Recovery Procedures

#### Database Corruption

```bash
DB=~/Library/Application\ Support/Tome/tome.db

# 1. Quit Tome, and any `tome serve` / `tome mcp` processes holding the database
pkill -f 'tome (serve|mcp)' || true

# 2. Back up the corrupted database BEFORE touching it. Also copy the -wal and
#    -shm files: without them a WAL-mode database is missing its most recent
#    committed transactions, which are exactly the ones you are trying to save.
cp "$DB" "$DB.corrupted"
cp "$DB-wal" "$DB.corrupted-wal" 2>/dev/null || true
cp "$DB-shm" "$DB.corrupted-shm" 2>/dev/null || true

# 3. Attempt recovery
sqlite3 "$DB.corrupted" ".recover" | sqlite3 "$DB.recovered"

# 4. Verify BEFORE replacing — .recover succeeds on output that is still broken
sqlite3 "$DB.recovered" "PRAGMA integrity_check;"   # must print: ok
sqlite3 "$DB.recovered" "SELECT count(*) FROM bookmarks;"

# 5. If both look right, replace
mv "$DB.recovered" "$DB"

# 6. If recovery fails, start fresh — bookmarks return from iCloud on next
#    launch if sync is enabled. If it is NOT enabled, this step loses them:
#    check `tome export` output exists first.
rm "$DB"
```

#### Search Index Corruption

```bash
# Tome can detect and auto-rebuild
# Or force rebuild:
tome debug rebuild-index

# This will:
# 1. Delete ~/Library/Caches/Tome/index/
# 2. Re-index all cached documentation
# 3. Takes a few minutes for large doc sets
```

#### iCloud Sync Recovery

If sync data is corrupted:

```bash
# 1. Export local data first — always, before anything destructive
tome export --all --output ~/tome-backup/

# 2. Reset this device's sync state (drops local replay offsets and this
#    device's op log; does NOT touch other devices' logs)
tome debug reset-sync

# 3. Replay from the container
tome debug resync

# 4. If still broken, restore from the export
tome import ~/tome-backup/
```

> There is no `tome sync --force`. `sync` is not a CLI command — bookmark sync is automatic and
> `pull` fetches documentation. See [PRD § CLI Specification](../PRD.md#cli-specification).

### Manual Data Export

```bash
# Export all user data to JSON
tome export --all --output ~/tome-export/

# Creates:
# ~/tome-export/bookmarks.json
# ~/tome-export/collections.json
# ~/tome-export/sources.json
# ~/tome-export/reading-positions.json
```

### Data Integrity Checks

```rust
// Built-in integrity checks
pub async fn check_integrity(db: &SqlitePool) -> IntegrityReport {
    let mut report = IntegrityReport::default();

    // SQLite integrity check
    let sqlite_ok = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
        .fetch_one(db)
        .await
        .map(|r| r == "ok")
        .unwrap_or(false);

    report.database_ok = sqlite_ok;

    // `PRAGMA foreign_key_check` returns one ROW PER VIOLATION, and no rows when
    // clean -- it is not a scalar. `query_scalar(...).fetch_one(...)` therefore
    // ERRORS on a healthy database, and `unwrap_or(-1)` turns that error into
    // "not ok". The original integrity check reported every clean database as
    // broken.
    let fk_violations: Vec<FkViolation> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(db)
        .await
        .unwrap_or_default();

    report.foreign_keys_ok = fk_violations.is_empty();
    report.fk_violations = fk_violations;   // report WHAT is broken, not just that it is

    // Orphaned records check
    report.orphaned_pages = count_orphaned_pages(db).await;
    report.orphaned_bookmarks = count_orphaned_bookmarks(db).await;

    report
}
```

Exposed to users:
```bash
tome debug check-integrity

# Output:
# Database: OK
# Foreign keys: OK
# Orphaned pages: 0
# Orphaned bookmarks: 0
# Search index: OK (15234 documents)
```

---

## Disaster Recovery

### Complete Data Loss

If `~/Library/Application Support/Tome/` is completely lost:

1. **Reinstall Tome** — download from releases or Homebrew
2. **Launch** — creates a fresh directory structure
3. **Sign in to iCloud** — bookmarks, annotations, collections and reading positions replay from
   the container automatically
4. **Re-add sources.** ⚠️ **Source configurations are not synced**, so this is the one thing that
   does not come back. This is a real gap: a user who loses their machine keeps their bookmarks but
   loses the list of documentation those bookmarks point *into*, which makes them useless until
   each source is re-added by hand.
   **Recommendation: sync source configurations too.** They are small (a few KB of YAML), they
   contain no secrets, and they are what makes the restore actually complete. Tracked as a change
   to P3-010's synced entity list.
5. **Re-sync documentation** — `tome pull --all`

### Partial Recovery Checklist

```
[ ] Check Time Machine backup for ~/Library/Application Support/Tome/
[ ] Check iCloud for synced data
[ ] Check ~/tome-backup/ or similar for exports
[ ] Recreate source configs from memory/browser history
[ ] Run integrity checks after recovery
```

### Time Machine Recovery

```bash
# Find Tome backups in Time Machine
tmutil listbackups | head -5

# Restore specific directory
tmutil restore "/Volumes/Time Machine/.../Library/Application Support/Tome" ~/Tome-recovered

# Or use Time Machine UI to browse and restore
```

---

## Incident Response

### Incident Severity Levels

**These are internal prioritization targets, not commitments to users.**
`16-support-maintenance.md` states "no SLAs or guaranteed response times", and that is the public
position. A solo maintainer cannot promise a four-hour response, and publishing one that is missed
costs more trust than never having made it.

| Level | Description | Target (best effort) |
|-------|-------------|----------------------|
| **P0** | Data loss or security vulnerability | Drop everything |
| **P1** | App unusable, crash loop | Same day |
| **P2** | Major feature broken | Next release |
| **P3** | Minor feature broken | Backlog |

### Incident Response Checklist

```markdown
## Incident: [Brief description]
**Reported:** [Date/time]
**Severity:** P0/P1/P2/P3
**Status:** Investigating / Mitigating / Resolved

### Timeline
- HH:MM - Incident reported via [source]
- HH:MM - Investigation started
- HH:MM - Root cause identified
- HH:MM - Mitigation deployed
- HH:MM - Incident resolved

### Impact
- Users affected: [estimate]
- Data affected: [description]

### Root Cause
[Explanation]

### Resolution
[What was done to fix it]

### Prevention
[What will prevent recurrence]

### Action Items
- [ ] Action 1
- [ ] Action 2
```

### Post-Incident Review

After any P0/P1 incident:

1. **Timeline** - What happened when
2. **Root cause** - Why it happened
3. **Impact** - What was affected
4. **Detection** - How was it discovered
5. **Response** - What was done
6. **Prevention** - How to prevent recurrence
7. **Action items** - Specific follow-ups

---

## Testing Recovery Procedures

### Quarterly Recovery Drill

Test recovery procedures quarterly:

1. **Backup current data** (just in case)
2. **Simulate failure** (e.g., delete database)
3. **Execute recovery procedure**
4. **Verify data integrity**
5. **Document issues found**
6. **Update procedures if needed**

### Recovery Test Scenarios

| Scenario | Test Method |
|----------|-------------|
| Database corruption | Delete tome.db, verify recreation |
| Index corruption | Delete index/, verify rebuild |
| iCloud sync loss | Reset sync, verify restore |
| Full reinstall | Delete Application Support + Caches, reinstall, verify |
| Version rollback | Install old version, verify compatibility |

---

## Debug Commands Reference

```bash
# Integrity checks
tome debug check-integrity
tome debug check-database
tome debug check-index

# Recovery commands
tome debug rebuild-index
tome debug reset-sync
tome debug rollback-migration --version N

# Export/Import
tome export --all --output ~/backup/
tome import ~/backup/

# Reset (destructive)
tome debug reset --confirm  # Deletes all local data
```

**Note:** Debug commands are hidden from `--help` by default. Use `tome debug --help` to see them.
