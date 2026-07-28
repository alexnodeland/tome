<!--
  The three-panel shell (S1-14, P1-017).

  Left: the library. Centre: the reader. Right: the page outline.

  Two things here are not obvious:

  1. **The sidebars are never unmounted, only given zero width.** Unmounting
     the reader's neighbours would be fine; unmounting anything that holds
     scroll position or focus is not, and a collapse animation on an element
     that does not exist does nothing. Width is the animated property and
     `visibility` hides the contents so collapsed panels leave the tab order.
  2. **Resizing does not go through Svelte state per frame.** A drag writes
     the width straight to the element and only commits to state (and to
     `localStorage`) on release. Reactive updates at pointer rate are what
     make a resize feel like it is fighting you.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { preferences } from '$lib/stores/preferences';

  interface Props {
    left: Snippet;
    main: Snippet;
    right: Snippet;
  }

  let { left, main, right }: Props = $props();

  let leftWidth = $state(preferences.leftWidth.load());
  let rightWidth = $state(preferences.rightWidth.load());
  let leftOpen = $state(preferences.leftOpen.load());
  let rightOpen = $state(preferences.rightOpen.load());

  /** Below this the right sidebar has no room; below the second, neither has. */
  const NARROW = 1024;
  const VERY_NARROW = 768;
  let windowWidth = $state(typeof window === 'undefined' ? 1400 : window.innerWidth);

  // Responsive collapse is derived, not written back into the preference:
  // widening the window must restore what the user chose, not what a narrow
  // window forced on them.
  const showLeft = $derived(leftOpen && windowWidth >= VERY_NARROW);
  const showRight = $derived(rightOpen && windowWidth >= NARROW);

  let leftElement: HTMLElement;
  let rightElement: HTMLElement;

  const MIN = 180;
  const MAX = 400;

  export function toggleLeft(): void {
    leftOpen = !leftOpen;
    preferences.leftOpen.save(leftOpen);
  }

  export function toggleRight(): void {
    rightOpen = !rightOpen;
    preferences.rightOpen.save(rightOpen);
  }

  export function toggleBoth(): void {
    const next = !(leftOpen && rightOpen);
    leftOpen = next;
    rightOpen = next;
    preferences.leftOpen.save(next);
    preferences.rightOpen.save(next);
  }

  function startResize(side: 'left' | 'right', event: PointerEvent): void {
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);

    const panel = side === 'left' ? leftElement : rightElement;
    const startX = event.clientX;
    const startWidth = side === 'left' ? leftWidth : rightWidth;
    let width = startWidth;

    const move = (e: PointerEvent) => {
      const delta = side === 'left' ? e.clientX - startX : startX - e.clientX;
      width = Math.min(MAX, Math.max(MIN, startWidth + delta));
      // Straight to the element: state and localStorage wait for release.
      panel.style.width = `${width}px`;
    };

    const done = () => {
      handle.releasePointerCapture(event.pointerId);
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', done);
      handle.removeEventListener('pointercancel', done);
      if (side === 'left') {
        leftWidth = width;
        preferences.leftWidth.save(width);
      } else {
        rightWidth = width;
        preferences.rightWidth.save(width);
      }
    };

    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', done);
    handle.addEventListener('pointercancel', done);
  }

  /** Keyboard resize, so the panels are operable with the pointer unplugged. */
  function keyResize(side: 'left' | 'right', event: KeyboardEvent): void {
    const step = event.shiftKey ? 32 : 8;
    let delta = 0;
    if (event.key === 'ArrowLeft') delta = side === 'left' ? -step : step;
    else if (event.key === 'ArrowRight') delta = side === 'left' ? step : -step;
    else return;

    event.preventDefault();
    if (side === 'left') {
      leftWidth = Math.min(MAX, Math.max(MIN, leftWidth + delta));
      preferences.leftWidth.save(leftWidth);
    } else {
      rightWidth = Math.min(MAX, Math.max(MIN, rightWidth + delta));
      preferences.rightWidth.save(rightWidth);
    }
  }
</script>

<svelte:window bind:innerWidth={windowWidth} />

<div class="layout">
  <aside
    bind:this={leftElement}
    class="sidebar sidebar--left"
    class:collapsed={!showLeft}
    style:width="{showLeft ? leftWidth : 0}px"
    aria-label="Library"
    aria-hidden={!showLeft}
  >
    <div class="panel">{@render left()}</div>
  </aside>

  <!-- A separator, not a button: it has a value (the width) and arrow keys
       change it, which is exactly what role="separator" describes. The
       FOCUSABLE variant of that role is the documented window-splitter
       pattern (ARIA 1.2 § separator), and it is why the handle is operable
       with the pointer unplugged. Svelte's a11y lint only knows the static
       variant, hence the ignores rather than a `<button>` that would announce
       itself as something it is not. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="handle"
    class:hidden={!showLeft}
    role="separator"
    aria-label="Resize library"
    aria-orientation="vertical"
    aria-valuenow={leftWidth}
    aria-valuemin={MIN}
    aria-valuemax={MAX}
    tabindex="0"
    onpointerdown={(e) => startResize('left', e)}
    onkeydown={(e) => keyResize('left', e)}
  ></div>

  <main>{@render main()}</main>

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="handle"
    class:hidden={!showRight}
    role="separator"
    aria-label="Resize outline"
    aria-orientation="vertical"
    aria-valuenow={rightWidth}
    aria-valuemin={MIN}
    aria-valuemax={MAX}
    tabindex="0"
    onpointerdown={(e) => startResize('right', e)}
    onkeydown={(e) => keyResize('right', e)}
  ></div>

  <aside
    bind:this={rightElement}
    class="sidebar sidebar--right"
    class:collapsed={!showRight}
    style:width="{showRight ? rightWidth : 0}px"
    aria-label="On this page"
    aria-hidden={!showRight}
  >
    <div class="panel">{@render right()}</div>
  </aside>
</div>

<style>
  .layout {
    display: flex;
    height: 100%;
    min-height: 0;
    background: var(--color-bg-primary);
  }

  .sidebar {
    flex: 0 0 auto;
    overflow: hidden;
    background: var(--color-bg-secondary);
    transition: width var(--duration-normal) var(--ease-standard);
  }

  .sidebar--left {
    border-right: 1px solid var(--color-border);
  }
  .sidebar--right {
    border-left: 1px solid var(--color-border);
  }

  /* Zero-width, not unmounted — but `visibility: hidden` so the contents
     leave the tab order. A collapsed panel whose links are still focusable
     is a keyboard trap that nothing on screen explains. */
  .sidebar.collapsed {
    border-color: transparent;
    visibility: hidden;
  }

  .panel {
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
  }

  main {
    flex: 1 1 0;
    min-width: 0;
    min-height: 0;
  }

  .handle {
    flex: 0 0 auto;
    width: 5px;
    margin-inline: -2px;
    cursor: col-resize;
    /* Above the panels so the whole 5px strip is grabbable, but with no
       paint of its own — the panel borders draw the visible line. */
    position: relative;
    z-index: 1;
    background: transparent;
  }

  .handle:hover,
  .handle:focus-visible {
    background: var(--color-focus);
  }

  .handle.hidden {
    display: none;
  }
</style>
