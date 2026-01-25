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
| v1.0.1 → v1.0.2 | ✓ Full compatibility |
| v1.0.2 → v1.0.1 | ✓ Usually compatible |
| v1.x → v2.x | ⚠️ Migration required |
| v2.x → v1.x | ✗ May not work |

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

Tome data is stored in:

```
~/.tome/
├── config.yaml              # Configuration
├── sources/                 # Source configs (YAML)
├── data/                    # Cached documentation
├── index/                   # Search index
├── tome.db                  # SQLite database
└── logs/                    # Log files
```

**Synced via iCloud:**
```
~/Library/Mobile Documents/iCloud~com~example~tome/
├── bookmarks.json
├── collections.json
├── reading-positions.json
└── sources-meta.json
```

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
# 1. Stop Tome
# 2. Backup corrupted database
cp ~/.tome/tome.db ~/.tome/tome.db.corrupted

# 3. Try to recover with sqlite3
sqlite3 ~/.tome/tome.db.corrupted ".recover" | sqlite3 ~/.tome/tome.db.recovered

# 4. If recovery works, replace
mv ~/.tome/tome.db.recovered ~/.tome/tome.db

# 5. If recovery fails, start fresh
rm ~/.tome/tome.db
# Tome will recreate on next launch

# 6. Re-sync from iCloud to restore bookmarks
# (happens automatically on launch)
```

#### Search Index Corruption

```bash
# Tome can detect and auto-rebuild
# Or force rebuild:
tome debug rebuild-index

# This will:
# 1. Delete ~/.tome/index/
# 2. Re-index all cached documentation
# 3. Takes a few minutes for large doc sets
```

#### iCloud Sync Recovery

If sync data is corrupted:

```bash
# 1. Export local data first
tome export --format json --output ~/tome-backup.json

# 2. Reset sync state
tome debug reset-sync

# 3. Force full sync
tome sync --force

# 4. If still broken, restore from local backup
tome import ~/tome-backup.json
```

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

    // Foreign key check
    let fk_violations: i64 = sqlx::query_scalar("PRAGMA foreign_key_check")
        .fetch_one(db)
        .await
        .unwrap_or(-1);

    report.foreign_keys_ok = fk_violations == 0;

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

If `~/.tome/` is completely lost:

1. **Reinstall Tome** - Download from releases or Homebrew
2. **Launch** - Creates fresh directory structure
3. **Sign in to iCloud** - Automatically syncs bookmarks, etc.
4. **Re-add sources** - Source configs need to be recreated
5. **Re-sync documentation** - `tome pull --all`

### Partial Recovery Checklist

```
[ ] Check Time Machine backup for ~/.tome/
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
tmutil restore /Volumes/Time\ Machine/.../Users/you/.tome ~/.tome-recovered

# Or use Time Machine UI to browse and restore
```

---

## Incident Response

### Incident Severity Levels

| Level | Description | Response Time |
|-------|-------------|---------------|
| **P0** | Data loss, security breach | Immediate |
| **P1** | App unusable, crash loop | < 4 hours |
| **P2** | Major feature broken | < 24 hours |
| **P3** | Minor feature broken | < 1 week |

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
| Full reinstall | Delete ~/.tome/, reinstall, verify |
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
