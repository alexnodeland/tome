import { describe, it, expect, beforeEach } from 'vitest';
import {
  appearanceAttributes,
  applyToDocument,
  loadAppearance,
  saveAppearance,
} from './appearance';

describe('appearance', () => {
  beforeEach(() => {
    localStorage.clear();
    for (const name of ['data-theme', 'data-text-size', 'data-measure', 'data-line-numbers']) {
      document.documentElement.removeAttribute(name);
    }
  });

  it('expresses every default by removing the attribute, never by a value', () => {
    // `data-theme="system"` would need a CSS rule duplicating the media query
    // that already handles it, and `data-text-size="default"` would need one
    // duplicating the root font size. Both are the kind of duplication that
    // drifts. Absence is the default.
    expect(appearanceAttributes(loadAppearance())).toEqual({
      'data-theme': null,
      'data-text-size': null,
      'data-measure': null,
      'data-line-numbers': null,
    });
  });

  it('maps non-default choices onto the attributes tokens.css selects on', () => {
    expect(
      appearanceAttributes({
        theme: 'dark',
        textSize: 'large',
        measure: 'narrow',
        lineNumbers: true,
      }),
    ).toEqual({
      'data-theme': 'dark',
      'data-text-size': 'large',
      'data-measure': 'narrow',
      'data-line-numbers': 'true',
    });
  });

  it('applies to the document and clears what it should', () => {
    applyToDocument({ theme: 'dark', textSize: 'small', measure: 'wide', lineNumbers: true });
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    expect(document.documentElement.getAttribute('data-measure')).toBe('wide');

    applyToDocument({
      theme: 'system',
      textSize: 'default',
      measure: 'default',
      lineNumbers: false,
    });
    // Not `data-theme="system"` — see above.
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
    expect(document.documentElement.hasAttribute('data-line-numbers')).toBe(false);
  });

  it('round-trips through storage', () => {
    saveAppearance({ theme: 'light', textSize: 'xlarge', measure: 'narrow', lineNumbers: true });
    expect(loadAppearance()).toEqual({
      theme: 'light',
      textSize: 'xlarge',
      measure: 'narrow',
      lineNumbers: true,
    });
  });

  it('ignores a stored value that is not one of the allowed ones', () => {
    // A hand-edited store, or something an older build wrote. Without this the
    // attribute would be set to a value no CSS rule matches, leaving the app
    // in the default theme while the preference reads as changed — which looks
    // like a broken theme rather than a rejected value.
    localStorage.setItem('tome.appearance.theme', 'solarized');
    localStorage.setItem('tome.appearance.textSize', '48px');
    expect(loadAppearance().theme).toBe('system');
    expect(loadAppearance().textSize).toBe('default');
  });
});
