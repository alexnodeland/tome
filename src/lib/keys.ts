/**
 * Keyboard shortcut helpers (S1-14).
 *
 * **Canonical list: [PRD Appendix C](../../docs/PRD.md#appendix-c-keyboard-shortcut-reference).**
 * Not restated here — four partially-contradictory copies previously existed
 * across the plan set, two of which shadowed macOS system shortcuts.
 *
 * What lives here is the one rule that has to be code rather than a table.
 */

/**
 * Whether a keystroke should be ignored because the user is typing.
 *
 * The PRD's first shortcut rule: single-letter reading keys (`J`, `K`, `G`,
 * `[`, `]`) bind on the reader surface only, and every handler bails if the
 * focused element takes text. Without it, typing "j" in the source filter box
 * scrolls the document — which is not a hypothetical, it is what the box
 * added in this very ticket would have caused.
 *
 * Modifier combinations (`Cmd+1`) are exempt: they do not collide with typing
 * and are also in the menu bar, which is how macOS users find them.
 */
export function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
}

/** Whether `event` is the platform's primary modifier plus `key`. */
export function isCommand(event: KeyboardEvent, key: string): boolean {
  // `metaKey` on macOS. Tome ships macOS only, but `ctrlKey` costs nothing
  // and means the shortcuts still work if someone runs the dev server in a
  // browser on another platform.
  return (event.metaKey || event.ctrlKey) && !event.altKey && event.key === key;
}
