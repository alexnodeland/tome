# Phase 5: Polish & Launch (v1.0)

**Goal:** Production-ready release
**Tickets:** 14
**Effort:** ~54.5 person-days
**Prerequisites:** Phases 1-4 complete
**Exit Criteria:** App is stable, performant, and ready for public use

> **Both blocking prerequisites are now resolved.**
>
> * **DEC-002** — the bundle identifier is `com.alexnodeland.tome` ([ADR-0004](../decisions/0004-bundle-identifier.md)).
> * **DEC-003** — the Apple Developer Program is **deferred**
>   ([ADR-0006](../decisions/0006-unsigned-distribution.md)). Tome ships **unsigned and
>   un-notarized** through `alexnodeland/homebrew-tap`. **P5-010 is therefore deferred**, and the
>   phase exit criterion changes from "passes Gatekeeper" to "installs from the tap and launches
>   after the documented quarantine step".
>
> The cost is real and is not hidden: Gatekeeper blocks first launch, and on macOS 15+ the
> Control-click bypass no longer exists. The cask's `caveats` carry the instructions.

---

## Ticket Summary

| ID | Title | Complexity | Priority | Dependencies |
|----|-------|------------|----------|--------------|
| P5-001 | Performance profiling and optimization | L | Critical | All previous |
| P5-002 | Implement lazy loading for large doc sets | M | High | P5-001 |
| P5-003 | Add incremental/delta indexing | M | High | P2-003 |
| P5-004 | Comprehensive error handling audit | M | High | All previous |
| P5-005 | Build error recovery system | M | High | P5-004 |
| P5-006 | Create onboarding experience | M | High | P1-022, P2-014 |
| P5-007 | Build preferences UI | M | Critical | P1-017 |
| P5-008 | Implement menu bar integration | M | High | P1-001 |
| P5-009 | Add global keyboard shortcut | S | Medium | P5-008 |
| ~~P5-010~~ | ~~macOS notarization setup~~ — **deferred**, ADR-0006 | M | — | Needs DEC-003 reversed |
| P5-011 | Build DMG installer (unsigned) | M | High | — |
| P5-012 | Publish to `alexnodeland/homebrew-tap` | S | Critical | P5-011 |
| P5-013 | Write user documentation | M | High | All previous |
| P5-014 | Create landing page | M | High | All previous |

---

## Detailed Tickets

### P5-001: Performance profiling and optimization

**Priority:** Critical
**Complexity:** L (1-2 weeks)
**Dependencies:** All previous phases
**Blocks:** P5-002, P5-003

#### Description
Profile the application and optimize critical performance paths.

#### Acceptance Criteria
- [ ] Profile startup time and optimize
- [ ] Profile memory usage and reduce
- [ ] Profile search latency and optimize
- [ ] Profile rendering performance
- [ ] Profile sync operations
- [ ] Document performance baselines
- [ ] Automated performance tests

#### Technical Notes
```rust
// Profiling tools
// - Instruments (macOS) for CPU/memory
// - cargo flamegraph for Rust hotspots
// - Chrome DevTools for WebView

// Key metrics to measure
struct PerformanceMetrics {
    startup_time_ms: u64,           // Target: < 500ms
    memory_idle_mb: u64,            // Target: < 200MB
    search_p95_ms: u64,             // Target: < 100ms
    page_render_p95_ms: u64,        // Target: < 100ms
    index_1000_pages_s: u64,        // Target: < 30s
}

// Optimization areas
// 1. Lazy initialization of heavy components
// 2. Connection pooling for SQLite
// 3. Memory-mapped I/O for large files
// 4. Index pruning for old content
// 5. WebView caching strategies
```

**Optimization Checklist:**
- [ ] Defer non-critical initialization
- [ ] Use connection pooling
- [ ] Implement query result caching
- [ ] Optimize CSS/rendering pipeline
- [ ] Reduce IPC overhead
- [ ] Profile and optimize hot loops

#### Success Metrics
- Startup < 500ms cold
- Memory < 200MB idle
- Search < 100ms P95

---

### P5-002: Implement lazy loading for large doc sets

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P5-001
**Blocks:** None

#### Description
Implement lazy loading to handle large documentation sets efficiently.

#### Acceptance Criteria
- [ ] Load page content on demand
- [ ] Cache recently accessed pages
- [ ] Evict old cache entries (LRU)
- [ ] Show loading state for slow loads
- [ ] Preload adjacent pages (optional)
- [ ] Memory usage bounded regardless of doc size

#### Technical Notes
```rust
pub struct PageCache {
    cache: LruCache<String, CachedPage>,
    max_size_bytes: usize,
    current_size: AtomicUsize,
}

impl PageCache {
    pub fn new(max_size_mb: usize) -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            max_size_bytes: max_size_mb * 1024 * 1024,
            current_size: AtomicUsize::new(0),
        }
    }

    pub async fn get_or_load(&self, key: &str, loader: impl Future<Output = Result<Page>>) -> Result<Arc<Page>> {
        if let Some(cached) = self.cache.get(key) {
            return Ok(cached.page.clone());
        }

        let page = loader.await?;
        let size = page.estimated_size();

        // Evict if necessary
        while self.current_size.load(Ordering::Relaxed) + size > self.max_size_bytes {
            if let Some((_, evicted)) = self.cache.pop_lru() {
                self.current_size.fetch_sub(evicted.size, Ordering::Relaxed);
            } else {
                break;
            }
        }

        let page = Arc::new(page);
        self.cache.push(key.to_string(), CachedPage { page: page.clone(), size });
        self.current_size.fetch_add(size, Ordering::Relaxed);

        Ok(page)
    }
}
```

#### Success Metrics
- Memory bounded to configured limit under a scripted browsing soak
- Cache hit rate > 80% **measured by the local diagnostics counter** during that soak — this is a
  lab measurement, not an observation of real users, since there is no telemetry
- Load time < 200ms for cache miss

---

### P5-003: Add incremental/delta indexing

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P2-003 (Indexing pipeline)
**Blocks:** None

#### Description
Implement incremental indexing to avoid full re-index on updates.

#### Acceptance Criteria
- [ ] Detect changed pages via content hash
- [ ] Index only changed documents
- [ ] Remove deleted documents
- [ ] Merge index segments efficiently
- [ ] Track index health metrics
- [ ] Force full re-index option

#### Technical Notes
```rust
pub struct IncrementalIndexer {
    search_engine: Arc<SearchEngine>,
    page_repo: Arc<PageRepository>,
}

impl IncrementalIndexer {
    pub async fn sync_index(&self, source_id: &str) -> Result<IndexStats> {
        let db_pages = self.page_repo.list_pages(source_id).await?;
        let index_state = self.search_engine.get_source_state(source_id)?;

        let mut stats = IndexStats::default();

        // Find pages to add or update
        for page in &db_pages {
            let key = format!("{}:{}", page.path, page.content_hash);
            if !index_state.contains(&key) {
                // Page is new or changed
                let content = self.load_content(page).await?;
                self.search_engine.index_document(source_id, page, &content)?;
                stats.indexed += 1;
            }
        }

        // Find pages to remove
        let current_paths: HashSet<_> = db_pages.iter().map(|p| &p.path).collect();
        for indexed_path in index_state.paths() {
            if !current_paths.contains(indexed_path) {
                self.search_engine.delete_document(source_id, indexed_path)?;
                stats.removed += 1;
            }
        }

        // Commit changes
        self.search_engine.commit()?;

        // Maybe merge segments
        if stats.indexed > 100 || stats.removed > 50 {
            self.search_engine.optimize()?;
        }

        Ok(stats)
    }
}
```

#### Success Metrics
- Incremental index < 10s for 100 changes
- Index size growth controlled
- No stale documents

---

### P5-004: Comprehensive error handling audit

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** All previous phases
**Blocks:** P5-005

#### Description
Audit all error paths and ensure consistent, user-friendly error handling.

> **Done by S4-3, 2026-07-30.** The taxonomy was already frozen (S0-4); what this ticket added was
> the *audit that keeps it honest*. `Error::suggestion` no longer has a `_ =>` arm, so adding a
> variant stops the build until someone decides what a person should do about it — a catch-all is
> how twenty variants came to share two suggestions. The audit found six messages that were
> fragments rather than sentences on its first run, and one that named `tome pull --all` where
> `tome debug rebuild-index` would have saved a full re-crawl.

#### Acceptance Criteria
- [x] All errors categorized — `tome-core/src/error.rs`, exhaustively matched by
      `error::tests::variant`, so a new variant cannot be added without being categorised
- [x] User-friendly error messages — every one is a whole sentence, asserted
- [x] Actionable error suggestions — every variant returns one, or is named in `NO_ACTION` with a
      reason. `BlockedByRobots` and `Io` are the two exceptions, and both are decisions
- [x] **Every command an error names must exist** — `REAL_COMMANDS`. This ticket's own technical
      notes made that mistake, suggesting `tome debug rebuild-index` when there was no such command;
      the list is that note, enforced
- [x] No stack traces shown to users — asserted against `Custom {`, `kind:`, `panicked at`, `::`
- [x] Errors logged locally for debugging — `tome-core/src/logging.rs`. `logs/` had been in the PRD
      and created by `Paths::ensure_created` since S0-3, and **nothing had ever written to it**.
      Daily rotation, 7-day retention, one `write_all` per event so two processes cannot interleave,
      and created lazily so a read-only command still creates nothing
- [x] **No error telemetry of any kind.** Nothing here leaves the machine.
- [x] Redacted diagnostics — `tome debug report`. No page paths, no search queries, no note text,
      `$HOME` rewritten to `~`. A test types a distinctive query and asserts it appears nowhere

#### Technical Notes
```rust
#[derive(Debug, thiserror::Error)]
pub enum TomeError {
    // Network errors
    #[error("Could not connect to {url}. Check your internet connection.")]
    NetworkError { url: String, #[source] source: reqwest::Error },

    #[error("The documentation site at {url} is not responding.")]
    Timeout { url: String },

    // File errors
    #[error("Could not read file: {path}")]
    FileNotFound { path: PathBuf },

    #[error("Permission denied: {path}. Check file permissions.")]
    PermissionDenied { path: PathBuf },

    // Configuration errors
    #[error("Invalid configuration in {file}: {message}")]
    ConfigError { file: PathBuf, message: String },

    // Search errors
    // The command is `tome debug rebuild-index`. Error strings naming a
    // non-existent command are worse than no suggestion at all.
    #[error("Search index is corrupted. Try rebuilding with `tome debug rebuild-index`.")]
    IndexCorrupted,

    // Sync errors
    #[error("iCloud sync failed. Make sure you're signed in to iCloud.")]
    SyncError { #[source] source: SyncError },
}

impl TomeError {
    pub fn suggestion(&self) -> Option<&str> {
        match self {
            Self::NetworkError { .. } => Some("Try again later or check your network."),
            Self::Timeout { .. } => Some("The server might be busy. Try again later."),
            Self::IndexCorrupted => Some("Run `tome debug rebuild-index` to fix."),
            _ => None,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::NetworkError { .. } | Self::Timeout { .. } | Self::SyncError { .. })
    }
}
```

#### Success Metrics
- All error types categorized
- User-facing messages helpful
- No internal details exposed

---

### P5-005: Build error recovery system

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P5-004
**Blocks:** None

#### Description
Implement automatic recovery from common error conditions.

> **Done by S4-3, 2026-07-30, without the `RecoveryManager` below.** Retry already lived in the
> fetcher (S1-4), where it belongs — it is the only layer that knows about `Retry-After`. A second
> generic retry wrapper above it would have retried operations that had already exhausted their
> retries. What was actually missing was a way for a *person* to repair a library, which is the
> commands rather than the abstraction.

#### Acceptance Criteria
- [x] Retry network errors with backoff — `fetch::Fetcher::request_with_retry`, honouring
      `Retry-After` on 429/503 and backing off exponentially on other 5xx
- [x] Rebuild corrupted indexes — `SearchEngine::open_or_rebuild` discards an index that will not
      open, and **`tome debug rebuild-index` repopulates it from local content with no network**.
      Before this, a discarded index left search silently empty until a full re-crawl
- [~] Recover from crashed sync state — **there is no sync.** ADR-0001 designs it and Stage 5 is
      deferred; this lands with it
- [x] Graceful degradation when offline — every read path is local. `rebuild-index` is asserted to
      make zero requests while a server is running and reachable
- [x] User notification for unrecoverable errors — `tome debug check` names the remedy for each
      finding, and exits non-zero so a script can act on it
- [~] Recovery actions in error dialogs — the CLI's half is done; the app's dialogs are S4-4/S4-5

#### Technical Notes
```rust
pub struct RecoveryManager {
    max_retries: u32,
    base_delay: Duration,
}

impl RecoveryManager {
    pub async fn with_retry<T, F, Fut>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut attempt = 0;
        let mut delay = self.base_delay;

        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) if e.is_retryable() && attempt < self.max_retries => {
                    attempt += 1;
                    tracing::warn!("Attempt {} failed, retrying in {:?}: {}", attempt, delay, e);
                    tokio::time::sleep(delay).await;
                    delay *= 2; // Exponential backoff
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub async fn recover_index(&self) -> Result<()> {
        tracing::info!("Attempting index recovery...");

        // 1. Backup current index
        let backup_path = backup_index()?;

        // 2. Rebuild from database
        match rebuild_index().await {
            Ok(_) => {
                tracing::info!("Index rebuilt successfully");
                fs::remove_dir_all(backup_path)?;
                Ok(())
            }
            Err(e) => {
                tracing::error!("Rebuild failed, restoring backup: {}", e);
                restore_backup(backup_path)?;
                Err(e)
            }
        }
    }
}
```

#### Success Metrics
- Network errors recovered 95%+
- Index corruption auto-repaired
- User informed of recovery actions

---

### P5-006: Create onboarding experience

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-022 (Manual add), P2-014 (Auto-detect)
**Blocks:** None

#### Description
Build a first-run experience that helps users get started.

> **Done by S4-4, 2026-07-30**, in three steps rather than five. The `tour` step is gone: a tour
> of three panels is a tour of what the user can already see, and the shortcuts step does the work
> it would have done. The `popularSources` array in the sketch below is exactly the hardcoded list
> the criteria then forbid — the catalogue is read from `registry/index.yaml`, which ships in the
> bundle as a Tauri resource.

#### Acceptance Criteria
- [x] Welcome screen on first launch — and only on a **first** run: dismissal is remembered, so
      removing your last source does not make you a first-time user again
- [x] **First source installed from the registry in one click** — `install_registry_source` writes
      the configuration and pulls, through the ordinary pipeline with robots.txt, the rate limit
      and the SSRF filter all inherited
- [x] Suggestions drawn from the registry rather than hardcoded, grouped by category, with the
      `verified` date shown — a stale date is the only warning that a scraper has rotted
- [x] Progress for the first sync — `install-progress` events, crawling/storing/indexing. The
      crawl phase shows a count and **no denominator**, because the total is unknown until the
      crawl ends and an invented one makes a bar that goes backwards
- [~] Tour of key features — replaced by the shortcuts step, see above
- [x] Keyboard shortcuts overview — `src/lib/shortcuts.ts`, listing **only what is bound**. A
      panel that showed ⌘D for "bookmark page" would teach the user something false, and they
      would find out by pressing it
- [x] Skip on every step, and the shell is never blocked
- [x] Works with no network — the catalogue is in the bundle, so the list renders offline and an
      install failure says what happened instead of leaving a spinner

#### Technical Notes
```svelte
<script>
  import { onMount } from 'svelte';
  import { isFirstRun, markOnboardingComplete } from '$lib/stores/settings';

  let step = 0;
  const steps = ['welcome', 'addSource', 'tour', 'shortcuts', 'complete'];

  const popularSources = [
    { name: 'Rust Standard Library', url: 'https://doc.rust-lang.org/std/' },
    { name: 'Python 3', url: 'https://docs.python.org/3/' },
    { name: 'React', url: 'https://react.dev/reference/' },
    { name: 'TypeScript', url: 'https://www.typescriptlang.org/docs/' },
    { name: 'MDN Web Docs', url: 'https://developer.mozilla.org/en-US/' },
  ];
</script>

{#if $isFirstRun}
  <div class="onboarding-modal">
    {#if steps[step] === 'welcome'}
      <WelcomeStep on:next={() => step++} on:skip={skipOnboarding} />
    {:else if steps[step] === 'addSource'}
      <AddSourceStep
        {popularSources}
        on:next={() => step++}
        on:back={() => step--}
      />
    {:else if steps[step] === 'tour'}
      <FeatureTourStep
        on:next={() => step++}
        on:back={() => step--}
      />
    {:else if steps[step] === 'shortcuts'}
      <ShortcutsStep
        on:next={() => step++}
        on:back={() => step--}
      />
    {:else if steps[step] === 'complete'}
      <CompleteStep on:finish={completeOnboarding} />
    {/if}

    <StepIndicator current={step} total={steps.length} />
  </div>
{/if}
```

#### Success Metrics
- < 2 minutes to complete in moderated testing with 5 first-time users
- Every participant ends with at least one working source
- No participant needs to open the documentation to finish

> "80 %+ users complete onboarding" was removed: Tome collects no telemetry, so completion rate is
> unobservable in production. Moderated testing with a handful of real people measures the same
> thing better and is actually possible.

---

### P5-007: Build preferences UI

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P1-017 (Layout)
**Blocks:** None

#### Description
Create the preferences window for configuring Tome.

> **Done by S4-5, 2026-07-30**, with a different shape from the schema below. The appearance
> preferences are **named steps, not free values** — `data-text-size="large"`, not
> `font_size: 19` — because `tokens.css` rescales the whole system off the root font size and
> every other size is a rem against it. A `font_size: 13` would shrink the UI chrome along with
> the prose. Same for the measure: `narrow`/`default`/`wide` in `ch`, so the column holds the
> same number of words at any text size.
>
> They live in `localStorage`, not a YAML file. The schema below implies a file that both the app
> and the CLI would read, but the CLI has no appearance and a sidebar width in a config file is a
> migration to maintain — the rule `stores/preferences.ts` already states.

#### Acceptance Criteria
- [x] Cmd+, opens preferences
- [x] General tab — confirm-before-remove, and an honest note that Tome does not check for updates
- [x] Appearance tab — theme, text size, column width, code line numbers
- [~] Sync tab — **omitted. There is no sync.** ADR-0001 designs it and Stage 5 is deferred; a tab
      of controls that do nothing is worse than no tab
- [~] Keyboard tab — a **reference**, not customisation. Shortcuts are not rebindable, and fields
      that discard what is typed into them are worse than saying so
- [x] Library tab (paths, version) — the same paths `tome status` prints, so the app and the CLI
      can be compared without leaving either
- [x] Changes apply immediately — to the DOM, the store, **and the reader frame**, which has an
      opaque origin and cannot inherit the shell's cascade. `ReaderFrame.settings` was built in
      S1-13 and had never had a caller until now
- [x] Reset to defaults

#### Technical Notes
```svelte
<script>
  import { preferences } from '$lib/stores/preferences';

  let activeTab = 'general';

  const tabs = [
    { id: 'general', label: 'General', icon: '⚙️' },
    { id: 'appearance', label: 'Appearance', icon: '🎨' },
    { id: 'sync', label: 'Sync', icon: '☁️' },
    { id: 'keyboard', label: 'Keyboard', icon: '⌨️' },
    { id: 'advanced', label: 'Advanced', icon: '🔧' },
  ];
</script>

<div class="preferences-window">
  <nav class="preferences-tabs">
    {#each tabs as tab}
      <button
        class:active={activeTab === tab.id}
        on:click={() => activeTab = tab.id}
      >
        <span class="icon">{tab.icon}</span>
        <span class="label">{tab.label}</span>
      </button>
    {/each}
  </nav>

  <div class="preferences-content">
    {#if activeTab === 'general'}
      <GeneralPreferences bind:preferences={$preferences.general} />
    {:else if activeTab === 'appearance'}
      <AppearancePreferences bind:preferences={$preferences.appearance} />
    {:else if activeTab === 'sync'}
      <SyncPreferences bind:preferences={$preferences.sync} />
    {:else if activeTab === 'keyboard'}
      <KeyboardPreferences bind:preferences={$preferences.keyboard} />
    {:else if activeTab === 'advanced'}
      <AdvancedPreferences bind:preferences={$preferences.advanced} />
    {/if}
  </div>
</div>
```

**Preferences Schema:**
```yaml
general:
  launch_at_login: false
  check_for_updates: true
  confirm_before_remove: true

appearance:
  theme: system  # light, dark, system
  font_size: 17
  line_height: 1.6
  code_font: SF Mono
  measure: 70ch

sync:
  enabled: true
  sync_on_launch: true
  conflict_resolution: last_write_wins

keyboard:
  global_shortcut: "Cmd+Shift+D"
  vim_mode: false

advanced:
  data_directory: ~/Library/Application Support/Tome   # $TOME_HOME overrides
  cache_size_mb: 500
  debug_mode: false
```

#### Success Metrics
- All preferences accessible
- Changes apply without restart
- Settings persist correctly

---

### P5-008: Implement menu bar integration

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-001 (Tauri/Swift setup)
**Blocks:** P5-009

#### Description
Add menu bar icon and quick access menu.

> **Done by S4-6, 2026-07-30 — and the Swift below is not what shipped.** SPIKE-001 found that
> Tauri's `tray-icon` feature *is* `NSStatusItem`: no `AppDelegate`, no Objective-C, no plugin
> bridge. The whole thing is `src-tauri/src/tray.rs`, 170 lines, no `unsafe`.
>
> The `NSPopover` is also gone. A popover needs its own content view; the app has one webview and
> the reader is an iframe inside it, so a second surface would mean a second window with its own
> capability set and its own copy of the tokens. Clicking the item raises the window and opens
> search, which is the thing the popover existed to do.

#### Acceptance Criteria
- [x] Menu bar icon — a **template image** (black plus alpha), so macOS recolours it for light,
      dark and highlighted states. Generated by `scripts/make-tray-icon.mjs`; two earlier glyphs
      were discarded for reading as window panes at 22 points
- [x] Click opens quick search — on mouse *up*, so dragging the item along the menu bar to
      reposition it does not also open the app
- [x] Right-click shows the menu
- [~] Recent searches — **not shown.** Search history lives in the frontend's `localStorage` and
      is not readable from Rust. It belongs in the change that makes it reachable
- [~] Bookmarks — **there are none.** Phase 3
- [x] Quick add source — opens the registry catalogue
- [x] Quit
- [x] Hide from Dock — `ActivationPolicy::Accessory`. The menu bar item is created
      unconditionally and before this can be called, so turning it off never leaves a user with
      no way back in

#### Technical Notes
```swift
// Swift AppDelegate for menu bar
class AppDelegate: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem?
    var popover: NSPopover?

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        if let button = statusItem?.button {
            button.image = NSImage(named: "MenuBarIcon")
            button.action = #selector(togglePopover)
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        }

        popover = NSPopover()
        popover?.contentSize = NSSize(width: 400, height: 500)
        popover?.behavior = .transient
        // Content from Tauri webview
    }

    @objc func togglePopover(_ sender: NSStatusBarButton) {
        let event = NSApp.currentEvent!

        if event.type == .rightMouseUp {
            showContextMenu()
        } else {
            if popover?.isShown == true {
                popover?.close()
            } else {
                popover?.show(relativeTo: sender.bounds, of: sender, preferredEdge: .minY)
            }
        }
    }

    func showContextMenu() {
        let menu = NSMenu()
        menu.addItem(NSMenuItem(title: "Search...", action: #selector(openSearch), keyEquivalent: ""))
        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Recent", action: nil, keyEquivalent: ""))
        // Add recent items
        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Preferences...", action: #selector(openPreferences), keyEquivalent: ","))
        menu.addItem(NSMenuItem(title: "Quit Tome", action: #selector(NSApp.terminate), keyEquivalent: "q"))

        statusItem?.menu = menu
        statusItem?.button?.performClick(nil)
        statusItem?.menu = nil
    }
}
```

#### Success Metrics
- Menu bar icon visible
- Quick search accessible
- Popover responsive

---

### P5-009: Add global keyboard shortcut

**Priority:** Medium
**Complexity:** S (1-2 days)
**Dependencies:** P5-008
**Blocks:** None

#### Description
Implement a global keyboard shortcut to activate Tome from anywhere.

> **Done by S4-6, 2026-07-30.** The interesting part is the conflict criterion, which cannot be
> satisfied the obvious way — see below.

#### Acceptance Criteria
- [x] Default ⌘⇧D, **off by default** (PRD Appendix C). A system-wide hotkey claimed at first
      launch is one taken from whatever the user had bound to it, silently
- [x] Customisable — a recorder that captures the next keystroke. It reads letters from
      `event.code`, not `event.key`: with Alt held macOS reports `key` as the composed character,
      and Alt+D arrives as `∂`
- [x] Works in the background and while hidden — `RegisterEventHotKey` is system-wide, and
      `activate` calls `show` before `set_focus` because a hidden window cannot take focus
- [x] **Conflict detection, two-sided — and the obvious half is not enough.** SPIKE-001 measured
      that registering `⌘Space` (Spotlight's) **succeeds**, and the handler then never fires:
      `RegisterEventHotKey` refuses another *application's* hotkey, not the system's, and no API
      lists either. So a failed registration is reported *and* `src/lib/accelerator.ts` refuses a
      list of reserved combinations, plus anything with fewer than two modifiers — a global ⌘K
      would override the frontmost app's own ⌘K everywhere
- [x] Visual feedback — the window comes forward and search opens. Someone who pressed a global
      shortcut is looking for something; making them then press ⌘K is a keystroke too many

#### Technical Notes
```swift
import Carbon.HIToolbox

class GlobalShortcutManager {
    private var eventHandler: EventHandlerRef?

    func register(shortcut: KeyboardShortcut) -> Bool {
        var hotKeyRef: EventHotKeyRef?
        var eventType = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))

        let hotKeyID = EventHotKeyID(signature: OSType(0x544F4D45), id: 1) // "TOME"

        let modifiers = carbonModifiers(from: shortcut.modifiers)
        let keyCode = carbonKeyCode(from: shortcut.key)

        let status = RegisterEventHotKey(
            UInt32(keyCode),
            UInt32(modifiers),
            hotKeyID,
            GetEventDispatcherTarget(),
            0,
            &hotKeyRef
        )

        if status != noErr {
            return false
        }

        // Install handler
        InstallEventHandler(
            GetEventDispatcherTarget(),
            { (_, event, _) -> OSStatus in
                NotificationCenter.default.post(name: .globalShortcutTriggered, object: nil)
                return noErr
            },
            1,
            &eventType,
            nil,
            &eventHandler
        )

        return true
    }
}
```

#### Success Metrics
- Shortcut works from any app
- Registration succeeds
- No conflicts detected

---

### P5-010: macOS notarization setup — DEFERRED

> **Not scheduled.** Requires Apple Developer Program membership, which is deferred by
> [ADR-0006](../decisions/0006-unsigned-distribution.md). Everything below is kept intact and
> correct so that enabling it later is a credentials change, not a redesign: enrol, add four
> secrets, add `notarytool submit` + `stapler staple` to the release workflow. Nothing about the
> app, the bundle identifier, or the entitlements changes.
>
> **Revisit at v1.0** — the natural trigger, and the point at which the Gatekeeper friction starts
> costing real adoption.

**Priority:** Deferred
**Complexity:** M (3-5 days)
**Dependencies:** All previous phases
**Blocks:** P5-011

#### Description
Set up code signing and notarization for macOS distribution.

#### Acceptance Criteria
- [ ] Apple Developer account configured
- [ ] Code signing identity set up
- [ ] Hardened runtime enabled
- [ ] Entitlements configured
- [ ] Notarization workflow automated
- [ ] Stapling automated
- [ ] Gatekeeper passes

#### Technical Notes
```toml
# tauri.conf.json
{
  "tauri": {
    "bundle": {
      "macOS": {
        "signingIdentity": "Developer ID Application: Your Name (TEAMID)",
        "entitlements": "./entitlements.plist",
        "hardened_runtime": true,
        "providerShortName": "TEAMID"
      }
    }
  }
}
```

**This is the authoritative entitlements file.** A second, different one previously existed in
`12-security-considerations.md`; that document now links here.

```xml
<!-- entitlements.plist -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- JavaScriptCore in WKWebView needs JIT. -->
    <key>com.apple.security.cs.allow-jit</key>
    <true/>

    <!-- Fetching documentation. -->
    <key>com.apple.security.network.client</key>
    <true/>

    <!-- User-chosen local documentation directories (source type: local). -->
    <key>com.apple.security.files.user-selected.read-write</key>
    <true/>

    <!-- iCloud Drive container for bookmark sync (Phase 3). -->
    <key>com.apple.developer.ubiquity-container-identifiers</key>
    <array>
        <string>iCloud.$(BUNDLE_ID)</string>
    </array>

    <!-- Keychain access for the API token. -->
    <key>keychain-access-groups</key>
    <array>
        <string>$(AppIdentifierPrefix)$(BUNDLE_ID)</string>
    </array>

    <!-- DELIBERATELY ABSENT, and why:

         com.apple.security.app-sandbox
           App Sandbox is incompatible with this design. The CLI installed by
           Homebrew is not sandboxed; the app would be, and its data directory
           would be redirected into a container. The two would see different
           libraries. Sandboxing is only mandatory for Mac App Store
           distribution, and Tome ships via DMG and Homebrew.
           See PRD "File System Layout".

         com.apple.security.cs.allow-unsigned-executable-memory
         com.apple.security.cs.disable-library-validation
           Both were in the original file. Both materially weaken the hardened
           runtime -- the second permits loading unsigned third-party libraries
           into the process -- and neither is required by a Rust + Tauri app.
           Adding hardened-runtime exceptions "just in case" is how an app ends
           up with a weaker security posture than an Electron app.

         com.apple.security.network.server
           The local API binds loopback only; no inbound entitlement is needed.
    -->
</dict>
</plist>
```

> **Note the contradiction this resolves.** `09-non-functional-requirements.md` asserted
> "Sandboxed: App Sandbox enabled" while this file never requested the sandbox entitlement and
> every path in the plan was `~/.tome`, which a sandboxed app cannot write. The NFR document has
> been corrected.

**CI Notarization Script:**
```bash
#!/bin/bash
set -euo pipefail

APP_PATH="$1"
: "${BUNDLE_ID:?set BUNDLE_ID (DEC-002)}"
: "${SIGNING_IDENTITY:?set SIGNING_IDENTITY}"

# Sign inner binaries first, then the bundle. Do NOT use `codesign --deep`:
# Apple documents it as unsuitable for signing (it cannot apply per-binary
# entitlements, and it silently re-signs nested code with the outer
# entitlements). Tauri already signs during bundling; this script exists for
# the sidecar CLI and any extra helpers.
find "$APP_PATH/Contents/MacOS" -type f -perm +111 | while read -r bin; do
  codesign --force --timestamp --options runtime \
    --sign "$SIGNING_IDENTITY" "$bin"
done

codesign --force --timestamp --verify --verbose \
  --sign "$SIGNING_IDENTITY" \
  --options runtime \
  --entitlements entitlements.plist \
  "$APP_PATH"

# Create zip for notarization
ditto -c -k --keepParent "$APP_PATH" tome.zip

# Submit for notarization
xcrun notarytool submit tome.zip \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_APP_PASSWORD" \
  --team-id "$TEAM_ID" \
  --wait

# Staple the ticket
xcrun stapler staple "$APP_PATH"

# Verify
spctl --assess --verbose "$APP_PATH"
```

#### Success Metrics
- App signed correctly
- Notarization succeeds
- Gatekeeper approves

---

### P5-011: Build DMG installer

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P5-010
**Blocks:** P5-012

#### Description
Create a professional DMG installer for distribution.

> **Built by S4-9/S4-8, 2026-07-30, with two corrections.** There is no `appdmg` and no
> `create-dmg`: Tauri's own bundler produces the DMG from `bundle.macOS.dmg` in
> `tauri.conf.json`, so the layout is configuration rather than a second tool with a second
> config file to drift. And the DMG is **not signed** — ADR-0006 defers the Apple Developer
> Program, which also deletes the `notarytool` and `stapler` steps sketched below.
>
> The one thing this ticket did not anticipate: **the DMG has to contain the CLI**, because
> the cask symlinks it out of the bundle. See S4-9.

#### Acceptance Criteria
- [x] DMG with custom background — `scripts/make-dmg-background.mjs`, generated from
      `public/tokens.css` so the installer cannot drift from the app's palette
- [x] Drag-to-Applications layout — `appPosition` / `applicationFolderPosition`, verified by
      mounting the built DMG and listing the volume
- [x] Correct volume icon — `.VolumeIcon.icns`, from the bundle icon set
- [~] License agreement — skipped. MIT OR Apache-2.0 needs no click-through, and a licence panel
      is one more thing between a user and the app
- [x] Compressed DMG — the bundler's default (UDZO)
- [~] Signed DMG — **deferred by ADR-0006.** `spctl` rejects the result; that is expected and the
      cask's caveats carry the fix
- [x] Automated build — `.github/workflows/release.yml`, on tag
- [x] **The DMG ships `Tome.app/Contents/MacOS/tome`** — asserted by `scripts/verify-bundle.sh`

#### Technical Notes
```bash
# Using create-dmg or appdmg

# appdmg config (appdmg.json)
{
  "title": "Tome",
  "icon": "assets/dmg-icon.icns",
  "background": "assets/dmg-background.png",
  "icon-size": 80,
  "window": {
    "size": {
      "width": 540,
      "height": 380
    }
  },
  "contents": [
    { "x": 140, "y": 200, "type": "file", "path": "Tome.app" },
    { "x": 400, "y": 200, "type": "link", "path": "/Applications" }
  ],
  "code-sign": {
    "signing-identity": "Developer ID Application: Your Name (TEAMID)"
  }
}

# Build command
appdmg appdmg.json Tome-1.0.0.dmg

# Notarize DMG
xcrun notarytool submit Tome-1.0.0.dmg ...
xcrun stapler staple Tome-1.0.0.dmg
```

#### Success Metrics
- DMG opens cleanly
- Drag-to-install works
- Gatekeeper approves

---

### P5-012: Publish distribution channels (own tap first)

**Priority:** High
**Complexity:** S (1-2 days)
**Dependencies:** P5-011
**Blocks:** None

#### Description
Make Tome installable via Homebrew.

> **Priority lowered from Critical, and the plan changed.** The original ticket assumed submission
> to `homebrew-cask` at launch. **homebrew-cask has notability requirements** — a brand-new project
> with no stars, forks, or watchers is rejected, and "Critical" priority on an action that a
> third party will decline is a scheduling trap. Ship an **own tap** on day one
> (`brew install --cask alexnodeland/tap/tome`), and submit to homebrew-cask later once the
> project clears the thresholds. Nothing about the user experience changes materially.

#### Acceptance Criteria
- [x] Cask authored at `packaging/homebrew/Casks/tome.rb`, this repository's source of truth,
      mirrored into the tap by the release workflow. **The tap repository itself must exist
      before the first tag** — that is the owner's, not a build step
- [x] SHA256 verification against the released DMG — real `version` and `sha256`, rewritten from
      the tag and the built artifact. Deliberately not `:latest` / `:no_check`: Tome is unsigned,
      so the checksum is the only integrity check a user gets
- [x] Zap stanza that matches the **actual** data locations. Every path was observed on a machine
      that had run Tome, and `scripts/verify-bundle.sh` re-derives the two that matter from
      `tome status --json`, so the list cannot rot when a path moves. The iCloud container is
      absent on purpose: sync does not exist, so no version has ever created it
- [x] Caveats explaining first run and the CLI — leading with `xattr -dr com.apple.quarantine`,
      because macOS 15 removed the Control-click bypass
- [x] `livecheck` for upgrade detection
- [x] Release automation updates the tap on tag — and warns rather than failing a
      already-published release when `HOMEBREW_TAP_TOKEN` is absent
- [x] **`brew style` actually runs** — `scripts/check-cask.sh` stages a throwaway tap, because
      Homebrew refuses to lint a cask outside one. The CHANGELOG claimed this for two stages
      while the file it named did not exist
- [x] **`tome config forget-token`** — `--zap` removes files, and the Keychain is not a file, so
      without this the one secret Tome creates survives the uninstall
- [ ] Submission to homebrew-cask tracked as a **post-launch** follow-up, gated on notability

#### Technical Notes
```ruby
# Casks/tome.rb
cask "tome" do
  version "1.0.0"
  sha256 "abc123..." # SHA256 of DMG

  url "https://github.com/yourname/tome/releases/download/v#{version}/Tome-#{version}.dmg"
  name "Tome"
  desc "Personal library for technical documentation"
  homepage "https://tome.dev"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :monterey"

  app "Tome.app"

  # Must match PRD "File System Layout" exactly. Anything missing here is data
  # left behind after `--zap`; anything wrong is a path that never existed.
  zap trash: [
    "~/Library/Application Support/Tome",
    "~/Library/Caches/Tome",
    "~/Library/Preferences/#{BUNDLE_ID}.plist",
    "~/Library/Mobile Documents/iCloud~#{BUNDLE_ID.tr('.', '~')}",
  ]

  caveats <<~EOS
    Tome stores its library in ~/Library/Application Support/Tome
    and cached documentation in ~/Library/Caches/Tome

    To get started, run Tome and add your first documentation source,
    or use the CLI: tome add https://docs.python.org/3/
  EOS
end
```

**Installation:**
```bash
brew install --cask tome
```

#### Success Metrics
- `brew install --cask tome` works
- Upgrade works
- Uninstall cleans up

---

### P5-013: Write user documentation

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** All previous phases
**Blocks:** None

#### Description
Create comprehensive user documentation.

#### Acceptance Criteria
- [ ] Getting started guide
- [ ] Adding documentation sources
- [ ] Search usage
- [ ] Bookmarks and annotations
- [ ] Sync setup
- [ ] CLI reference
- [ ] Keyboard shortcuts
- [ ] Troubleshooting guide
- [ ] FAQ

#### Technical Notes
```markdown
# Tome Documentation

## Getting Started
1. Download Tome from [tome.dev](https://tome.dev) or `brew install --cask tome`
2. Launch Tome
3. Add your first documentation source

## Adding Documentation
### From the registry (recommended)
1. Press `Cmd+N` or click "Add Source"
2. Search the built-in registry and pick a source
3. Tome installs a tested configuration and starts the first sync

### From an arbitrary URL
1. Press `Cmd+N` or click "Add Source"
2. Enter the documentation URL
3. Tome detects the platform and proposes a configuration for you to confirm

### Supported Platforms
- ReadTheDocs / Sphinx
- rustdoc (Rust documentation)
- mdBook
- Man pages
- Any website (generic scraper)

## Searching
- Global search: Cmd+K
- Search in page: Cmd+F
- Scoped search: Select source first, then search

## Bookmarks
- Bookmark page: Cmd+D
- View bookmarks: Cmd+B
- Add to collection: Right-click bookmark

## Sync
Tome syncs bookmarks and reading positions via iCloud.
Enable in Preferences > Sync.

## CLI
```bash
tome add <url>      # Add documentation
tome search <query> # Search
tome list           # List sources
tome pull           # Update documentation
```

## Keyboard Shortcuts
| Action | Shortcut |
|--------|----------|
| Global search | Cmd+K |
| Search in page | Cmd+F |
| Bookmark | Cmd+D |
| Back | Cmd+[ |
| Forward | Cmd+] |

Full list: PRD Appendix C. Do not restate it here -- four copies previously
existed and had already drifted apart.
```

#### Success Metrics
- All features documented
- Screenshots current
- No broken links

---

### P5-014: Create landing page

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** All previous phases
**Blocks:** None

#### Description
Create a marketing landing page for Tome.

#### Acceptance Criteria
- [ ] Clear value proposition
- [ ] Feature highlights
- [ ] Screenshots/demo video
- [ ] Download buttons
- [ ] Documentation link
- [ ] GitHub link
- [ ] Mobile responsive
- [ ] Fast loading

#### Technical Notes
```html
<!DOCTYPE html>
<html>
<head>
  <title>Tome - Your Personal Documentation Library</title>
  <meta name="description" content="Unified, offline access to technical documentation. Beautiful, fast, and yours.">
</head>
<body>
  <header>
    <nav>
      <a href="/" class="logo">Tome</a>
      <a href="/docs">Docs</a>
      <a href="https://github.com/yourname/tome">GitHub</a>
      <a href="/download" class="cta">Download</a>
    </nav>
  </header>

  <main>
    <section class="hero">
      <h1>Your Personal Documentation Library</h1>
      <p>Unified, offline access to technical documentation. Beautiful, fast, and entirely yours.</p>
      <div class="cta-buttons">
        <a href="/download" class="primary">Download for macOS</a>
        <code>brew install --cask tome</code>
      </div>
    </section>

    <section class="features">
      <div class="feature">
        <h3>All Your Docs in One Place</h3>
        <p>ReadTheDocs, rustdoc, man pages, and more. Unified search across everything.</p>
      </div>
      <div class="feature">
        <h3>Beautiful Typography</h3>
        <p>Documentation should read like a well-set book, not a raw HTML dump.</p>
      </div>
      <div class="feature">
        <h3>Works Offline</h3>
        <p>Your docs, your machine. No internet required once synced.</p>
      </div>
      <div class="feature">
        <h3>AI-Ready</h3>
        <p>MCP server and Claude Code plugin for seamless AI integration.</p>
      </div>
    </section>

    <section class="screenshot">
      <img src="/screenshot.png" alt="Tome screenshot" />
    </section>
  </main>

  <footer>
    <p>&copy; 2026 Tome. Open source — see LICENSE.
       <!-- DEC-001: licence not yet chosen. Do not assert a licence in
            marketing copy before the LICENSE file exists. --></p>
  </footer>
</body>
</html>
```

#### Success Metrics
- Page loads < 2s
- Clear call to action
- Mobile friendly

---

## Phase 5 Dependency Graph

```
All Previous Phases
        │
        ├──── P5-001 (Performance) ────┬──── P5-002 (Lazy Loading)
        │                              │
        │                              └──── P5-003 (Incremental Index)
        │
        ├──── P5-004 (Error Audit) ──── P5-005 (Recovery)
        │
        ├──── P5-006 (Onboarding)
        │         │
        │         ├── P1-022 (Manual Add)
        │         └── P2-014 (Auto-detect)
        │
        ├──── P5-007 (Preferences)
        │         │
        │         └── P1-017 (Layout)
        │
        ├──── P5-008 (Menu Bar) ──── P5-009 (Global Shortcut)
        │
        ├──── P5-010 (Notarization) ──── P5-011 (DMG) ──── P5-012 (Homebrew)
        │
        ├──── P5-013 (Documentation)
        │
        └──── P5-014 (Landing Page)
```

---

## Exit Criteria Checklist

- [ ] Startup time < 500ms *(benchmark, not stopwatch)*
- [ ] Memory usage < 200MB idle *(instrumented launch test)*
- [ ] Search latency < 100ms P95 *(P2-018 benchmark)*
- [ ] Relevance eval (P2-019) and detection eval (P2-020) both at or above target
- [ ] No known critical bugs
- [ ] **Offline verification passes**: full session with networking disabled, including images
- [ ] Error handling comprehensive
- [ ] Onboarding flow complete
- [ ] Preferences UI functional
- [ ] Menu bar integration working
- [ ] Global shortcut functional
- [ ] DMG installer created (unsigned)
- [ ] `brew install --cask alexnodeland/tap/tome` works on a clean machine
- [ ] The documented quarantine step is verified on a machine that never built Tome
- [ ] `brew uninstall --cask --zap tome` leaves nothing behind
- [ ] ~~App signed and notarized~~ — deferred, ADR-0006
- [ ] User documentation complete
- [ ] Landing page live

---

## Launch Checklist

### Pre-Launch (1 week before)
- [ ] Final performance testing
- [ ] Security review against `12-security-considerations.md`, specifically: API auth cannot be
      bypassed, SSRF filter rejects the full test vector list, sanitizer blocks the XSS corpus,
      no secrets in logs or diagnostics bundles
- [ ] `LICENSE` file present and matching what the landing page claims (DEC-001)
- [ ] Bundle identifier consistent across app, Keychain, iCloud container, and cask zap (DEC-002)
- [ ] Third-party licence attributions generated and shipped
- [ ] `SECURITY.md` published with a reporting address that someone reads
- [ ] Beta tester feedback incorporated
- [ ] Documentation reviewed; every command in it exists
- [ ] Landing page ready
- [ ] Social media prepared

### Launch Day
- [ ] GitHub release created
- [ ] DMG uploaded
- [ ] Homebrew cask PR submitted
- [ ] Landing page deployed
- [ ] Announcement posted
- [ ] Monitoring enabled

### Post-Launch (1 week after)
- [ ] Monitor crash reports
- [ ] Respond to issues
- [ ] Collect user feedback
- [ ] Plan v1.0.1 hotfix if needed
