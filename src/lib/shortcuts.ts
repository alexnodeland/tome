/**
 * The keyboard shortcuts this build actually binds.
 *
 * **[PRD Appendix C](../../docs/PRD.md#appendix-c-keyboard-shortcut-reference)
 * is the canonical list** and covers the whole product, including features
 * that do not exist yet — bookmarks, annotations, sync. This is the subset
 * that is wired up, and it exists so that the onboarding overview and the
 * Preferences keyboard tab show a person what will happen when they press a
 * key rather than what was planned.
 *
 * A shortcuts panel that lists ⌘D for "bookmark page" when nothing is bound
 * to ⌘D teaches the user something false, and they find out by pressing it.
 * When a feature lands, its row moves here in the same change.
 */

export interface Shortcut {
  keys: string;
  action: string;
}

export interface ShortcutGroup {
  title: string;
  shortcuts: Shortcut[];
}

export const SHORTCUTS: ShortcutGroup[] = [
  {
    title: 'Search',
    shortcuts: [
      { keys: '⌘K', action: 'Search everything' },
      { keys: '⌘F', action: 'Find in this page' },
      { keys: '⌘G', action: 'Next match' },
      { keys: '⇧⌘G', action: 'Previous match' },
    ],
  },
  {
    title: 'Navigation',
    shortcuts: [
      { keys: '⌘[', action: 'Back' },
      { keys: '⌘]', action: 'Forward' },
    ],
  },
  {
    title: 'View',
    shortcuts: [
      { keys: '⌘1', action: 'Toggle the library sidebar' },
      { keys: '⌘2', action: 'Toggle the outline sidebar' },
      { keys: '⌘\\', action: 'Toggle both sidebars' },
      { keys: '⌘=', action: 'Larger text' },
      { keys: '⌘-', action: 'Smaller text' },
      { keys: '⌘0', action: 'Reset text size' },
    ],
  },
  {
    title: 'App',
    shortcuts: [{ keys: '⌘,', action: 'Preferences' }],
  },
];
