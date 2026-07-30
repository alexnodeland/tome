/**
 * Keystrokes → Tauri accelerators, and the combinations to refuse (P5-009).
 *
 * **The important finding, measured in SPIKE-001:** registering a shortcut
 * macOS itself reserves *succeeds*. `CmdOrCtrl+Space` registers cleanly, and
 * the handler then never fires, because the system consumes the keystroke
 * before any application sees it. `RegisterEventHotKey` only refuses a
 * combination held by another application's hotkey — not one held by macOS —
 * and there is no API that lists either.
 *
 * So "it registered" is not evidence that it works, and the only way a user
 * finds out is by pressing it and watching nothing happen. This module is the
 * half of conflict detection that Rust cannot do: refuse the combinations that
 * are known to be dead before they are stored.
 */

/** Shortcuts macOS reserves. Registering one succeeds and never fires. */
const RESERVED: Record<string, string> = {
  'CmdOrCtrl+Space': 'Spotlight',
  'CmdOrCtrl+Alt+Space': 'Finder search',
  'Control+CmdOrCtrl+Space': 'the Character Viewer',
  'CmdOrCtrl+Tab': 'the application switcher',
  'CmdOrCtrl+Shift+3': 'screenshots',
  'CmdOrCtrl+Shift+4': 'screenshots',
  'CmdOrCtrl+Shift+5': 'screen recording',
  'CmdOrCtrl+Shift+Q': 'log out',
  'Control+ArrowUp': 'Mission Control',
  'Control+ArrowDown': 'application windows',
  'Control+ArrowLeft': 'the previous desktop',
  'Control+ArrowRight': 'the next desktop',
};

/**
 * Why this combination cannot be used, or null if it can.
 *
 * Two rules. The reserved list is the measured one. The two-modifier
 * requirement is the general one: a global `⌘K` overrides the frontmost
 * application's own ⌘K everywhere, in every app, for as long as Tome is
 * running — which is almost never what someone means by "summon Tome".
 */
export function unusableBecause(accelerator: string): string | null {
  const reserved = RESERVED[accelerator];
  if (reserved) {
    return `macOS uses that for ${reserved}. It would register and never fire.`;
  }
  const modifiers = accelerator.split('+').length - 1;
  if (modifiers < 2) {
    return 'Use at least two modifiers. A single-modifier shortcut overrides that key in every application while Tome is running.';
  }
  return null;
}

/**
 * Turn a keystroke into a Tauri accelerator, or null if it is not one yet.
 *
 * Null for a modifier held on its own — the user is on the way to a shortcut,
 * not finished — and for a keystroke with no modifier at all, which would
 * swallow that key system-wide.
 */
export function accelerator(event: KeyboardEvent): string | null {
  const parts: string[] = [];
  if (event.metaKey) parts.push('CmdOrCtrl');
  // Control is only a distinct modifier when Command is not held: on macOS
  // `CmdOrCtrl` already means Command, and emitting both would produce an
  // accelerator naming Command twice.
  if (event.ctrlKey && !event.metaKey) parts.push('Control');
  if (event.altKey) parts.push('Alt');
  if (event.shiftKey) parts.push('Shift');
  if (parts.length === 0) return null;

  if (['Meta', 'Control', 'Alt', 'Shift'].includes(event.key)) return null;

  // `event.code` rather than `event.key` for letters and digits. With Alt
  // held, macOS reports `key` as the composed character — Alt+D is `∂` — and
  // Tauri's parser has no idea what to do with that. With Shift held, a digit
  // arrives as its symbol.
  const code = event.code;
  let named = event.key.toUpperCase();
  if (/^Key[A-Z]$/.test(code)) named = code.slice(3);
  else if (/^Digit[0-9]$/.test(code)) named = code.slice(5);
  else if (code === 'Space') named = 'Space';
  else if (/^Arrow(Up|Down|Left|Right)$/.test(code)) named = code;
  return [...parts, named].join('+');
}

/** `CmdOrCtrl+Shift+D` as `⌘⇧D`, which is what the key caps say. */
export function pretty(value: string): string {
  return value
    .split('+')
    .map((part) => {
      switch (part) {
        case 'CmdOrCtrl':
        case 'Command':
        case 'Super':
          return '⌘';
        case 'Shift':
          return '⇧';
        case 'Alt':
        case 'Option':
          return '⌥';
        case 'Control':
        case 'Ctrl':
          return '⌃';
        case 'ArrowUp':
          return '↑';
        case 'ArrowDown':
          return '↓';
        case 'ArrowLeft':
          return '←';
        case 'ArrowRight':
          return '→';
        default:
          return part;
      }
    })
    .join('');
}
