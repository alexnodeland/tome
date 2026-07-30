<!--
  Global search (S2-7, specs P2-004/005/008/016/017).

  One component rather than four, because the pieces the tickets split apart
  are not separable in practice: the keyboard handler (P2-017) needs the
  results list's length, the results list (P2-005) needs the scope (P2-008) to
  know what it is showing, and the history list (P2-016) occupies the same
  region as the results and swaps with them. Splitting them would mean four
  components passing the same five pieces of state up and down.

  # The one rule that must not be broken here

  **A snippet's text is rendered as a text node, never as HTML.** It is crawled
  page content, and unlike the reader — which is a sandboxed iframe with its
  own opaque origin — this markup lives in the app's own DOM, where the IPC
  bridge is reachable. The backend therefore sends `SnippetSpan[]`, and each
  span's text goes through Svelte's normal interpolation, which escapes. There
  is no `{@html}` in this file and there must never be one.
-->
<script lang="ts">
  import { trapFocus } from '$lib/a11y';
  import { tick } from 'svelte';
  import { searchHistory } from '$lib/stores/searchHistory';
  import {
    search as runSearch,
    type SearchHit,
    type SearchSuggestion,
    type SourceSummary,
  } from '$lib/tauri';

  interface Props {
    open: boolean;
    sources: SourceSummary[];
    /** The source to scope to, or `null` for everything. Owned by the shell
     *  so it survives the modal closing, which is P2-008's "remember last
     *  used scope". */
    scope: string | null;
    onscope: (scope: string | null) => void;
    onclose: () => void;
    onselect: (hit: SearchHit) => void;
  }

  let { open, sources, scope, onscope, onclose, onselect }: Props = $props();

  /** P2-004's "debounced 150ms". */
  const DEBOUNCE_MS = 150;
  const LIMIT = 30;

  let query = $state('');
  let hits = $state<SearchHit[]>([]);
  let suggestions = $state<SearchSuggestion[]>([]);
  let elapsed = $state(0);
  let truncated = $state(false);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let selected = $state(0);
  let history = $state<string[]>([]);

  let input = $state<HTMLInputElement>();
  let listbox = $state<HTMLElement>();
  let timer: ReturnType<typeof setTimeout> | undefined;

  /**
   * Which response is current.
   *
   * Searches are async and a debounce does not serialise them: a slow query
   * issued at keystroke 3 can resolve after a fast one issued at keystroke 5,
   * and the results list would then show answers to a question the user has
   * already finished changing. Every request carries a sequence number and a
   * stale response is dropped.
   */
  let sequence = 0;

  /** P2-016: recents replace results while the query is empty. */
  const showingHistory = $derived(query.trim() === '' && history.length > 0);

  const scopeName = $derived(
    scope === null ? null : (sources.find((s) => s.id === scope)?.name ?? scope),
  );

  $effect(() => {
    if (open) {
      history = searchHistory.list();
      // `tick` rather than a bare call: the input does not exist until the
      // `{#if open}` block has rendered.
      void tick().then(() => input?.focus());
    } else {
      // Reset on close so reopening does not flash the previous query's
      // results before the new search resolves.
      clearTimeout(timer);
      query = '';
      hits = [];
      suggestions = [];
      error = null;
      selected = 0;
      loading = false;
    }
  });

  /** Re-run when the scope changes, so the results match the indicator. */
  $effect(() => {
    void scope;
    if (open && query.trim() !== '') schedule();
  });

  function schedule(): void {
    clearTimeout(timer);
    const text = query;
    if (text.trim() === '') {
      // No request at all: an empty query has no answer, and firing one would
      // make the recents list flicker in behind a spinner.
      sequence += 1;
      hits = [];
      suggestions = [];
      loading = false;
      error = null;
      return;
    }
    loading = true;
    timer = setTimeout(() => void execute(text), DEBOUNCE_MS);
  }

  async function execute(text: string): Promise<void> {
    const ticket = ++sequence;
    try {
      const response = await runSearch(text, scope, LIMIT);
      if (ticket !== sequence) return; // A newer query has been issued.
      hits = response.hits;
      suggestions = response.suggestions;
      elapsed = response.elapsed_ms;
      truncated = response.truncated;
      error = null;
      selected = 0;
    } catch (e) {
      if (ticket !== sequence) return;
      error = e instanceof Error ? e.message : String(e);
      hits = [];
      suggestions = [];
    } finally {
      if (ticket === sequence) loading = false;
    }
  }

  function choose(hit: SearchHit): void {
    history = searchHistory.add(query);
    onselect(hit);
    onclose();
  }

  function rerun(entry: string): void {
    query = entry;
    schedule();
    input?.focus();
  }

  function forget(entry: string): void {
    history = searchHistory.remove(entry);
  }

  function clearHistory(): void {
    history = searchHistory.clear();
  }

  /** Apply a correction the backend offered, as a real search. */
  function accept(suggestion: SearchSuggestion): void {
    query = query.replace(new RegExp(escapeRegExp(suggestion.typed), 'gi'), suggestion.meant);
    schedule();
    input?.focus();
  }

  /** A typed term is not a pattern. Without this, searching for `c++` and
   *  accepting a correction would throw on an invalid regular expression. */
  function escapeRegExp(text: string): string {
    return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  /**
   * P2-017: ↑/↓ move, Enter opens, and both ends wrap.
   *
   * Wrapping matters more than it looks: without it, holding ↓ stops silently
   * at the last result and the user cannot tell whether the key stopped
   * working or the list ended.
   */
  function keydown(event: KeyboardEvent): void {
    const length = showingHistory ? history.length : hits.length;

    switch (event.key) {
      case 'Escape':
        event.preventDefault();
        onclose();
        return;
      case 'ArrowDown':
        if (length === 0) return;
        event.preventDefault();
        selected = (selected + 1) % length;
        break;
      case 'ArrowUp':
        if (length === 0) return;
        event.preventDefault();
        selected = (selected - 1 + length) % length;
        break;
      case 'Home':
        if (length === 0) return;
        event.preventDefault();
        selected = 0;
        break;
      case 'End':
        if (length === 0) return;
        event.preventDefault();
        selected = length - 1;
        break;
      case 'Enter': {
        event.preventDefault();
        if (showingHistory) {
          const entry = history[selected];
          if (entry !== undefined) rerun(entry);
          return;
        }
        const hit = hits[selected];
        if (hit !== undefined) choose(hit);
        return;
      }
      default:
        return;
    }
    scrollSelectedIntoView();
  }

  function scrollSelectedIntoView(): void {
    void tick().then(() => {
      const element = listbox?.querySelector('[aria-selected="true"]');
      // Guarded rather than called: `scrollIntoView` is not implemented in
      // every DOM the app is rendered into (jsdom has no layout, so it has no
      // scrolling), and an unhandled rejection from a *cosmetic* scroll would
      // be a real failure caused by a nicety.
      if (element instanceof HTMLElement && typeof element.scrollIntoView === 'function') {
        element.scrollIntoView({ block: 'nearest' });
      }
    });
  }

  /** The kind badge, expanded. `fn` on its own reads as an abbreviation of
   *  nothing to someone who does not write Rust. */
  function kindLabel(kind: string): string {
    return kind;
  }
</script>

{#if open}
  <!-- The backdrop closes on click (P2-004). It is a plain div with a click
       handler and `aria-hidden`, not a button: it is not in the tab order and
       Escape already provides the keyboard route, so a focusable backdrop
       would only add a tab stop that does nothing visible. -->
  <div class="backdrop" role="presentation" onclick={onclose} onkeydown={keydown}></div>

  <div
    class="modal"
    role="dialog"
    aria-modal="true"
    aria-label="Search documentation"
    use:trapFocus
    tabindex="-1"
    onkeydown={keydown}
  >
    <div class="field">
      {#if scope !== null}
        <!-- P2-008's scope indicator, with its own clear button. -->
        <span class="scope">
          {scopeName}
          <button
            class="clear-scope"
            onclick={() => onscope(null)}
            aria-label="Search all sources"
            title="Search all sources"
          >
            <span aria-hidden="true">×</span>
          </button>
        </span>
      {/if}
      <!-- Focused by the $effect above rather than by an `autofocus`
           attribute: the element does not exist until this block renders, and
           `autofocus` only fires on initial page load, so it would do nothing
           on the second and every subsequent open. -->
      <input
        bind:this={input}
        bind:value={query}
        oninput={schedule}
        type="text"
        autocomplete="off"
        spellcheck="false"
        placeholder={scope === null ? 'Search all documentation…' : `Search ${scopeName}…`}
        aria-label="Search query"
        aria-controls="search-results"
        aria-activedescendant={hits.length > 0 ? `search-result-${selected}` : undefined}
      />
      {#if loading}
        <span class="spinner" aria-label="Searching">…</span>
      {/if}
    </div>

    {#if sources.length > 1}
      <div class="scopes">
        <button class:active={scope === null} onclick={() => onscope(null)}>All</button>
        {#each sources as source (source.id)}
          <button class:active={scope === source.id} onclick={() => onscope(source.id)}>
            {source.name}
          </button>
        {/each}
      </div>
    {/if}

    {#if suggestions.length > 0}
      <p class="suggestions">
        Did you mean
        {#each suggestions as suggestion, i (suggestion.typed)}
          {#if i > 0},
          {/if}<button class="correction" onclick={() => accept(suggestion)}
            >{suggestion.meant}</button
          >
        {/each}?
      </p>
    {/if}

    <div class="results" bind:this={listbox} id="search-results" role="listbox" tabindex="-1">
      {#if error !== null}
        <p class="state error" role="alert">{error}</p>
      {:else if showingHistory}
        <div class="recents-header">
          <span>Recent searches</span>
          <button class="link" onclick={clearHistory}>Clear all</button>
        </div>
        {#each history as entry, index (entry)}
          <div class="recent" class:selected={index === selected}>
            <button
              class="recent-run"
              role="option"
              aria-selected={index === selected}
              onclick={() => rerun(entry)}
              onmouseenter={() => (selected = index)}
            >
              {entry}
            </button>
            <button class="forget" onclick={() => forget(entry)} aria-label={`Forget ${entry}`}>
              <span aria-hidden="true">×</span>
            </button>
          </div>
        {/each}
      {:else if query.trim() === ''}
        <p class="state">
          Type to search. Prefix a term with <code>@</code> to match declared symbols only.
        </p>
      {:else if loading && hits.length === 0}
        <p class="state">Searching…</p>
      {:else if hits.length === 0}
        <p class="state">
          No results for “{query}”.
          {#if scope !== null}
            <button class="link" onclick={() => onscope(null)}>Search all sources</button>
          {/if}
        </p>
      {:else}
        <p class="count">
          {hits.length}{truncated ? '+' : ''}
          {hits.length === 1 ? 'result' : 'results'} in {elapsed.toFixed(0)} ms
        </p>
        {#each hits as hit, index (hit.source_id + hit.path)}
          <button
            class="hit"
            id={`search-result-${index}`}
            role="option"
            aria-selected={index === selected}
            class:selected={index === selected}
            onclick={() => choose(hit)}
            onmouseenter={() => (selected = index)}
          >
            <span class="hit-title">
              {hit.title}
              {#if hit.symbol_kind !== null}
                <span class="kind">{kindLabel(hit.symbol_kind)}</span>
              {/if}
            </span>
            <span class="hit-source">{hit.source_name}</span>
            {#if hit.snippet.length > 0}
              <span class="snippet">
                <!-- Text nodes. See the note at the top of this file. -->
                {#each hit.snippet as span, spanIndex (spanIndex)}{#if span.matched}<mark
                      >{span.text}</mark
                    >{:else}{span.text}{/if}{/each}
              </span>
            {/if}
          </button>
        {/each}
      {/if}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 40%);
    z-index: 10;
  }

  .modal {
    position: fixed;
    top: 12vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(680px, 92vw);
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    z-index: 11;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
    font-family: var(--font-ui);
  }

  .field {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-3);
    border-bottom: 1px solid var(--color-border);
  }

  .field input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: var(--text-lg);
    color: var(--color-text-primary);
    background: transparent;
    border: none;
    outline: none;
    /* The chrome disables selection globally; a text field must opt back in
       or the user cannot select what they have typed. */
    user-select: text;
    -webkit-user-select: text;
  }

  .scope {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    flex: none;
    padding: var(--space-1) var(--space-2);
    font-size: var(--text-sm);
    color: var(--color-text-secondary);
    background: var(--color-bg-tertiary);
    border-radius: var(--radius-sm);
  }

  .clear-scope,
  .forget {
    padding: 0 var(--space-1);
    color: var(--color-text-tertiary);
    background: none;
    border: none;
    cursor: default;
  }

  .clear-scope:hover,
  .forget:hover {
    color: var(--color-text-primary);
  }

  .spinner {
    flex: none;
    color: var(--color-text-tertiary);
  }

  .scopes {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-3);
    border-bottom: 1px solid var(--color-border);
  }

  .scopes button {
    padding: var(--space-1) var(--space-2);
    font-size: var(--text-xs);
    color: var(--color-text-secondary);
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: default;
  }

  .scopes button.active {
    color: var(--color-text-primary);
    background: var(--color-bg-tertiary);
    border-color: var(--color-border);
  }

  .suggestions {
    margin: 0;
    padding: var(--space-2) var(--space-3);
    font-size: var(--text-sm);
    color: var(--color-text-secondary);
    border-bottom: 1px solid var(--color-border);
  }

  .correction,
  .link {
    font: inherit;
    color: var(--color-link);
    background: none;
    border: none;
    padding: 0;
    cursor: default;
    text-decoration: underline;
  }

  .results {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: var(--space-2) 0;
  }

  .count,
  .state,
  .recents-header {
    margin: 0;
    padding: var(--space-2) var(--space-3);
    font-size: var(--text-sm);
    color: var(--color-text-secondary);
  }

  .recents-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  .state.error {
    color: var(--color-error-text);
  }

  .hit,
  .recent-run {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    width: 100%;
    text-align: left;
    padding: var(--space-2) var(--space-3);
    background: none;
    border: none;
    font: inherit;
    color: var(--color-text-primary);
    cursor: default;
  }

  .hit.selected,
  .recent.selected {
    background: var(--color-bg-tertiary);
    box-shadow: inset 2px 0 0 var(--color-accent);
  }

  .recent {
    display: flex;
    align-items: center;
  }

  .recent-run {
    flex: 1;
    min-width: 0;
    flex-direction: row;
  }

  .hit-title {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    font-size: var(--text-md);
  }

  .kind {
    font-size: var(--text-xs);
    font-family: var(--font-mono);
    color: var(--color-text-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 0 var(--space-1);
  }

  .hit-source {
    font-size: var(--text-xs);
    color: var(--color-text-tertiary);
  }

  .snippet {
    font-size: var(--text-sm);
    color: var(--color-text-secondary);
    line-height: var(--leading-normal);
  }

  .snippet mark {
    color: var(--color-text-primary);
    background: var(--color-highlight-yellow);
    border-radius: 2px;
  }

  code {
    font-family: var(--font-mono);
    font-size: var(--text-code);
    background: var(--color-code-bg);
    border-radius: var(--radius-sm);
    padding: 0 var(--space-1);
  }
</style>
