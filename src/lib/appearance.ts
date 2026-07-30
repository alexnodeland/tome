/**
 * Appearance preferences → `<html>` attributes (P5-007).
 *
 * One module, because the same attributes have to reach two documents: the
 * app shell and the sandboxed reader frame, which has an opaque origin and
 * cannot share the app's cascade. `public/tokens.css` is linked by both for
 * exactly that reason, and this is its runtime half — a second copy of these
 * mappings would produce one theme in the chrome and another in the page,
 * which is the drift the shared token file exists to prevent.
 *
 * **Attributes, not inline styles.** Theme, text size and measure are all
 * CSS-driven, so applying a preference is one attribute and no re-render:
 * switching theme costs nothing even on a page with two hundred highlighted
 * code blocks, and nothing is re-highlighted (highlighting emits classes, not
 * colours).
 *
 * `null` means "remove the attribute", which is how a default is expressed:
 * `data-theme` absent lets `prefers-color-scheme` decide, and inventing
 * `data-theme="system"` would need a CSS rule that duplicated the media query.
 */
import { preferences, type Measure, type TextSize, type Theme } from '$lib/stores/preferences';

export interface Appearance {
  theme: Theme;
  textSize: TextSize;
  measure: Measure;
  lineNumbers: boolean;
}

export function loadAppearance(): Appearance {
  return {
    theme: preferences.theme.load(),
    textSize: preferences.textSize.load(),
    measure: preferences.measure.load(),
    lineNumbers: preferences.lineNumbers.load(),
  };
}

export function saveAppearance(appearance: Appearance): void {
  preferences.theme.save(appearance.theme);
  preferences.textSize.save(appearance.textSize);
  preferences.measure.save(appearance.measure);
  preferences.lineNumbers.save(appearance.lineNumbers);
}

/** The attribute set both documents are given. */
export function appearanceAttributes(appearance: Appearance): Record<string, string | null> {
  return {
    'data-theme': appearance.theme === 'system' ? null : appearance.theme,
    'data-text-size': appearance.textSize === 'default' ? null : appearance.textSize,
    'data-measure': appearance.measure === 'default' ? null : appearance.measure,
    // Reader-only, and harmless on the shell: the app chrome contains no code
    // blocks for the rule to select.
    'data-line-numbers': appearance.lineNumbers ? 'true' : null,
  };
}

/** Apply to the app shell's own document. */
export function applyToDocument(appearance: Appearance): void {
  const root = document.documentElement;
  for (const [name, value] of Object.entries(appearanceAttributes(appearance))) {
    if (value === null) root.removeAttribute(name);
    else root.setAttribute(name, value);
  }
}
