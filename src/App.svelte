<!--
  The application shell.

  S1-13 wires the reader end to end; the three-panel layout, the real library
  sidebar, and the TOC are S1-14, which replaces the temporary picker below.
  What is here now is deliberately the smallest thing that proves the whole
  path works: sources from the database, pages from the database, a page
  rendered by Rust and displayed in the sandboxed frame.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import Reader from '$lib/components/Reader.svelte';
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
  let error = $state<string | null>(null);
  let loading = $state(true);

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
    } catch (e) {
      error = message(e);
    }
  }

  /**
   * A link was clicked inside the reader.
   *
   * The renderer has already made internal links library paths and left
   * external ones as absolute URLs (`pipeline::relink`), so the shape of the
   * href is the routing decision. Full history and back/forward are S1-15.
   */
  function navigate(href: string): void {
    if (href.startsWith('#')) {
      void open(page?.path ?? '', href.slice(1));
      return;
    }
    if (/^[a-z][a-z0-9+.-]*:/i.test(href)) {
      // External. Opening it in the user's browser needs the opener plugin,
      // which the capabilities file does not grant yet — S1-15 adds both
      // together rather than silently doing nothing here.
      error = `External links are not routed yet: ${href}`;
      return;
    }
    const [path, hash] = href.split('#', 2);
    if (path) void open(path, hash ?? null);
  }
</script>

<div class="shell">
  <header>
    <h1>Tome</h1>
    {#if sources.length > 0}
      <select
        aria-label="Source"
        value={selectedSource}
        onchange={(e) => selectSource(e.currentTarget.value)}
      >
        {#each sources as source (source.id)}
          <option value={source.id}>{source.name} ({source.page_count})</option>
        {/each}
      </select>
      <select
        aria-label="Page"
        value={page?.path ?? ''}
        onchange={(e) => open(e.currentTarget.value)}
      >
        {#each pages as summary (summary.path)}
          <option value={summary.path}>{summary.title}</option>
        {/each}
      </select>
    {/if}
  </header>

  {#if error}
    <p class="error selectable" role="alert">{error}</p>
  {/if}

  <main>
    {#if loading}
      <p class="notice" aria-live="polite">Loading…</p>
    {:else if sources.length === 0}
      <div class="notice empty">
        <p>The library is empty.</p>
        <p class="hint selectable">
          Add a source configuration and run <code>tome pull &lt;source-id&gt;</code>.
        </p>
      </div>
    {:else}
      <Reader {page} {fragment} onnavigate={navigate} />
    {/if}
  </main>
</div>

<style>
  .shell {
    display: grid;
    grid-template-rows: auto auto 1fr;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    /* Room for the traffic lights: the window uses a transparent title bar
       with a hidden title, so the top-left of the content area is under
       them. */
    padding: var(--space-2) var(--space-4) var(--space-2) 5.5rem;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  h1 {
    margin: 0;
    font-family: var(--font-ui);
    font-size: var(--text-sm);
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--color-text-secondary);
  }

  select {
    font: inherit;
    font-family: var(--font-ui);
    font-size: var(--text-sm);
    max-width: 20rem;
  }

  main {
    min-height: 0;
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
    font-size: var(--text-sm);
  }

  .empty p {
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
