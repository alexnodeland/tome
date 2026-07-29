<!--
  In-page find (S2-8, spec P2-007).

  The bar lives in the *app*, the search lives in the *frame*. That split is
  forced rather than chosen: the reader is `sandbox="allow-scripts"` with no
  `allow-same-origin`, so its origin is opaque and the app cannot read its
  document. `window.find()` called from here would search the app's own chrome
  and report nothing.

  So this component owns a text field, a count, and four buttons, and every
  actual operation is a `postMessage` whose answer arrives later through
  `onFindResults`. Nothing here knows what the page contains.
-->
<script lang="ts">
  import { tick } from 'svelte';

  interface Props {
    open: boolean;
    /** Matches found, and which one is current (1-based, 0 for none). */
    total: number;
    index: number;
    /** Whether the frame can paint highlights at all. */
    supported: boolean;
    onfind: (query: string) => void;
    onstep: (direction: 1 | -1) => void;
    onclose: () => void;
  }

  let { open, total, index, supported, onfind, onstep, onclose }: Props = $props();

  /**
   * Shorter than global search's 150 ms.
   *
   * Find-in-page is local work with no index and no IPC to Rust, and the
   * feedback loop is tighter: the user is watching the page highlight as they
   * type, not waiting for a list to appear.
   */
  const DEBOUNCE_MS = 80;

  let query = $state('');
  let input = $state<HTMLInputElement>();
  let timer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    if (open) {
      void tick().then(() => {
        input?.focus();
        // Select rather than clear: reopening with the previous query still
        // in place and ready to be replaced is what every other find bar
        // does, and clearing loses a query the user may want to step through
        // again.
        input?.select();
      });
    } else {
      clearTimeout(timer);
    }
  });

  function schedule(): void {
    clearTimeout(timer);
    const text = query;
    timer = setTimeout(() => onfind(text), DEBOUNCE_MS);
  }

  function keydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      onclose();
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      // Enter steps forward, Shift+Enter back — P2-007's navigation, and the
      // convention every browser's find bar uses.
      onstep(event.shiftKey ? -1 : 1);
    }
  }
</script>

{#if open}
  <div class="find" role="search">
    <input
      bind:this={input}
      bind:value={query}
      oninput={schedule}
      onkeydown={keydown}
      type="text"
      autocomplete="off"
      spellcheck="false"
      placeholder="Find in page…"
      aria-label="Find in page"
    />

    <span class="count" aria-live="polite">
      {#if !supported}
        unavailable
      {:else if total > 0}
        {index} of {total}
      {:else if query.trim() === ''}
        <!-- Nothing typed yet. Blank rather than "no matches", which would
             be a claim about a search that has not run. -->
        &nbsp;
      {:else}
        no matches
      {/if}
    </span>

    <button
      onclick={() => onstep(-1)}
      disabled={total === 0}
      aria-label="Previous match"
      title="Previous match (⇧⌘G)"
    >
      <span aria-hidden="true">↑</span>
    </button>
    <button
      onclick={() => onstep(1)}
      disabled={total === 0}
      aria-label="Next match"
      title="Next match (⌘G)"
    >
      <span aria-hidden="true">↓</span>
    </button>
    <button onclick={onclose} aria-label="Close find" title="Close (Esc)">
      <span aria-hidden="true">×</span>
    </button>
  </div>
{/if}

<style>
  .find {
    position: absolute;
    top: var(--space-2);
    right: var(--space-3);
    z-index: 2;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-sm);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-md);
  }

  input {
    width: 16rem;
    font: inherit;
    color: var(--color-text-primary);
    background: transparent;
    border: none;
    outline: none;
    /* The chrome disables selection globally; a text field must opt back in
       or the user cannot select what they have typed. */
    user-select: text;
    -webkit-user-select: text;
  }

  .count {
    flex: none;
    min-width: 6rem;
    text-align: right;
    color: var(--color-text-tertiary);
    font-variant-numeric: tabular-nums;
  }

  button {
    padding: 0 var(--space-1);
    color: var(--color-text-secondary);
    background: none;
    border: none;
    cursor: default;
  }

  button:disabled {
    color: var(--color-text-tertiary);
  }

  button:not(:disabled):hover {
    color: var(--color-text-primary);
  }
</style>
