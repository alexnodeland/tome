<!--
  The reader pane (S1-13).

  A sandboxed <iframe> and nothing else. The component's whole job is to own
  the frame's lifecycle and pass messages; it never touches page HTML beyond
  handing the string from Rust to `ReaderFrame.showPage`. Rendering, escaping,
  and highlighting all happen in Rust — see `crates/tome-core/src/render.rs`
  and the contract it owes `sanitize.rs`.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import FindBar from '$lib/components/FindBar.svelte';
  import { appearanceAttributes, loadAppearance, type Appearance } from '$lib/appearance';
  import { ReaderFrame } from '$lib/reader/bridge';
  import type { ReaderPage } from '$lib/tauri';

  interface Props {
    page: ReaderPage | null;
    /** Fragment to scroll to once the page has loaded. */
    fragment?: string | null;
    /** Scroll offset to restore, for back/forward (S1-15). */
    scrollTop?: number;
    /**
     * Bumped by the shell on every navigation.
     *
     * Without it, going *back* to the page already displayed would be a
     * no-op — same source, same path, same fragment — and the scroll
     * position the history entry remembers would never be applied.
     */
    token?: number;
    onnavigate?: (href: string, modified: boolean) => void;
    onscroll?: (state: { top: number; activeId: string | null }) => void;
  }

  let {
    page = null,
    fragment = null,
    scrollTop = 0,
    token = 0,
    onnavigate,
    onscroll,
  }: Props = $props();

  let element: HTMLIFrameElement;
  let bridge: ReaderFrame | undefined;
  let shownKey = $state<string | null>(null);

  // In-page find (S2-8). The state lives here rather than in the shell
  // because it belongs to the frame's lifetime: changing page clears it.
  let findOpen = $state(false);
  let findTotal = $state(0);
  let findIndex = $state(0);
  let findSupported = $state(true);

  onMount(() => {
    bridge = new ReaderFrame(element, {
      onNavigate: (href, modified) => onnavigate?.(href, modified),
      onScroll: (state) => onscroll?.({ top: state.top, activeId: state.activeId }),
      onFindResults: (state) => {
        findTotal = state.total;
        findIndex = state.index;
        findSupported = state.supported;
      },
    });
    bridge.attach();
    // The frame has an opaque origin and cannot inherit the shell's cascade,
    // so it is told the appearance explicitly. Queued by the bridge until the
    // frame reports ready, which is why this can run before it has loaded.
    bridge.settings(appearanceAttributes(loadAppearance()));
    return () => bridge?.destroy();
  });

  // Push a page into the frame when it changes. Keyed on the navigation
  // token and the page's identity, not on object identity: re-rendering the
  // same page (a theme change, a parent re-render) must not reset the
  // reader's scroll position to the top, while going *back* to it must.
  $effect(() => {
    if (!bridge || !page) return;
    const key = `${token} ${page.source_id} ${page.path} ${fragment ?? ''}`;
    if (key === shownKey) return;
    shownKey = key;
    bridge.showPage(page.html, { fragment: fragment ?? undefined, scrollTop });
    // The frame drops its match ranges when the document is replaced; the
    // counter here has to follow, or the bar reports matches for a page that
    // is no longer displayed.
    findTotal = 0;
    findIndex = 0;
  });

  /**
   * Push appearance into the frame (P5-007).
   *
   * Attributes, so nothing re-renders and nothing is re-highlighted — the
   * whole point of highlighting emitting classes rather than colours. The
   * frame is not re-created, so the reader keeps its scroll position across a
   * theme change.
   */
  export function applyAppearance(appearance: Appearance): void {
    bridge?.settings(appearanceAttributes(appearance));
  }

  /** Scroll the frame to a heading. Called by the TOC sidebar (S1-14). */
  export function scrollToHeading(id: string): void {
    bridge?.scrollTo(id);
  }

  /** Open the find bar, or refocus it if it is already open (⌘F). */
  export function openFind(): void {
    findOpen = true;
  }

  export function closeFind(): void {
    findOpen = false;
    // Clearing on close is the point of closing: leaving the page painted
    // yellow after the bar has gone gives no way to remove the highlighting.
    bridge?.findClear();
    findTotal = 0;
    findIndex = 0;
  }

  /** ⌘G / ⇧⌘G. Does nothing when the bar is closed, rather than searching for
   *  whatever was last typed into a bar the user cannot see. */
  export function stepFind(direction: 1 | -1): void {
    if (!findOpen) return;
    bridge?.findStep(direction);
  }

  export function isFindOpen(): boolean {
    return findOpen;
  }
</script>

<div class="reader" class:empty={!page}>
  <!-- srcdoc and sandbox are set by ReaderFrame.attach(), not here, so that
       the one place the sandbox could be weakened is in code next to the
       comment explaining why it must not be. -->
  <iframe bind:this={element} title="Documentation"></iframe>

  <FindBar
    open={findOpen}
    total={findTotal}
    index={findIndex}
    supported={findSupported}
    onfind={(query) => bridge?.find(query)}
    onstep={(direction) => bridge?.findStep(direction)}
    onclose={closeFind}
  />

  {#if !page}
    <p class="placeholder">Select a page to read.</p>
  {/if}
</div>

<style>
  .reader {
    position: relative;
    height: 100%;
    background: var(--color-bg-primary);
  }

  iframe {
    display: block;
    width: 100%;
    height: 100%;
    border: 0;
    /* The frame paints its own background from the shared tokens; making it
       transparent here would show the app's background through during load
       and flash on every navigation. */
    background: var(--color-bg-primary);
  }

  .empty iframe {
    visibility: hidden;
  }

  .placeholder {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 0;
    color: var(--color-text-secondary);
    font-family: var(--font-ui);
    font-size: var(--text-sm);
  }
</style>
