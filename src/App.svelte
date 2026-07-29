<!--
  The application shell (S1-14).

  Three panels: the library, the reader, the page outline. This component
  owns the state they share — which source, which page, which heading is
  current — and nothing else. Navigation history is S1-15.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import Layout from '$lib/components/Layout.svelte';
  import Library from '$lib/components/Library.svelte';
  import Outline from '$lib/components/Outline.svelte';
  import Reader from '$lib/components/Reader.svelte';
  import SearchModal from '$lib/components/SearchModal.svelte';
  import { isCommand, isCommandShift } from '$lib/keys';
  import { classifyLink, NavigationHistory, type HistoryEntry } from '$lib/navigation';
  import { preferences } from '$lib/stores/preferences';
  import {
    listPages,
    listSources,
    openExternal,
    readPage,
    sourceExists,
    type PageSummary,
    type ReaderPage,
    type SearchHit,
    type SourceSummary,
  } from '$lib/tauri';

  let sources = $state<SourceSummary[]>([]);
  let pages = $state<PageSummary[]>([]);
  let selectedSource = $state<string | null>(null);
  let page = $state<ReaderPage | null>(null);
  let fragment = $state<string | null>(null);
  let activeHeading = $state<string | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);
  /** Where to restore the reader to, and the token that makes it re-apply. */
  let restoreScroll = $state(0);
  let navigationToken = $state(0);

  let layout = $state<Layout>();
  let reader = $state<Reader>();

  let searchOpen = $state(false);
  let searchScope = $state<string | null>(null);
  /** What had focus when search opened, so Escape gives it back (P2-004). */
  let focusBeforeSearch: Element | null = null;

  // Not `$state`: the history object mutates in place on every scroll event,
  // and making it reactive would re-render the whole shell at pointer rate.
  // The two things the UI actually needs from it are mirrored below.
  const history = new NavigationHistory();
  let canGoBack = $state(false);
  let canGoForward = $state(false);

  function syncHistoryButtons(): void {
    canGoBack = history.canGoBack();
    canGoForward = history.canGoForward();
  }

  onMount(async () => {
    try {
      sources = await listSources();
      const first = sources[0];
      if (first) await selectSource(first.id);
    } catch (e) {
      error = message(e);
    } finally {
      loading = false;
    }
    await restoreSearchScope();
  });

  /**
   * P2-008 asks the scope to be remembered across launches. A source can be
   * removed between them, and a scope naming one that is gone would silently
   * return nothing for ever — so the remembered value is checked before it is
   * used, and dropped if it no longer names anything.
   */
  async function restoreSearchScope(): Promise<void> {
    const remembered = preferences.searchScope.load();
    if (remembered === '') return;
    try {
      searchScope = (await sourceExists(remembered)) ? remembered : null;
      if (searchScope === null) preferences.searchScope.save('');
    } catch {
      searchScope = null;
    }
  }

  function setSearchScope(scope: string | null): void {
    searchScope = scope;
    preferences.searchScope.save(scope ?? '');
  }

  function openSearch(): void {
    focusBeforeSearch = document.activeElement;
    searchOpen = true;
  }

  function closeSearch(): void {
    searchOpen = false;
    // Focus goes back where it came from. Without this it lands on <body>,
    // and the next keystroke goes nowhere.
    if (focusBeforeSearch instanceof HTMLElement) focusBeforeSearch.focus();
    focusBeforeSearch = null;
  }

  /** Open a search result, which may live in another source. */
  function openResult(hit: SearchHit): void {
    void navigateTo({
      sourceId: hit.source_id,
      path: hit.path,
      fragment: null,
      scrollTop: 0,
    });
  }

  function message(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
  }

  async function selectSource(id: string): Promise<void> {
    error = null;
    try {
      pages = await listPages(id);
      selectedSource = id;
      const first = pages[0];
      if (first) await navigateTo({ sourceId: id, path: first.path, fragment: null, scrollTop: 0 });
      else page = null;
    } catch (e) {
      error = message(e);
    }
  }

  /** Go somewhere, and remember that we did. */
  async function open(path: string, hash: string | null = null): Promise<void> {
    if (!selectedSource) return;
    await navigateTo({ sourceId: selectedSource, path, fragment: hash, scrollTop: 0 });
  }

  /**
   * Display an entry and push it onto the history.
   *
   * Cross-source navigation goes through here too: an entry names its source,
   * so following a link into another documentation set reloads that source's
   * page list before showing the page — and back returns to the first source
   * with no special case.
   */
  async function navigateTo(entry: HistoryEntry): Promise<void> {
    if (await show(entry)) {
      history.push(entry);
      syncHistoryButtons();
    }
  }

  /**
   * Display an entry WITHOUT touching the history. Back and forward use this;
   * pushing there would make the buttons undo each other for ever.
   *
   * Returns whether it worked, so a failed navigation does not leave a
   * history entry pointing at a page that would not open.
   */
  async function show(entry: HistoryEntry): Promise<boolean> {
    error = null;
    try {
      if (entry.sourceId !== selectedSource) {
        pages = await listPages(entry.sourceId);
        selectedSource = entry.sourceId;
      }
      page = await readPage(entry.sourceId, entry.path);
      fragment = entry.fragment;
      restoreScroll = entry.scrollTop;
      activeHeading = null;
      navigationToken += 1;
      return true;
    } catch (e) {
      error = message(e);
      return false;
    }
  }

  async function goBack(): Promise<void> {
    const entry = history.back();
    if (entry) {
      await show(entry);
      syncHistoryButtons();
    }
  }

  async function goForward(): Promise<void> {
    const entry = history.forward();
    if (entry) {
      await show(entry);
      syncHistoryButtons();
    }
  }

  /**
   * A link was clicked inside the reader.
   *
   * The renderer has already turned internal links into library paths and
   * left external ones absolute (`pipeline::relink`), so the *shape* of the
   * href is the routing decision — no URL parsing and no host comparison
   * here. Back/forward and cross-source routing are S1-15.
   */
  function navigate(href: string): void {
    const target = classifyLink(href);
    switch (target.kind) {
      case 'fragment':
        // A same-page fragment scrolls; it does not reload the page.
        // Reloading would lose the reader's position on every permalink
        // click, and Sphinx puts a permalink on every heading. It is still
        // history, so back returns to where the reader was.
        activeHeading = target.fragment;
        reader?.scrollToHeading(target.fragment);
        if (selectedSource && page) {
          history.push({
            sourceId: selectedSource,
            path: page.path,
            fragment: target.fragment,
            scrollTop: 0,
          });
          syncHistoryButtons();
        }
        break;
      case 'page':
        void open(target.path, target.fragment);
        break;
      case 'external':
        // Rust decides whether this is safe to hand to the OS; the frontend
        // only asks. See `open_external` in src-tauri/src/reader.rs.
        openExternal(target.url).catch((e) => (error = message(e)));
        break;
    }
  }

  /** The reader scrolled. Remember it, so back returns to the same place. */
  function readerScrolled(state: { top: number; activeId: string | null }): void {
    activeHeading = state.activeId;
    history.recordScroll(state.top);
  }

  /**
   * Window-level shortcuts. Only modifier combinations live here — see
   * `$lib/keys.ts` for why single-letter keys must be reader-scoped and must
   * bail while a text field has focus.
   */
  function shortcuts(event: KeyboardEvent): void {
    if (isCommand(event, 'k')) {
      event.preventDefault();
      // Toggle rather than open: pressing the shortcut again is the most
      // natural way to dismiss a modal it opened.
      if (searchOpen) closeSearch();
      else openSearch();
    } else if (isCommand(event, '1')) {
      event.preventDefault();
      layout?.toggleLeft();
    } else if (isCommand(event, '2')) {
      event.preventDefault();
      layout?.toggleRight();
    } else if (isCommand(event, '\\')) {
      event.preventDefault();
      layout?.toggleBoth();
    } else if (isCommand(event, 'f')) {
      // ⌘F is the browser's own find in a dev-server tab, and it would search
      // the app's chrome rather than the page — the reader's document is not
      // reachable from here. Preventing the default and routing to the frame
      // is what makes it find the thing the user is looking at.
      event.preventDefault();
      reader?.openFind();
    } else if (isCommand(event, 'g')) {
      event.preventDefault();
      reader?.stepFind(1);
    } else if (isCommandShift(event, 'g')) {
      event.preventDefault();
      reader?.stepFind(-1);
    } else if (isCommand(event, '[')) {
      event.preventDefault();
      void goBack();
    } else if (isCommand(event, ']')) {
      event.preventDefault();
      void goForward();
    }
  }
</script>

<svelte:window onkeydown={shortcuts} />

<SearchModal
  open={searchOpen}
  {sources}
  scope={searchScope}
  onscope={setSearchScope}
  onclose={closeSearch}
  onselect={openResult}
/>

<div class="shell">
  <header>
    <nav class="history" aria-label="History">
      <button onclick={goBack} disabled={!canGoBack} aria-label="Back" title="Back (⌘[)">
        <span aria-hidden="true">‹</span>
      </button>
      <button
        onclick={goForward}
        disabled={!canGoForward}
        aria-label="Forward"
        title="Forward (⌘])"
      >
        <span aria-hidden="true">›</span>
      </button>
    </nav>
    <h1>{page?.title ?? 'Tome'}</h1>
    {#if page}
      <span class="path selectable" title={page.path}>{page.path}</span>
    {/if}
  </header>

  {#if error}
    <p class="error selectable" role="alert">{error}</p>
  {/if}

  <div class="body">
    {#if loading}
      <p class="notice" aria-live="polite">Loading…</p>
    {:else if sources.length === 0}
      <div class="notice">
        <p>The library is empty.</p>
        <p class="hint selectable">
          Add a source configuration and run <code>tome pull &lt;source-id&gt;</code>.
        </p>
      </div>
    {:else}
      <Layout bind:this={layout}>
        {#snippet left()}
          <Library
            {sources}
            {pages}
            {selectedSource}
            selectedPage={page?.path ?? null}
            onselectsource={(id) => selectSource(id)}
            onselectpage={(path) => open(path)}
          />
        {/snippet}

        {#snippet main()}
          <Reader
            bind:this={reader}
            {page}
            {fragment}
            scrollTop={restoreScroll}
            token={navigationToken}
            onnavigate={navigate}
            onscroll={readerScrolled}
          />
        {/snippet}

        {#snippet right()}
          <Outline
            outline={page?.outline ?? []}
            activeId={activeHeading}
            onselect={(id) => {
              activeHeading = id;
              reader?.scrollToHeading(id);
            }}
          />
        {/snippet}
      </Layout>
    {/if}
  </div>
</div>

<style>
  /* Flex, not `grid-template-rows: auto auto 1fr`.
     The grid version assigned tracks by child *order*, and the error banner
     is conditional — so with no error the three-panel layout landed in the
     second `auto` track and the `1fr` track sat empty. It only looked right
     when the content happened to be tall enough to fill the window on its
     own; a source with one short page left a dead band below the panels.
     `.body` claims the remaining height explicitly, whatever precedes it. */
  .shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .body {
    flex: 1 1 auto;
    /* Without this a tall child (the reader) sets the flex base size and
       pushes the panel past the window instead of scrolling inside it. */
    min-height: 0;
  }

  header {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    /* Room for the traffic lights: the window uses a transparent title bar
       with a hidden title, so the top-left of the content area is under
       them. */
    padding: var(--space-2) var(--space-4) var(--space-2) 5.5rem;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    /* The window is dragged by its title bar; without this the header is a
       dead strip that looks draggable and is not. */
    -webkit-app-region: drag;
  }

  .history {
    display: flex;
    gap: var(--space-1);
    -webkit-app-region: no-drag;
  }

  .history button {
    width: 1.6rem;
    height: 1.6rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    font-size: var(--text-lg);
    line-height: 1;
    color: var(--color-text-secondary);
  }

  .history button:hover:not(:disabled) {
    background: var(--color-bg-tertiary);
    color: var(--color-text-primary);
  }

  /* Disabled is carried by opacity AND by the `disabled` attribute, which is
     what a screen reader announces and what stops the click. Colour alone
     would say nothing to either. */
  .history button:disabled {
    opacity: 0.35;
  }

  h1 {
    margin: 0;
    font-family: var(--font-ui);
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .path {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--color-text-tertiary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    -webkit-app-region: no-drag;
  }

  .notice {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    height: 100%;
    margin: 0;
    color: var(--color-text-secondary);
    font-family: var(--font-ui);
    font-size: var(--text-sm);
  }

  .notice p {
    margin: 0;
  }

  .hint {
    color: var(--color-text-tertiary);
  }

  .error {
    margin: 0;
    padding: var(--space-2) var(--space-4);
    background: var(--color-bg-tertiary);
    border-bottom: 1px solid var(--color-border);
    color: var(--color-error-text);
    font-family: var(--font-ui);
    font-size: var(--text-sm);
  }

  code {
    font-family: var(--font-mono);
    font-size: 0.9em;
    background: var(--color-code-bg);
    padding: 0.1em 0.3em;
    border-radius: var(--radius-sm);
  }
</style>
