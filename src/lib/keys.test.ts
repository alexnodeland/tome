import { describe, expect, it } from 'vitest';
import { isCommand, isCommandShift } from './keys';

function key(init: Partial<KeyboardEventInit> & { key: string }): KeyboardEvent {
  return new KeyboardEvent('keydown', init);
}

describe('isCommand', () => {
  it('matches the primary modifier plus the key', () => {
    expect(isCommand(key({ key: 'k', metaKey: true }), 'k')).toBe(true);
    // ctrl too, so the shortcuts still work in a browser on another platform
    // during development.
    expect(isCommand(key({ key: 'k', ctrlKey: true }), 'k')).toBe(true);
  });

  it('does not match without a modifier', () => {
    expect(isCommand(key({ key: 'k' }), 'k')).toBe(false);
  });

  it('does not match a different key', () => {
    expect(isCommand(key({ key: 'j', metaKey: true }), 'k')).toBe(false);
  });

  it('does not match when Alt is held', () => {
    expect(isCommand(key({ key: 'k', metaKey: true, altKey: true }), 'k')).toBe(false);
  });

  it('does not match when Shift is held', () => {
    // Shift is excluded rather than ignored: ⌘G and ⇧⌘G are different
    // actions in Appendix C, and a check that accepted both would make them
    // indistinguishable.
    expect(isCommand(key({ key: 'G', metaKey: true, shiftKey: true }), 'g')).toBe(false);
    expect(isCommand(key({ key: '1', metaKey: true, shiftKey: true }), '1')).toBe(false);
  });
});

describe('isCommandShift', () => {
  it('matches the shifted form even though event.key is uppercase', () => {
    // The whole reason this helper exists. With Shift held, `event.key` for
    // the G key is 'G', so `event.key === 'g'` never fires and ⇧⌘G silently
    // does nothing.
    expect(isCommandShift(key({ key: 'G', metaKey: true, shiftKey: true }), 'g')).toBe(true);
  });

  it('does not match without Shift', () => {
    expect(isCommandShift(key({ key: 'g', metaKey: true }), 'g')).toBe(false);
  });

  it('does not match without the primary modifier', () => {
    expect(isCommandShift(key({ key: 'G', shiftKey: true }), 'g')).toBe(false);
  });

  it('does not match when Alt is held', () => {
    expect(
      isCommandShift(key({ key: 'G', metaKey: true, shiftKey: true, altKey: true }), 'g'),
    ).toBe(false);
  });
});

describe('the two together', () => {
  it('are mutually exclusive, so one event never fires two actions', () => {
    for (const event of [
      key({ key: 'g', metaKey: true }),
      key({ key: 'G', metaKey: true, shiftKey: true }),
      key({ key: 'g' }),
    ]) {
      expect(isCommand(event, 'g') && isCommandShift(event, 'g')).toBe(false);
    }
  });
});
