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
  import { isCommand } from '$lib/keys';
  import {
    listPages,
    listSources,
    readPage,
    type PageSummary,
    type ReaderPage,
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

  let layout = $state<Layout>();
  let reader = $state<Reader>();

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
  });

  function message(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
  }

  async function selectSource(id: string): Promise<void> {
    selectedSource = id;
    page = null;
    error = null;
    try {
      pages = await listPages(id);
      const first = pages[0];
      if (first) await open(first.path);
    } catch (e) {
      error = message(e);
    }
  }

  async function open(path: string, hash: string | null = null): Promise<void> {
    if (!selectedSource) return;
    error = null;
    try {
      page = await readPage(selectedSource, path);
      fragment = hash;
      activeHeading = null;
    } catch (e) {
      error = message(e);
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
    if (href.startsWith('#')) {
      const id = href.slice(1);
      // A same-page fragment scrolls; it does not reload the page. Reloading
      // would lose the reader's position on every permalink click, and Sphinx
      // puts a permalink on every heading.
      activeHeading = id;
      reader?.scrollToHeading(id);
      return;
    }
    if (/^[a-z][a-z0-9+.-]*:/i.test(href)) {
      // External. Opening it in the user's browser needs the opener plugin
      // and a capability grant; S1-15 adds both together rather than silently
      // doing nothing here.
      error = `External links are not routed yet: ${href}`;
      return;
    }
    const [path, hash] = href.split('#', 2);
    if (path) void open(path, hash ?? null);
  }

  /**
   * Window-level shortcuts. Only modifier combinations live here — see
   * `$lib/keys.ts` for why single-letter keys must be reader-scoped and must
   * bail while a text field has focus.
   */
  function shortcuts(event: KeyboardEvent): void {
    if (isCommand(event, '1')) {
      event.preventDefault();
      layout?.toggleLeft();
    } else if (isCommand(event, '2')) {
      event.preventDefault();
      layout?.toggleRight();
    } else if (isCommand(event, '\\')) {
      event.preventDefault();
      layout?.toggleBoth();
    }
  }
</script>

<svelte:window onkeydown={shortcuts} />

<div class="shell">
  <header>
    <h1>{page?.title ?? 'Tome'}</h1>
    {#if page}
      <span class="path selectable" title={page.path}>{page.path}</span>
    {/if}
  </header>

  {#if error}
    <p class="error selectable" role="alert">{error}</p>
  {/if}

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
          onnavigate={navigate}
          onscroll={(state) => (activeHeading = state.activeId)}
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

<style>
  .shell {
    display: grid;
    grid-template-rows: auto auto 1fr;
    height: 100%;
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
