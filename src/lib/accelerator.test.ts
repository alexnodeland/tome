import { describe, it, expect } from 'vitest';
import { accelerator, pretty, unusableBecause } from './accelerator';

/** A KeyboardEvent with just the fields the parser reads. */
function key(init: Partial<KeyboardEvent> & { key: string; code?: string }): KeyboardEvent {
  return new KeyboardEvent('keydown', { code: init.key, ...init });
}

describe('accelerator', () => {
  it('builds a Tauri accelerator from modifiers plus a key', () => {
    expect(accelerator(key({ key: 'd', code: 'KeyD', metaKey: true, shiftKey: true }))).toBe(
      'CmdOrCtrl+Shift+D',
    );
  });

  it('reads letters from `code`, not `key`', () => {
    // With Alt held, macOS reports `key` as the composed character — Alt+D is
    // `∂` — and Tauri's parser has no idea what to do with that.
    expect(accelerator(key({ key: '∂', code: 'KeyD', metaKey: true, altKey: true }))).toBe(
      'CmdOrCtrl+Alt+D',
    );
    // And digits arrive as their shifted symbol.
    expect(accelerator(key({ key: '#', code: 'Digit3', metaKey: true, shiftKey: true }))).toBe(
      'CmdOrCtrl+Shift+3',
    );
  });

  it('does not name Command twice when Control and Command are both held', () => {
    // `CmdOrCtrl` already means Command on macOS.
    expect(accelerator(key({ key: 'k', code: 'KeyK', metaKey: true, ctrlKey: true }))).toBe(
      'CmdOrCtrl+K',
    );
  });

  it('returns null for a modifier still being held', () => {
    // The user is on the way to a shortcut, not finished. Treating this as a
    // rejection would flash an error on every attempt.
    expect(accelerator(key({ key: 'Meta', metaKey: true }))).toBeNull();
    expect(accelerator(key({ key: 'Shift', shiftKey: true }))).toBeNull();
  });

  it('returns null for a keystroke with no modifier at all', () => {
    expect(accelerator(key({ key: 'd', code: 'KeyD' }))).toBeNull();
  });
});

describe('unusableBecause', () => {
  it('refuses combinations macOS reserves', () => {
    // SPIKE-001 measured this: `CmdOrCtrl+Space` registers *successfully* and
    // the handler then never fires, because the system consumes the keystroke
    // first. Registration failure is therefore not conflict detection, and
    // this list is the half of it that Rust cannot do.
    expect(unusableBecause('CmdOrCtrl+Space')).toMatch(/Spotlight/);
    expect(unusableBecause('CmdOrCtrl+Shift+3')).toMatch(/screenshots/);
    expect(unusableBecause('Control+ArrowRight')).toMatch(/desktop/);
  });

  it('requires at least two modifiers', () => {
    // A global ⌘K overrides the frontmost application's own ⌘K everywhere,
    // in every app, for as long as Tome is running.
    expect(unusableBecause('CmdOrCtrl+K')).toMatch(/two modifiers/);
    expect(unusableBecause('CmdOrCtrl+Shift+K')).toBeNull();
  });

  it('accepts the default', () => {
    expect(unusableBecause('CmdOrCtrl+Shift+D')).toBeNull();
  });
});

describe('pretty', () => {
  it('shows what the key caps say', () => {
    expect(pretty('CmdOrCtrl+Shift+D')).toBe('⌘⇧D');
    expect(pretty('Control+Alt+ArrowUp')).toBe('⌃⌥↑');
  });
});
