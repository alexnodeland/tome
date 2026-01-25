# Phase 3: Bookmarks & Sync (v0.3)

**Goal:** Personal layer and cross-device sync
**Tickets:** 15
**Prerequisites:** Phase 2 complete (can run parallel with Phase 4)
**Exit Criteria:** Bookmarks sync reliably between two Macs via iCloud

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
| P3-010 | Design CloudKit sync architecture | L | Critical | P3-001 |
| P3-011 | Implement CKRecord models | M | High | P3-010 |
| P3-012 | Build sync engine core | L | Critical | P3-010, P3-011 |
| P3-013 | Add conflict resolution handling | M | High | P3-012 |
| P3-014 | Create sync status UI | M | High | P3-012 |
| P3-015 | Implement offline queue system | M | High | P3-012 |

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
    source_id TEXT NOT NULL REFERENCES sources(id),
    page_path TEXT NOT NULL,
    title TEXT NOT NULL,
    scroll_position REAL,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    sync_status TEXT DEFAULT 'pending', -- pending, synced, conflict
    device_id TEXT NOT NULL,
    UNIQUE(source_id, page_path, device_id)
);

CREATE TABLE highlights (
    id TEXT PRIMARY KEY,
    bookmark_id TEXT NOT NULL REFERENCES bookmarks(id) ON DELETE CASCADE,
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    color TEXT DEFAULT 'yellow',
    note TEXT,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL
);

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
CREATE INDEX idx_bookmarks_sync ON bookmarks(sync_status);
CREATE INDEX idx_highlights_bookmark ON highlights(bookmark_id);
```

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
- [ ] Select text and highlight via menu or keyboard
- [ ] Multiple highlight colors available
- [ ] Highlights persist (stored as character offsets)
- [ ] Highlights render on page load
- [ ] Click highlight to show actions (note, remove, copy)
- [ ] Handle content changes (best effort preservation)
- [ ] Keyboard shortcut for quick highlight

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
    const mark = document.createElement('mark');
    mark.dataset.highlightId = id;
    mark.style.backgroundColor = color;
    range.surroundContents(mark);
  }
}
```

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

### P3-010: Design CloudKit sync architecture

**Priority:** Critical
**Complexity:** L (1-2 weeks)
**Dependencies:** P3-001
**Blocks:** P3-011, P3-012

#### Description
Design the iCloud sync architecture using CloudKit.

#### Acceptance Criteria
- [ ] Define CKRecord types for all synced entities
- [ ] Determine zone structure (private database)
- [ ] Design sync protocol (push/pull, change tokens)
- [ ] Plan conflict resolution strategy
- [ ] Handle offline scenarios
- [ ] Design rate limiting approach
- [ ] Plan for backwards compatibility

#### Technical Notes
```swift
// CloudKit Record Types
struct SyncSchema {
    static let bookmarkType = "Bookmark"
    static let collectionType = "Collection"
    static let sourceMetaType = "SourceMeta"
    static let readingPositionType = "ReadingPosition"
}

// Sync Architecture
/*
┌─────────────────────────────────────────────┐
│                 Local SQLite                │
│  (bookmarks, collections, positions)        │
└─────────────────┬───────────────────────────┘
                  │
          ┌───────▼───────┐
          │  Sync Engine  │
          │  ───────────  │
          │  - Change     │
          │    detection  │
          │  - Conflict   │
          │    resolution │
          │  - Queue mgmt │
          └───────┬───────┘
                  │
┌─────────────────▼───────────────────────────┐
│            CloudKit Private DB              │
│  ┌─────────────────────────────────────┐    │
│  │        Custom Zone: "TomeData"      │    │
│  │  - Bookmarks                        │    │
│  │  - Collections                      │    │
│  │  - SourceMeta                       │    │
│  │  - ReadingPositions                 │    │
│  └─────────────────────────────────────┘    │
└─────────────────────────────────────────────┘
*/
```

**Sync Protocol:**
1. On app launch: fetch server changes since last change token
2. Apply remote changes to local database (with conflict detection)
3. Push local pending changes to server
4. Store new change token

#### Success Metrics
- Architecture handles 10,000+ records
- Conflict resolution clearly defined
- Offline-first guaranteed

---

### P3-011: Implement CKRecord models

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P3-010
**Blocks:** P3-012

#### Description
Create the Swift CloudKit record models and conversion logic.

#### Acceptance Criteria
- [ ] CKRecord serialization for all entity types
- [ ] CKRecord deserialization with validation
- [ ] Handle optional fields gracefully
- [ ] System fields (modifiedAt, createdBy) used correctly
- [ ] Asset handling for future features
- [ ] Type-safe field access

#### Technical Notes
```swift
extension Bookmark {
    static let recordType = "Bookmark"

    enum Field: String {
        case sourceId
        case pagePath
        case title
        case scrollPosition
        case highlightsJSON
        case collectionsJSON
        case deviceId
    }

    func toCKRecord() -> CKRecord {
        let recordID = CKRecord.ID(recordName: id.uuidString)
        let record = CKRecord(recordType: Self.recordType, recordID: recordID)

        record[Field.sourceId.rawValue] = sourceId.uuidString
        record[Field.pagePath.rawValue] = pagePath
        record[Field.title.rawValue] = title
        record[Field.scrollPosition.rawValue] = scrollPosition as NSNumber?
        record[Field.highlightsJSON.rawValue] = encodeHighlights(highlights)
        record[Field.collectionsJSON.rawValue] = encodeCollections(collections)
        record[Field.deviceId.rawValue] = deviceId

        return record
    }

    init(from record: CKRecord) throws {
        guard record.recordType == Self.recordType else {
            throw SyncError.invalidRecordType
        }

        self.id = UUID(uuidString: record.recordID.recordName)!
        self.sourceId = UUID(uuidString: record[Field.sourceId.rawValue] as! String)!
        self.pagePath = record[Field.pagePath.rawValue] as! String
        // ...
    }
}
```

#### Success Metrics
- Round-trip serialization lossless
- Handles all field types
- Error handling comprehensive

---

### P3-012: Build sync engine core

**Priority:** Critical
**Complexity:** L (1-2 weeks)
**Dependencies:** P3-010, P3-011
**Blocks:** P3-013, P3-014, P3-015

#### Description
Implement the core sync engine that coordinates local and cloud data.

#### Acceptance Criteria
- [ ] Fetch remote changes (CKFetchRecordZoneChangesOperation)
- [ ] Push local changes (CKModifyRecordsOperation)
- [ ] Track sync state (change tokens per zone)
- [ ] Handle partial failures
- [ ] Retry failed operations with backoff
- [ ] Merge remote changes into local database
- [ ] Emit sync events for UI updates

#### Technical Notes
```swift
class SyncEngine {
    private let container: CKContainer
    private let database: CKDatabase
    private var changeTokens: [CKRecordZone.ID: CKServerChangeToken] = [:]

    func sync() async throws {
        // 1. Fetch remote changes
        let remoteChanges = try await fetchRemoteChanges()

        // 2. Apply remote changes locally
        let conflicts = try await applyRemoteChanges(remoteChanges)

        // 3. Resolve conflicts
        let resolved = try await resolveConflicts(conflicts)

        // 4. Push local changes
        let pendingChanges = try await getPendingLocalChanges()
        try await pushChanges(pendingChanges + resolved)

        // 5. Mark as synced
        try await markSynced(pendingChanges)
    }

    private func fetchRemoteChanges() async throws -> [CKRecord] {
        let zone = CKRecordZone(zoneName: "TomeData")
        let operation = CKFetchRecordZoneChangesOperation(
            recordZoneIDs: [zone.zoneID],
            configurationsByRecordZoneID: [
                zone.zoneID: CKFetchRecordZoneChangesOperation.ZoneConfiguration(
                    previousServerChangeToken: changeTokens[zone.zoneID]
                )
            ]
        )
        // Configure operation callbacks
        // ...
    }
}
```

#### Success Metrics
- Full sync < 5s for 1000 records (good network)
- Incremental sync < 1s for 10 changes
- Zero data loss

---

### P3-013: Add conflict resolution handling

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P3-012
**Blocks:** None

#### Description
Implement conflict detection and resolution strategies.

#### Acceptance Criteria
- [ ] Detect conflicts (same record modified on multiple devices)
- [ ] Last-write-wins for simple fields
- [ ] Merge strategy for collections (union)
- [ ] User notification for significant conflicts
- [ ] Conflict log for debugging
- [ ] Manual resolution UI (future)

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
            _ => unimplemented!(),
        }
    }
}
```

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

### P3-015: Implement offline queue system

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P3-012
**Blocks:** None

#### Description
Build the offline queue to handle changes when network is unavailable.

#### Acceptance Criteria
- [ ] Queue changes when offline
- [ ] Persist queue to disk
- [ ] Replay queue when online
- [ ] Handle queue conflicts
- [ ] Limit queue size (with warning)
- [ ] Show queued changes count
- [ ] Clear queue option (with warning)

#### Technical Notes
```rust
pub struct OfflineQueue {
    db: SqlitePool,
}

impl OfflineQueue {
    pub async fn enqueue(&self, change: SyncChange) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO sync_queue (id, change_type, entity_type, entity_id, payload, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            change.id,
            change.change_type,
            change.entity_type,
            change.entity_id,
            change.payload,
            change.created_at,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn dequeue_batch(&self, limit: usize) -> Result<Vec<SyncChange>> {
        // Get oldest N changes
    }

    pub async fn mark_completed(&self, ids: &[Uuid]) -> Result<()> {
        // Remove from queue
    }

    pub async fn count(&self) -> Result<usize> {
        // Count pending
    }
}
```

```sql
CREATE TABLE sync_queue (
    id TEXT PRIMARY KEY,
    change_type TEXT NOT NULL, -- create, update, delete
    entity_type TEXT NOT NULL, -- bookmark, collection, etc.
    entity_id TEXT NOT NULL,
    payload TEXT NOT NULL, -- JSON
    created_at TEXT NOT NULL,
    attempts INTEGER DEFAULT 0,
    last_error TEXT
);

CREATE INDEX idx_sync_queue_created ON sync_queue(created_at);
```

#### Success Metrics
- Queue persists across restarts
- Replay in order
- No lost changes

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
    └──── P3-010 (CloudKit Design) ◄──────────────┘
              │
              ├──── P3-011 (CKRecord)
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
- [ ] Reading position remembered per page
- [ ] CloudKit sync configured and working
- [ ] Bookmarks sync between two Macs
- [ ] Sync status visible in UI
- [ ] Offline changes queued and replayed
- [ ] Conflicts resolved automatically (last-write-wins)
- [ ] Sync reliability > 99.5%
