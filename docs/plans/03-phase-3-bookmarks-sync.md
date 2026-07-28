# Phase 3: Bookmarks & Sync (v0.3)

**Goal:** Personal layer and cross-device sync
**Tickets:** 15
**Effort:** ~68 person-days
**Prerequisites:** Phase 2 complete (can run parallel with Phase 4)
**Exit Criteria:** Bookmarks sync reliably between two Macs via iCloud

> **This phase is the first candidate to cut (DEC-004).** It is 68 person-days, it contains the
> highest-scoring risk in the register (RISK-002), and cutting it costs the least: bookmarks and
> annotations still work perfectly on a single machine. If the project is solo, ship v1.0 without
> sync and add it once there is evidence people want it.

> **Sync mechanism changed: iCloud Drive container, not CloudKit.** See
> [PRD § iCloud Sync Architecture](../PRD.md#icloud-sync-architecture) for the reasoning. In short:
> CloudKit is Swift-only, the core is Rust, and the CLI — which runs outside the app — must sync
> too. The file-based approach is the contingency the risk register already recorded for RISK-002;
> it is a better primary. P3-010, P3-011 and P3-012 below are rewritten accordingly.

---

## Ticket Summary

| ID | Title | Complexity | Priority | Dependencies |
|----|-------|------------|----------|--------------|
| P3-001 | Design bookmark data model | M | Critical | P1-004 |
| P3-002 | Implement bookmark CRUD operations | M | Critical | P3-001 |
| P3-003 | Build bookmark UI component | M | High | P3-002 |
| P3-004 | Add bookmark keyboard shortcut (Cmd+D) | S | High | P3-002, P3-003 |
| P3-005 | Implement bookmark collections | M | Critical | P3-001, P3-002 |
| P3-006 | Create collection management UI | M | High | P3-005 |
| P3-007 | Build text highlighting system | L | High | P1-016 |
| P3-008 | Add annotation/notes to highlights | M | Critical | P3-007 |
| P3-009 | Implement reading position memory | M | High | P1-016, P1-004 |
| P3-010 | Design file-based iCloud sync architecture | L | Critical | P3-001 |
| P3-011 | Implement operation log and codec | M | High | P3-010 |
| P3-012 | Build sync engine core | L | Critical | P3-010, P3-011 |
| P3-013 | Add conflict resolution handling | M | High | P3-012 |
| P3-014 | Create sync status UI | M | High | P3-012 |
| P3-015 | Sync limits, backpressure and status surface | S | Medium | P3-012 |

---

## Detailed Tickets

### P3-001: Design bookmark data model

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P1-004 (SQLite)
**Blocks:** P3-002, P3-005, P3-010

#### Description
Design the data model for bookmarks, highlights, and annotations.

#### Acceptance Criteria
- [ ] Bookmark entity with source, page, title, position
- [ ] Highlight ranges as character offsets
- [ ] Notes attached to highlights or bookmarks
- [ ] Collection membership (many-to-many)
- [ ] Sync metadata (modified_at, sync_status, device_id)
- [ ] SQLite schema migrations
- [ ] Indexes for common queries

#### Technical Notes
```sql
CREATE TABLE bookmarks (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    page_path TEXT NOT NULL,
    title TEXT NOT NULL,
    note TEXT,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    -- Lamport counter for deterministic conflict resolution across devices.
    lamport INTEGER NOT NULL DEFAULT 0,
    -- Which device wrote last. METADATA ONLY -- never part of identity.
    last_writer TEXT NOT NULL,
    deleted_at TEXT,                 -- tombstone; retained 90 days, then vacuumed
    UNIQUE(source_id, page_path)     -- one bookmark per page, per library
);

-- Annotations are independent of bookmarks: you can highlight a page you never
-- bookmarked, and a bookmarked page can carry many annotations. The original
-- schema made every highlight a child of a bookmark, which forced a phantom
-- bookmark to exist for any highlight and cascade-deleted annotations when a
-- user removed a bookmark.
CREATE TABLE annotations (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    page_path TEXT NOT NULL,
    -- Robust anchoring (W3C Web Annotation selectors). See PRD "Annotation anchoring".
    quote TEXT NOT NULL,             -- the exact highlighted text
    prefix TEXT NOT NULL,            -- ~32 chars before
    suffix TEXT NOT NULL,            -- ~32 chars after
    hint_start INTEGER,              -- position hint only, NOT the source of truth
    hint_end INTEGER,
    anchor_state TEXT NOT NULL DEFAULT 'exact',  -- exact | approximate | orphaned
    color TEXT DEFAULT 'yellow',
    note TEXT,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    lamport INTEGER NOT NULL DEFAULT 0,
    last_writer TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX idx_annotations_page ON annotations(source_id, page_path);
CREATE INDEX idx_annotations_state ON annotations(anchor_state) WHERE anchor_state != 'exact';

CREATE TABLE collections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    icon TEXT,
    color TEXT,
    sort_order INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL
);

CREATE TABLE bookmark_collections (
    bookmark_id TEXT REFERENCES bookmarks(id) ON DELETE CASCADE,
    collection_id TEXT REFERENCES collections(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL,
    PRIMARY KEY (bookmark_id, collection_id)
);

CREATE INDEX idx_bookmarks_source ON bookmarks(source_id);
CREATE INDEX idx_bookmarks_live ON bookmarks(source_id, page_path) WHERE deleted_at IS NULL;

-- Sync state lives in its own table rather than a `sync_status` column, because a
-- column cannot express "never synced" vs "pending" vs "in flight" vs "conflicted"
-- vs "sync disabled", and because it must be cheap to clear without rewriting rows.
CREATE TABLE sync_state (
    entity_type TEXT NOT NULL,       -- bookmark | annotation | collection | position
    entity_id   TEXT NOT NULL,
    state       TEXT NOT NULL,       -- pending | in_flight | synced | conflicted
    last_error  TEXT,
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (entity_type, entity_id)
);
CREATE INDEX idx_sync_pending ON sync_state(state) WHERE state != 'synced';
```

#### Why `device_id` left the uniqueness constraint

The original key was `UNIQUE(source_id, page_path, device_id)`. That means the *same page*
bookmarked on a laptop and a desktop produces **two rows**, and no amount of merge logic downstream
can collapse them, because by definition they are distinct records. Sync would faithfully replicate
both and the user would see every bookmark duplicated once per machine they own. A sync key must
identify the thing, not the writer.

```rust
#[derive(Debug, Clone)]
pub struct Bookmark {
    pub id: Uuid,
    pub source_id: Uuid,
    pub page_path: String,
    pub title: String,
    pub scroll_position: Option<f32>,
    pub highlights: Vec<Highlight>,
    pub collections: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub sync_status: SyncStatus,
    pub device_id: String,
}

#[derive(Debug, Clone)]
pub struct Highlight {
    pub id: Uuid,
    pub start_offset: u32,
    pub end_offset: u32,
    pub color: HighlightColor,
    pub note: Option<String>,
}
```

#### Success Metrics
- Schema supports all bookmark features
- Efficient queries for common operations
- Migration path clear

---

### P3-002: Implement bookmark CRUD operations

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P3-001
**Blocks:** P3-003, P3-004, P3-005

#### Description
Build the data access layer for bookmark operations.

#### Acceptance Criteria
- [ ] Create bookmark (with optional highlights)
- [ ] Read bookmarks (all, by source, by collection)
- [ ] Update bookmark (title, position, notes)
- [ ] Delete bookmark (with cascade)
- [ ] Add/remove from collections
- [ ] Batch operations for sync
- [ ] Optimistic updates with rollback

#### Technical Notes
```rust
pub struct BookmarkRepository {
    db: SqlitePool,
}

impl BookmarkRepository {
    pub async fn create(&self, bookmark: &Bookmark) -> Result<()> {
        // Insert bookmark and highlights in transaction
    }

    pub async fn get(&self, id: &Uuid) -> Result<Option<Bookmark>> {
        // Fetch with highlights
    }

    pub async fn list_by_source(&self, source_id: &Uuid) -> Result<Vec<Bookmark>> {
        // List all bookmarks for a source
    }

    pub async fn list_by_collection(&self, collection_id: &Uuid) -> Result<Vec<Bookmark>> {
        // List all bookmarks in a collection
    }

    pub async fn update(&self, bookmark: &Bookmark) -> Result<()> {
        // Update with modified_at timestamp
    }

    pub async fn delete(&self, id: &Uuid) -> Result<()> {
        // Cascade delete highlights
    }

    pub async fn add_to_collection(&self, bookmark_id: &Uuid, collection_id: &Uuid) -> Result<()>;
    pub async fn remove_from_collection(&self, bookmark_id: &Uuid, collection_id: &Uuid) -> Result<()>;
}
```

#### Success Metrics
- CRUD operations < 10ms each
- Transaction safety verified
- No orphaned records

---

### P3-003: Build bookmark UI component

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P3-002
**Blocks:** P3-004

#### Description
Create the UI for viewing and managing bookmarks.

#### Acceptance Criteria
- [ ] Bookmark indicator in reader toolbar
- [ ] Bookmark panel/sidebar (Cmd+B)
- [ ] List view with source grouping
- [ ] Quick search/filter bookmarks
- [ ] Click to navigate to bookmarked page
- [ ] Edit bookmark title
- [ ] Delete bookmark with confirmation
- [ ] Show highlight count per bookmark

#### Technical Notes
```svelte
<script>
  import { bookmarks } from '$lib/stores/bookmarks';
  import { currentPage } from '$lib/stores/navigation';

  $: isBookmarked = $bookmarks.some(
    b => b.source_id === $currentPage.sourceId && b.page_path === $currentPage.path
  );
</script>

<!-- Toolbar indicator -->
<button
  class="bookmark-button"
  class:active={isBookmarked}
  on:click={toggleBookmark}
  title={isBookmarked ? 'Remove bookmark' : 'Add bookmark'}
>
  <BookmarkIcon filled={isBookmarked} />
</button>

<!-- Bookmarks panel -->
<aside class="bookmarks-panel">
  <header>
    <h2>Bookmarks</h2>
    <input type="search" placeholder="Filter..." bind:value={filter} />
  </header>

  {#each groupedBookmarks as [source, items]}
    <section class="bookmark-group">
      <h3>{source}</h3>
      {#each items as bookmark}
        <BookmarkItem
          {bookmark}
          on:click={() => navigate(bookmark)}
          on:delete={() => deleteBookmark(bookmark.id)}
        />
      {/each}
    </section>
  {/each}
</aside>
```

#### Success Metrics
- Render 500 bookmarks in < 100ms
- Smooth scrolling in list
- Responsive to state changes

---

### P3-004: Add bookmark keyboard shortcut (Cmd+D)

**Priority:** High
**Complexity:** S (1-2 days)
**Dependencies:** P3-002, P3-003
**Blocks:** None

#### Description
Implement the keyboard shortcut to toggle bookmarks.

#### Acceptance Criteria
- [ ] Cmd+D toggles bookmark on current page
- [ ] Visual feedback on toggle (icon animation)
- [ ] Toast notification confirming action
- [ ] Works when focus is in reader
- [ ] No conflict with other shortcuts
- [ ] Option to add to collection immediately (hold)

#### Technical Notes
```typescript
function handleKeydown(e: KeyboardEvent) {
  if (e.metaKey && e.key === 'd') {
    e.preventDefault();
    toggleCurrentPageBookmark();
  }
}

async function toggleCurrentPageBookmark() {
  const { sourceId, path, title } = getCurrentPage();
  const existing = findBookmark(sourceId, path);

  if (existing) {
    await deleteBookmark(existing.id);
    showToast('Bookmark removed');
  } else {
    await createBookmark({ sourceId, path, title });
    showToast('Page bookmarked');
  }
}
```

#### Success Metrics
- Toggle latency < 100ms
- No missed keystrokes
- Clear visual feedback

---

### P3-005: Implement bookmark collections

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P3-001, P3-002
**Blocks:** P3-006

#### Description
Add ability to organize bookmarks into collections.

#### Acceptance Criteria
- [ ] Create/rename/delete collections
- [ ] Assign icon and color to collection
- [ ] Add bookmark to multiple collections
- [ ] Remove bookmark from collection
- [ ] Reorder collections (drag or sort order)
- [ ] Default "All Bookmarks" view
- [ ] Collection count displayed

#### Technical Notes
```rust
pub struct CollectionRepository {
    db: SqlitePool,
}

impl CollectionRepository {
    pub async fn create(&self, collection: &Collection) -> Result<()>;
    pub async fn list(&self) -> Result<Vec<Collection>>;
    pub async fn update(&self, collection: &Collection) -> Result<()>;
    pub async fn delete(&self, id: &Uuid) -> Result<()>;
    pub async fn reorder(&self, ids: &[Uuid]) -> Result<()>;
    pub async fn get_bookmark_count(&self, id: &Uuid) -> Result<u32>;
}
```

```typescript
interface Collection {
  id: string;
  name: string;
  icon: string; // emoji or icon name
  color: string; // hex color
  sortOrder: number;
  bookmarkCount: number;
}
```

#### Success Metrics
- Collection operations < 20ms
- Proper cascade on delete
- Reorder persists correctly

---

### P3-006: Create collection management UI

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P3-005
**Blocks:** None

#### Description
Build the UI for creating and managing collections.

#### Acceptance Criteria
- [ ] Collection list in bookmarks sidebar
- [ ] Create new collection button
- [ ] Edit collection (name, icon, color)
- [ ] Delete collection (with confirmation)
- [ ] Drag bookmarks into collections
- [ ] Context menu for quick actions
- [ ] Empty collection state

#### Technical Notes
```svelte
<script>
  import { collections } from '$lib/stores/collections';

  let editingId: string | null = null;
  let newCollectionMode = false;
</script>

<div class="collections-sidebar">
  <button on:click={() => newCollectionMode = true}>
    + New Collection
  </button>

  {#if newCollectionMode}
    <CollectionEditor
      on:save={createCollection}
      on:cancel={() => newCollectionMode = false}
    />
  {/if}

  <ul class="collection-list">
    <li class="collection-item all">
      <span>All Bookmarks</span>
      <span class="count">{totalBookmarks}</span>
    </li>

    {#each $collections as collection}
      <li
        class="collection-item"
        on:drop={(e) => addToCollection(e, collection.id)}
        on:dragover={allowDrop}
      >
        {#if editingId === collection.id}
          <CollectionEditor
            {collection}
            on:save={(e) => updateCollection(e.detail)}
            on:cancel={() => editingId = null}
          />
        {:else}
          <span class="icon">{collection.icon}</span>
          <span class="name">{collection.name}</span>
          <span class="count">{collection.bookmarkCount}</span>
          <button on:click={() => editingId = collection.id}>✏️</button>
        {/if}
      </li>
    {/each}
  </ul>
</div>
```

#### Success Metrics
- Drag-and-drop works smoothly
- Inline editing responsive
- Visual hierarchy clear

---

### P3-007: Build text highlighting system

**Priority:** High
**Complexity:** L (1-2 weeks)
**Dependencies:** P1-016 (WebView)
**Blocks:** P3-008

#### Description
Enable users to highlight text passages in documentation.

#### Acceptance Criteria
- [ ] Select text and highlight via menu or keyboard (`Cmd+Shift+H` — `Cmd+H` is Hide Application)
- [ ] Multiple highlight colors available
- [ ] Highlights persist using quote/prefix/suffix anchoring; offsets are a hint only
- [ ] Highlights render on page load
- [ ] Overlapping and adjacent highlights render correctly (nested `<mark>` handled)
- [ ] Click highlight to show actions (note, remove, copy)
- [ ] **Re-anchor pass runs when a page's content hash changes**; results classified
      `exact` / `approximate` / `orphaned`
- [ ] Orphaned highlights retain their quoted text and note, and appear in a "needs attention" view
- [ ] Highlight colors meet the contrast requirement against both light and dark reader backgrounds
      *and* against highlighted text — a yellow wash that makes text unreadable is not accessible

#### Technical Notes
```javascript
// WebView JavaScript for highlighting
class HighlightManager {
  private highlights: Map<string, HighlightData> = new Map();

  public highlight(color: string): HighlightData | null {
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed) return null;

    const range = selection.getRangeAt(0);
    const startOffset = this.getDocumentOffset(range.startContainer, range.startOffset);
    const endOffset = this.getDocumentOffset(range.endContainer, range.endOffset);

    const id = crypto.randomUUID();
    const data = { id, startOffset, endOffset, color };

    this.applyHighlight(range, id, color);
    this.highlights.set(id, data);

    selection.removeAllRanges();
    return data;
  }

  public restoreHighlights(highlights: HighlightData[]): void {
    for (const hl of highlights) {
      const range = this.offsetsToRange(hl.startOffset, hl.endOffset);
      if (range) {
        this.applyHighlight(range, hl.id, hl.color);
      }
    }
  }

  private applyHighlight(range: Range, id: string, color: string): void {
    // `range.surroundContents()` throws InvalidStateError whenever the range
    // partially selects a non-Text node -- which is the common case as soon as a
    // selection crosses a <code>, <em>, or line break. The original sample would
    // have failed on most real highlights.
    //
    // Walk the range's text nodes and wrap each one instead.
    for (const textNode of this.textNodesIn(range)) {
      const mark = document.createElement('mark');
      mark.dataset.highlightId = id;
      mark.className = `tome-hl tome-hl--${color}`;   // class, not inline style: the
                                                       // reader CSP forbids inline styles
      textNode.parentNode!.replaceChild(mark, textNode);
      mark.appendChild(textNode);
    }
  }
}
```

**Anchoring is not offsets.** Highlights are stored as quote + prefix + suffix with the offset kept
only as a lookup hint, and are re-anchored whenever a page's `content_hash` changes. See
[PRD § Annotation anchoring](../PRD.md#5-bookmarks--annotations). A highlight that cannot be
re-anchored becomes `orphaned` and is surfaced for the user to re-place — **never deleted.**

```rust
// Rust side
#[tauri::command]
async fn add_highlight(
    state: State<'_, AppState>,
    bookmark_id: String,
    start_offset: u32,
    end_offset: u32,
    color: String,
) -> Result<Highlight, Error> {
    // Store highlight in database
}
```

#### Success Metrics
- Highlight creation < 50ms
- Restoration accurate to character
- No DOM corruption

---

### P3-008: Add annotation/notes to highlights

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P3-007
**Blocks:** None

#### Description
Allow users to attach notes to highlights.

#### Acceptance Criteria
- [ ] Click highlight to open note editor
- [ ] Note saves automatically (debounced)
- [ ] Note indicator on highlighted text
- [ ] View all notes in sidebar
- [ ] Search notes content
- [ ] Markdown support in notes
- [ ] Export notes

#### Technical Notes
```svelte
<script>
  export let highlight: Highlight;
  export let onSave: (note: string) => void;

  let noteContent = highlight.note || '';
  let isEditing = false;

  const saveDebounced = debounce((content: string) => {
    onSave(content);
  }, 500);

  $: saveDebounced(noteContent);
</script>

<div class="highlight-popover">
  {#if isEditing}
    <textarea
      bind:value={noteContent}
      placeholder="Add a note..."
      on:blur={() => isEditing = false}
    />
  {:else}
    <div class="note-preview" on:click={() => isEditing = true}>
      {#if noteContent}
        {@html renderMarkdown(noteContent)}
      {:else}
        <span class="placeholder">Click to add note</span>
      {/if}
    </div>
  {/if}

  <div class="actions">
    <button on:click={copyText}>Copy</button>
    <button on:click={removeHighlight}>Remove</button>
  </div>
</div>
```

#### Success Metrics
- Note autosave < 1s after typing stops
- Markdown renders correctly
- Notes searchable

---

### P3-009: Implement reading position memory

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-016 (WebView), P1-004 (SQLite)
**Blocks:** None

#### Description
Remember and restore scroll position for each page.

#### Acceptance Criteria
- [ ] Track scroll position as user reads
- [ ] Store position (throttled, every 5s or on leave)
- [ ] Restore position when returning to page
- [ ] Per-source, per-page granularity
- [ ] Handle page content changes gracefully
- [ ] Optional "resume reading" prompt

#### Technical Notes
```typescript
// Track scroll position
let lastPosition = 0;

const throttledSave = throttle((position: number) => {
  invoke('save_reading_position', {
    sourceId: currentSource,
    pagePath: currentPath,
    position,
  });
}, 5000);

function onScroll(e: Event) {
  const position = getScrollPercentage();
  if (Math.abs(position - lastPosition) > 0.01) {
    lastPosition = position;
    throttledSave(position);
  }
}

// Restore on load
async function restorePosition(sourceId: string, path: string) {
  const position = await invoke('get_reading_position', { sourceId, path });
  if (position) {
    scrollToPercentage(position);
  }
}
```

```sql
CREATE TABLE reading_positions (
    source_id TEXT NOT NULL,
    page_path TEXT NOT NULL,
    scroll_position REAL NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (source_id, page_path)
);
```

#### Success Metrics
- Position accurate to within 1%
- Restore < 100ms
- No jank on scroll

---

### P3-010: Design file-based iCloud sync architecture

**Priority:** Critical
**Complexity:** L (1-2 weeks)
**Dependencies:** P3-001
**Blocks:** P3-011, P3-012

#### Description
Design sync over an iCloud Drive ubiquity container using per-device append-only operation logs.

**Why not CloudKit.** The original design used `CKRecord`s in a custom zone. Four problems, in
order of severity:

1. **CloudKit is Swift/Objective-C only.** The core engine is Rust. Adopting CloudKit means a Swift
   sync layer talking to a Rust database — the most state-heavy, hardest-to-debug component in the
   product, straddling a language boundary.
2. **The CLI and MCP server run outside the app process.** They must see the same bookmarks. A
   CloudKit-based engine living in the app cannot serve them without inventing an IPC protocol.
3. **RISK-002 scores CloudKit sync 16 (Critical)** on "undocumented edge cases, rate limits, and
   conflict scenarios" — and its recorded contingency is already "iCloud Drive file-based sync".
4. Change tokens, zone subscriptions, and `CKError` partial-failure handling are a large surface to
   learn for a single-user, low-write-volume workload.

The file container gives the same guarantee that actually matters — data appears on the user's
other Mac without a server — for a fraction of the surface.

#### Acceptance Criteria
- [ ] Container layout defined, with each device writing **only** its own subdirectory
- [ ] Operation log format defined and versioned
- [ ] Convergence rules defined for every field type (scalar / set / tombstone)
- [ ] Compaction policy defined (who may compact what, and when)
- [ ] Behaviour defined for: iCloud disabled, signed out, out of quota, container unavailable,
      file present but unreadable, partial download (iCloud evicts local copies)
- [ ] Migration path defined for a schema-version bump seen from a *newer* device
- [ ] Bounds documented: max ops before compaction, max container size, retention window

#### Technical Notes

```
Documents/
├── schema-version                 # single integer; a newer version is read-only, never guessed at
└── devices/
    ├── 5C1F…-a/                   # this device writes ONLY here
    │   ├── manifest.json          # device name, platform, last write, schema version
    │   └── ops-000017.jsonl       # append-only, one op per line
    └── 9B22…-b/                   # other devices: read-only to us
        ├── manifest.json
        └── ops-000004.jsonl
```

```jsonc
// one line of ops-*.jsonl
{
  "op": "upsert",                  // upsert | delete
  "entity": "bookmark",
  "id": "6f1c…",
  "lamport": 412,                  // monotonic per device, advanced past any value seen
  "wall": "2026-07-28T09:14:02Z",  // tiebreak only; clocks are not trusted for ordering
  "device": "5C1F…-a",
  "fields": { "title": "Vec in std::vec", "note": null }
}
```

**Why per-device directories.** Two devices never write the same file, so iCloud's own conflict
machinery is never invoked. This removes the single largest source of failure in naive file-based
sync — `.icloud` conflict copies appearing and silently diverging.

**Convergence rules:**

| Field kind | Rule | Rationale |
|---|---|---|
| Scalar (title, note, colour) | Highest `lamport`; tie → later `wall`; tie → higher `device` id | Deterministic on every device without coordination |
| Set (collection membership) | Add-wins | Losing a bookmark from a collection is worse than an extra membership |
| Delete | Tombstone, retained 90 days | A delete that arrives before the create it deletes must still win |
| Annotation anchor state | Recomputed locally, never synced | It depends on locally cached content, which differs per device |

**Reading is idempotent replay.** Local SQLite is a materialized view of the logs plus local ops.
This makes "resync from scratch" a supported, testable operation rather than a recovery hack.

**Failure posture.** Sync is never on the critical path of a user action. A bookmark is written to
SQLite and to the local op log; whether iCloud has propagated it is a background concern. If the
container is unavailable, everything continues working and ops queue on disk — there is no separate
offline queue to get out of step (which is why P3-015 shrinks to a status/limits ticket).

#### Success Metrics
- Design handles 10 000 ops without pathological replay cost
- Every convergence rule has a named test scenario before implementation starts
- Two devices applying the same op set in different orders reach byte-identical state

---

### P3-011: Implement operation log and codec

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P3-010
**Blocks:** P3-012

#### Description
Implement writing, reading, and compacting the per-device operation logs.

#### Acceptance Criteria
- [ ] Serialize/deserialize every synced entity to an op record
- [ ] Append is atomic and crash-safe (write to temp, fsync, rename)
- [ ] Reader tolerates a truncated final line — a crash mid-append must not poison the log
- [ ] **Unknown fields and unknown entity types are preserved and re-emitted, not dropped.** A newer
      device's data must survive a round-trip through an older one.
- [ ] Unknown `op` values are skipped with a warning rather than aborting replay
- [ ] Lamport clock persists across restarts and advances past the maximum value ever observed
- [ ] Compaction: replace this device's own logs with a state snapshot; never touch another
      device's directory
- [ ] Round-trip property test: arbitrary entity → op → entity is lossless

#### Technical Notes
```rust
#[derive(Serialize, Deserialize)]
pub struct Op {
    pub op: OpKind,
    pub entity: EntityType,
    pub id: Uuid,
    pub lamport: u64,
    pub wall: DateTime<Utc>,
    pub device: DeviceId,
    pub fields: serde_json::Map<String, Value>,  // open map: forward compatible by construction
}
```

Deliberately **not** typed as a fixed struct per entity. A closed enum of fields would drop data
written by a future version, which is the classic way file-based sync corrupts a user's library
after they upgrade one machine.

Deserialization must not use `unwrap()` / `as!`-style forced conversions anywhere. These records
arrive from another machine; treat them as untrusted input. The earlier CloudKit sample used
`record[...] as! String` and `UUID(uuidString:)!`, either of which turns one malformed record into
a crash loop on launch.

#### Success Metrics
- Fuzzed/truncated logs never panic
- 10 000 ops written in < 1 s, replayed in < 2 s

---

### P3-012: Build sync engine core

**Priority:** Critical
**Complexity:** L (1-2 weeks)
**Dependencies:** P3-010, P3-011
**Blocks:** P3-013, P3-014, P3-015

#### Description
Coordinate local state and the iCloud container: emit local ops, discover and replay remote logs,
materialize into SQLite.

#### Acceptance Criteria
- [ ] Every local mutation writes SQLite **and** appends an op, in one transaction
- [ ] Watch the container for changes (`NSMetadataQuery` semantics / file watching) with polling fallback
- [ ] **Request download of evicted files before reading.** iCloud may keep only a placeholder
      locally; reading it without downloading yields spurious "empty log".
- [ ] Replay remote logs incrementally: track the byte offset consumed per remote log file
- [ ] Materialize into SQLite idempotently — replaying the same op twice is a no-op
- [ ] Full rebuild from logs is a supported operation, not an error path
- [ ] Emit sync events for the UI
- [ ] Never block a user action on sync
- [ ] Sync is disabled cleanly: no container access, no errors, local-only operation

#### Technical Notes
```rust
pub struct SyncEngine { paths: Paths, db: SqlitePool, device: DeviceId, clock: LamportClock }

impl SyncEngine {
    /// Idempotent. Safe to call on a timer, on file-change notification, and at launch.
    pub async fn tick(&self) -> Result<SyncReport> {
        self.ensure_downloaded().await?;          // materialize evicted placeholders
        let remote = self.discover_remote_logs()?; // every dir except our own
        for log in remote {
            let ops = self.read_since(&log).await?;  // resumes from stored byte offset
            self.apply(ops).await?;                  // convergence rules from P3-010
        }
        self.maybe_compact_own_log().await?;
        Ok(report)
    }
}
```

#### Success Metrics
- Two-device convergence within 30 s on a normal connection
- Simulated 3-device, 1 000-op, randomized-order replay converges to identical state, 1 000 runs
- Zero lost writes under simulated crash at every await point

---
### P3-013: Add conflict resolution handling

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P3-012
**Blocks:** None

#### Description
Implement conflict detection and resolution strategies.

#### Acceptance Criteria
- [ ] Deterministic resolution: Lamport counter, then wall clock, then device id — so **every
      device independently reaches the same answer** without coordination
- [ ] Set fields (collection membership) merge add-wins
- [ ] Deletes are tombstones and win over concurrent edits within the retention window
- [ ] Never destroy user-authored text: if two devices edited the same note concurrently, keep both
      (the loser is appended under a "conflicted copy" marker) rather than discarding one
- [ ] Conflict log for debugging, capped and rotated
- [ ] No modal conflict dialogs in v1 — resolution is automatic
- [ ] Property test: applying any permutation of an op set yields identical final state

> Plain last-write-wins on a free-text note silently deletes something a person typed. Wall-clock
> ordering across machines is also unreliable — a device with a skewed clock wins every conflict.
> Both are why the Lamport counter is primary and why notes are preserved rather than overwritten.

#### Technical Notes
```rust
pub enum ConflictResolution {
    LastWriteWins,
    Merge,
    LocalWins,
    RemoteWins,
    Manual,
}

pub struct ConflictResolver {
    strategy: ConflictResolution,
}

impl ConflictResolver {
    pub fn resolve(&self, local: &Bookmark, remote: &Bookmark) -> Bookmark {
        match self.strategy {
            ConflictResolution::LastWriteWins => {
                if local.modified_at > remote.modified_at {
                    local.clone()
                } else {
                    remote.clone()
                }
            }
            ConflictResolution::Merge => {
                // Merge collections, keep newer values for other fields
                let mut merged = if local.modified_at > remote.modified_at {
                    local.clone()
                } else {
                    remote.clone()
                };

                // Union of collections
                let mut collections: HashSet<_> = local.collections.iter().cloned().collect();
                collections.extend(remote.collections.iter().cloned());
                merged.collections = collections.into_iter().collect();

                // Union of highlights (by offset)
                // ...

                merged
            }
            ConflictResolution::LocalWins  => local.clone(),
            ConflictResolution::RemoteWins => remote.clone(),
            ConflictResolution::Manual => {
                // v1 has no manual-resolution UI. Rather than panic, fall back to the
                // deterministic default and record the conflict for inspection.
                tracing::warn!(id = %local.id, "manual resolution unavailable; using merge");
                Self::merge(local, remote)
            }
        }
    }
}
```

> The original sample ended in `_ => unimplemented!()`, i.e. a panic reachable from remote data.
> Anything driven by another device's input must degrade, not abort.

#### Success Metrics
- No data loss in conflicts
- Merge produces sensible results
- Conflicts logged for audit

---

### P3-014: Create sync status UI

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P3-012
**Blocks:** None

#### Description
Build the UI to show sync status and progress.

#### Acceptance Criteria
- [ ] Sync indicator in toolbar (icon)
- [ ] Animated during active sync
- [ ] Show last sync time on hover
- [ ] Error state with retry option
- [ ] Click for detailed sync status
- [ ] Show pending changes count
- [ ] Manual sync trigger

#### Technical Notes
```svelte
<script>
  import { syncStatus } from '$lib/stores/sync';

  $: statusIcon = getStatusIcon($syncStatus.state);
  $: statusClass = $syncStatus.state;
</script>

<button
  class="sync-indicator {statusClass}"
  on:click={showSyncDetails}
  title={getSyncTitle($syncStatus)}
>
  <SyncIcon state={$syncStatus.state} />
</button>

<!-- Detailed popup -->
{#if showDetails}
  <div class="sync-details-popup">
    <h3>Sync Status</h3>

    <dl>
      <dt>Last synced</dt>
      <dd>{formatRelativeTime($syncStatus.lastSyncedAt)}</dd>

      <dt>Pending changes</dt>
      <dd>{$syncStatus.pendingCount}</dd>

      {#if $syncStatus.error}
        <dt>Error</dt>
        <dd class="error">{$syncStatus.error}</dd>
      {/if}
    </dl>

    <button on:click={triggerSync} disabled={$syncStatus.state === 'syncing'}>
      Sync Now
    </button>
  </div>
{/if}

<style>
  .sync-indicator.syncing {
    animation: spin 1s linear infinite;
  }
  .sync-indicator.error {
    color: var(--color-error);
  }
</style>
```

#### Success Metrics
- Status updates in real-time
- Animation smooth at 60fps
- Clear error messaging

---

### P3-015: Sync limits, backpressure and status surface

**Priority:** Medium
**Complexity:** S (1-2 days)
**Dependencies:** P3-012
**Blocks:** None

#### Description
Bound the growth of the operation log and surface sync state honestly.

**This ticket shrank from M to S because the separate offline queue is gone.** Under the file-based
design, the operation log *is* the queue: local mutations always append locally and are replayed
whenever the container is reachable. A second queue on top of an append-only log is a second source
of truth that can disagree with the first — the original design would have needed reconciliation
logic between `sync_queue` and the record state, which is exactly the kind of avoidable complexity
that causes lost writes.

#### Acceptance Criteria
- [ ] Log growth bounded: compact this device's log past a threshold (ops count or bytes)
- [ ] Warn when the container is unreachable for longer than a configurable window (default 7 days)
- [ ] Surface pending-op count, last successful replay, and per-device last-seen time
- [ ] Detect and report the states users actually hit: iCloud signed out, iCloud Drive disabled,
      out of quota, container not yet provisioned
- [ ] A stale device (not seen in 90 days) can be forgotten by the user, dropping its logs
- [ ] Refuse to grow unboundedly: past a hard cap, stop appending and raise a visible error rather
      than filling the user's disk

#### Success Metrics
- Log stays under the size cap across a 10 000-op soak
- Every failure state above renders a specific, actionable message — not "sync failed"

---

## Phase 3 Dependency Graph

```
P1-004 (SQLite)
    │
    ▼
P3-001 (Data Model) ──────────────────────────────┐
    │                                              │
    ├──── P3-002 (CRUD) ──────┬──── P3-003 (UI)   │
    │         │               │         │         │
    │         │               │         ▼         │
    │         ▼               │    P3-004 (Cmd+D) │
    │    P3-005 (Collections) │                   │
    │         │               │                   │
    │         ▼               │                   │
    │    P3-006 (Collection UI)                   │
    │                                              │
    └──── P3-010 (Sync Design, file-based) ◄──────┘
              │
              ├──── P3-011 (Op log + codec)
              │         │
              ▼         ▼
         P3-012 (Sync Engine) ─────┬──── P3-013 (Conflicts)
              │                    │
              ├──── P3-014 (Status UI)
              │
              └──── P3-015 (Offline Queue)


P1-016 (WebView)
    │
    ├──── P3-007 (Highlighting) ──── P3-008 (Notes)
    │
    └──── P3-009 (Reading Position)
              │
              ▼
         P1-004 (SQLite)
```

---

## Exit Criteria Checklist

- [ ] Bookmarks can be created, viewed, and deleted
- [ ] Cmd+D toggles bookmark on current page
- [ ] Collections organize bookmarks
- [ ] Text can be highlighted with multiple colors
- [ ] Notes can be attached to highlights
- [ ] **Highlights survive a re-sync that changes the page**, or are marked `orphaned` — verified by
      a test that edits fixture content between syncs
- [ ] Reading position remembered per page, restored by heading then percentage
- [ ] iCloud Drive container sync configured and working
- [ ] Bookmarks sync between two Macs; **the same page bookmarked on both produces one bookmark**
- [ ] Sync status visible in UI, with specific messages for signed-out / disabled / quota states
- [ ] Offline changes replay on reconnect
- [ ] **Convergence property test passes**: any permutation of a 1 000-op set across 3 simulated
      devices yields identical state
- [ ] **Zero lost writes** under simulated crash at each await point

> "Sync reliability > 99.5 %" was removed. There is no telemetry to measure it against, and the
> number is not meaningful without defining the unit of an "operation". Convergence and
> zero-loss under fault injection are testable; a percentage is not.
