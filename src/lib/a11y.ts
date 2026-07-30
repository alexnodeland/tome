/**
 * Accessibility helpers (S4-7).
 *
 * One thing so far, because one thing was actually missing. The audit found
 * the rest already in place: the panels are landmarks with labels, a collapsed
 * sidebar is `visibility: hidden` so it leaves the tab order rather than
 * merely being `aria-hidden` while still focusable, the search results are a
 * real `listbox`/`option` tree with `aria-activedescendant`, errors are
 * `role="alert"`, loading states are `aria-live`, and both stylesheets honour
 * `prefers-reduced-motion`.
 */

/**
 * Keep Tab inside a container while it is open.
 *
 * `aria-modal="true"` tells assistive technology that the rest of the page is
 * inert. **It does nothing to the keyboard.** Without this, Tab from the last
 * control in the search modal moves into the shell behind it — the focus ring
 * disappears behind an overlay, and the next Return activates something the
 * user cannot see. That is the whole defect, and it is invisible to anyone
 * using a pointer.
 *
 * Used as a Svelte action:
 *
 * ```svelte
 * <div role="dialog" aria-modal="true" use:trapFocus>
 * ```
 *
 * It does not move focus on mount — the components already decide what should
 * be focused, and a trap that also grabbed focus would fight them.
 */
export function trapFocus(node: HTMLElement) {
  function keydown(event: KeyboardEvent): void {
    if (event.key !== 'Tab') return;

    const focusable = tabbable(node);
    // Nothing to cycle between: swallow the keystroke rather than letting it
    // escape to the shell behind. An empty dialog is a bug, but focus landing
    // behind an overlay is a worse one.
    if (focusable.length === 0) {
      event.preventDefault();
      return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;
    // Both modals park focus on the container itself when they open
    // (`tabindex="-1"`), so this is the state most Tab presses start from.
    // The browser's document-order fallback would usually land on the right
    // element anyway; handling it explicitly means the first Tab is
    // deterministic rather than dependent on what happens to follow the
    // dialog in the DOM.
    const atEdge = active === node || !node.contains(active);

    // Only the two edges are intercepted. Everything in between is the
    // browser's own tab order, which already handles `tabindex`, disabled
    // controls, and shadow roots better than a reimplementation would.
    if (event.shiftKey && (active === first || atEdge)) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && (active === last || atEdge)) {
      event.preventDefault();
      first?.focus();
    }
  }

  node.addEventListener('keydown', keydown);
  return {
    destroy() {
      node.removeEventListener('keydown', keydown);
    },
  };
}

/**
 * The elements inside `root` that Tab would visit.
 *
 * Deliberately a selector rather than a walk: the alternative is checking
 * computed styles on every node, which is a layout read per element and runs
 * on every Tab press.
 */
function tabbable(root: HTMLElement): HTMLElement[] {
  const selector = [
    'a[href]',
    'button:not([disabled])',
    'input:not([disabled])',
    'select:not([disabled])',
    'textarea:not([disabled])',
    '[tabindex]:not([tabindex="-1"])',
  ].join(',');

  return [...root.querySelectorAll<HTMLElement>(selector)].filter((element) => {
    // `hidden` and `display: none` do not match `:not([disabled])`, and an
    // element inside a collapsed section is still in the DOM. `offsetParent`
    // is null for both, and for `visibility: hidden` — which is exactly how
    // the layout collapses a sidebar.
    if (element.hasAttribute('hidden')) return false;
    return element.offsetParent !== null || element === document.activeElement;
  });
}
