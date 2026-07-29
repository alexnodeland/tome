/**
 * Keyboard shortcut helpers.
 *
 * **Canonical list: [PRD Appendix C](../../docs/PRD.md#appendix-c-keyboard-shortcut-reference).**
 * Not restated here — four partially-contradictory copies previously existed
 * across the plan set, two of which shadowed macOS system shortcuts.
 *
 * # Why there is no `isTyping` guard here
 *
 * The PRD's first shortcut rule is that single-letter reading keys (`J`, `K`,
 * `G`, `[`, `]`) must never fire while a text field has focus — otherwise
 * typing "j" in the library filter scrolls the document. S1-14 added such a
 * guard here, and S1-15 removed it, because **nothing in the app webview can
 * use it**: the app binds only modifier combinations, which do not collide
 * with typing, and the reading keys scroll the *reader*, whose document lives
 * in a sandboxed iframe the app cannot reach.
 *
 * So when those keys are implemented they belong in `public/reader-frame.js`,
 * with the guard written against *that* document's `activeElement`. A helper
 * here would have been checking the wrong document. It is written down rather
 * than silently deleted so the rule is not lost with the code.
 */

/**
 * Whether `event` is the platform's primary modifier plus `key`, with no
 * Shift.
 *
 * **Shift is excluded, not ignored.** Appendix C binds ⌘G and ⇧⌘G to
 * different actions (next and previous match), so a check that accepted both
 * would make the two indistinguishable — and it would also fire ⌘1 on ⇧⌘1,
 * which is not a shortcut at all.
 */
export function isCommand(event: KeyboardEvent, key: string): boolean {
  return matches(event, key) && !event.shiftKey;
}

/**
 * Whether `event` is the primary modifier plus Shift plus `key`.
 *
 * The case comparison is the point. With Shift held, `event.key` for the G
 * key is `'G'`, not `'g'` — so a naive `event.key === 'g'` never fires for
 * ⇧⌘G, and the shortcut silently does nothing. Appendix C has three
 * Shift-bearing shortcuts and every one of them would have had this bug.
 */
export function isCommandShift(event: KeyboardEvent, key: string): boolean {
  return matches(event, key) && event.shiftKey;
}

function matches(event: KeyboardEvent, key: string): boolean {
  // `metaKey` on macOS. Tome ships macOS only, but `ctrlKey` costs nothing
  // and means the shortcuts still work if someone runs the dev server in a
  // browser on another platform.
  return (
    (event.metaKey || event.ctrlKey) &&
    !event.altKey &&
    event.key.toLowerCase() === key.toLowerCase()
  );
}
