#!/usr/bin/env node
//
// Contrast and parity gate for the design tokens — implementation plan S1-12,
// specified by docs/plans/15-design-system.md § "Automated contrast checking".
//
// That section exists because the plan review found two token pairs below
// 4.5:1 in a document that names accessibility as a principle. Colour is the
// one part of a design system that cannot be reviewed by looking at it: the
// failures were "grey on off-white" and "grey on near-black", both of which
// look fine and neither of which passes. This is the five lines of CI it
// asked for.
//
// It checks four things, and the last three matter as much as the first:
//
//   1. Every foreground/background pair that can legitimately combine meets
//      WCAG 2.1 — 4.5:1 for text, 3:1 for UI boundaries — in BOTH themes.
//   2. Light and dark define the same set of colour tokens. A token with no
//      dark variant silently uses its light value on a dark background, which
//      is exactly what the original stylesheet did with the status colours.
//   3. The `@media (prefers-color-scheme: dark)` block and the
//      `:root[data-theme="dark"]` override are byte-identical. They have to
//      be duplicated — a media query cannot honour an explicit user choice,
//      and an override that only wins in one direction is worse than none —
//      and duplication that nothing checks is duplication that drifts.
//   4. Every `var(--token)` in a consuming stylesheet or component resolves.
//      A misspelt custom property is not a CSS error — it silently falls back
//      to the property's initial value — and `public/reader.css` is loaded by
//      an iframe that does not exist until S1-13, so nothing else would catch
//      a typo in it.
//
// No dependencies: it parses the one file it is about. A CSS parser would be
// a dependency added to read a file this project writes.

import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const TOKENS = join(root, 'public/tokens.css');

// Pairs that can legitimately combine, with the ratio each must meet: 4.5:1
// for text (WCAG 1.4.3), 3:1 for a non-text boundary that carries meaning
// (1.4.11).
//
// What is deliberately NOT asserted, and why — these are judgements, so they
// are written down rather than left as absences:
//
//   * **Panel dividers** (`--color-border` on a background) measure ~1.2:1.
//     1.4.11 governs boundaries *needed to identify a component*; a hairline
//     between two panels that are already distinguished by their fills is
//     decoration. Forcing 3:1 would darken every edge in the app to satisfy
//     a clause it does not fall under. The design system's checklist said
//     "UI borders meet 3:1", which is too broad to be true — it now says
//     which boundaries.
//   * **Status fills** (`--color-success` and friends) measure 2.2:1 on
//     light. They are legitimate only because the design system forbids
//     colour as the sole carrier of meaning: the shape carries it, the
//     colour reinforces. Coloured *words* are a different thing and use the
//     `--color-*-text` tokens, which are asserted below.
const PAIRS = [
  // Text on each surface it can sit on.
  ['--color-text-primary', '--color-bg-primary', 4.5],
  ['--color-text-secondary', '--color-bg-primary', 4.5],
  ['--color-text-tertiary', '--color-bg-primary', 4.5],
  ['--color-text-primary', '--color-bg-secondary', 4.5],
  ['--color-text-primary', '--color-bg-tertiary', 4.5],
  ['--color-text-inverse', '--color-accent', 4.5],

  // Links are most of a documentation page.
  ['--color-link', '--color-bg-primary', 4.5],
  ['--color-link', '--color-bg-secondary', 4.5],
  ['--color-link', '--color-bg-tertiary', 4.5],

  // Coloured words: admonition titles, error messages, sync status labels.
  ['--color-success-text', '--color-bg-primary', 4.5],
  ['--color-warning-text', '--color-bg-primary', 4.5],
  ['--color-error-text', '--color-bg-primary', 4.5],
  ['--color-success-text', '--color-bg-tertiary', 4.5],
  ['--color-warning-text', '--color-bg-tertiary', 4.5],
  ['--color-error-text', '--color-bg-tertiary', 4.5],

  // The focus ring IS a boundary that carries meaning, on every surface it
  // can appear over. This is the pair that forced --color-focus apart from
  // --color-accent.
  ['--color-focus', '--color-bg-primary', 3.0],
  ['--color-focus', '--color-bg-secondary', 3.0],
  ['--color-focus', '--color-bg-tertiary', 3.0],

  // Code sits on its own surface, not on the page background.
  ['--color-code-text', '--color-code-bg', 4.5],
  ['--color-text-tertiary', '--color-code-bg', 4.5], // line numbers

  // Syntax tokens, all against the code surface. These are the ones most
  // likely to have been chosen by eye, and so most likely to be wrong.
  ['--tok-comment', '--color-code-bg', 4.5],
  ['--tok-string', '--color-code-bg', 4.5],
  ['--tok-keyword', '--color-code-bg', 4.5],
  ['--tok-constant', '--color-code-bg', 4.5],
  ['--tok-entity', '--color-code-bg', 4.5],
  ['--tok-support', '--color-code-bg', 4.5],
  ['--tok-variable', '--color-code-bg', 4.5],
  ['--tok-punctuation', '--color-code-bg', 4.5],
  ['--tok-invalid', '--tok-invalid-bg', 4.5],

  // Body text over each highlight wash. A saturated yellow fails this and
  // makes highlighted text the least readable text on the page.
  ['--color-text-primary', '--color-highlight-yellow', 4.5],
  ['--color-text-primary', '--color-highlight-green', 4.5],
  ['--color-text-primary', '--color-highlight-blue', 4.5],
  ['--color-text-primary', '--color-highlight-pink', 4.5],
];

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/** Declarations inside the first block whose selector line matches `pattern`. */
function block(css, pattern, { inMediaQuery = false } = {}) {
  const source = inMediaQuery ? darkMediaQuery(css) : stripMediaQueries(css);
  const at = source.search(pattern);
  if (at === -1) return null;
  const open = source.indexOf('{', at);
  const close = source.indexOf('}', open);
  if (open === -1 || close === -1) return null;
  return declarations(source.slice(open + 1, close));
}

/** The body of `@media (prefers-color-scheme: dark) { ... }`. */
function darkMediaQuery(css) {
  const at = css.indexOf('@media (prefers-color-scheme: dark)');
  if (at === -1) return '';
  // Balance braces from the query's own opening one, so the nested `:root`
  // block is not mistaken for the end of the query.
  const open = css.indexOf('{', at);
  let depth = 0;
  for (let i = open; i < css.length; i++) {
    if (css[i] === '{') depth++;
    else if (css[i] === '}' && --depth === 0) return css.slice(open + 1, i);
  }
  return '';
}

/** Everything outside any `@media` block. */
function stripMediaQueries(css) {
  let out = '';
  for (let i = 0; i < css.length;) {
    const at = css.indexOf('@media', i);
    if (at === -1) {
      out += css.slice(i);
      break;
    }
    out += css.slice(i, at);
    const open = css.indexOf('{', at);
    let depth = 0;
    let j = open;
    for (; j < css.length; j++) {
      if (css[j] === '{') depth++;
      else if (css[j] === '}' && --depth === 0) break;
    }
    i = j + 1;
  }
  return out;
}

function declarations(body) {
  const out = new Map();
  // Comments come out FIRST, before splitting on `;`. This file's comments
  // are prose about contrast ratios and contain colons ("WCAG 2.1:",
  // "16.8:1"), so a declaration preceded by one would otherwise parse its
  // name out of the comment and be silently skipped — which is exactly what
  // this script's first run did, reporting a dozen tokens as undefined.
  const source = body.replace(/\/\*[\s\S]*?\*\//g, '');
  for (const declaration of source.split(';')) {
    const at = declaration.indexOf(':');
    if (at === -1) continue;
    const name = declaration.slice(0, at).trim();
    if (!name.startsWith('--')) continue;
    out.set(name, declaration.slice(at + 1).trim());
  }
  return out;
}

// ---------------------------------------------------------------------------
// Contrast
// ---------------------------------------------------------------------------

function rgb(hex) {
  const m = /^#([0-9a-f]{6})$/i.exec(hex);
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** Relative luminance, WCAG 2.1 definition. */
function luminance([r, g, b]) {
  const channel = (v) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

function contrast(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

// ---------------------------------------------------------------------------

const css = readFileSync(TOKENS, 'utf8');

const themes = {
  light: block(css, /:root,\s*:root\[data-theme='light'\]/),
  dark: block(css, /:root\s*\{/, { inMediaQuery: true }),
};
const darkOverride = block(css, /:root\[data-theme='dark'\]/);

const failures = [];

for (const [name, tokens] of Object.entries(themes)) {
  if (!tokens || tokens.size === 0) {
    failures.push(`could not find the ${name} token block in public/tokens.css`);
  }
}
if (!darkOverride || darkOverride.size === 0) {
  failures.push("could not find the :root[data-theme='dark'] block");
}

if (failures.length === 0) {
  // 2. Light and dark define the same colour tokens.
  const colourish = (name) => name.startsWith('--color-') || name.startsWith('--tok-');
  for (const name of themes.light.keys()) {
    if (colourish(name) && !themes.dark.has(name)) {
      failures.push(
        `${name} has no dark variant — it would use its light value on a dark background`,
      );
    }
  }
  for (const name of themes.dark.keys()) {
    if (colourish(name) && !themes.light.has(name)) {
      failures.push(`${name} is defined only in dark`);
    }
  }

  // 3. The media query and the explicit override agree.
  for (const [name, value] of themes.dark) {
    if (!darkOverride.has(name)) {
      failures.push(`${name} is in the dark media query but not in :root[data-theme='dark']`);
    } else if (darkOverride.get(name) !== value) {
      failures.push(
        `${name} drifted: media query has ${value}, [data-theme='dark'] has ${darkOverride.get(name)}`,
      );
    }
  }
  for (const name of darkOverride.keys()) {
    if (colourish(name) && !themes.dark.has(name)) {
      failures.push(`${name} is in :root[data-theme='dark'] but not in the dark media query`);
    }
  }

  // 1. Contrast.
  for (const [theme, tokens] of Object.entries(themes)) {
    for (const [fg, bg, required] of PAIRS) {
      const fgHex = tokens.get(fg);
      const bgHex = tokens.get(bg);
      if (!fgHex || !bgHex) {
        failures.push(`${theme}: ${fg} or ${bg} is not defined`);
        continue;
      }
      const [a, b] = [rgb(fgHex), rgb(bgHex)];
      if (!a || !b) {
        failures.push(`${theme}: ${fg} (${fgHex}) or ${bg} (${bgHex}) is not a #rrggbb literal`);
        continue;
      }
      const ratio = contrast(a, b);
      if (ratio < required) {
        failures.push(
          `${theme}: ${fg} (${fgHex}) on ${bg} (${bgHex}) is ${ratio.toFixed(2)}:1, needs ${required}:1`,
        );
      }
    }
  }
}

// 4. Every `var(--token)` a consumer references is actually defined.
//
// `var(--color-accent-hovr)` is not a CSS error; it silently resolves to
// nothing and the property falls back to its initial value — usually black
// text, or no background. The reader's stylesheet is loaded by a sandboxed
// iframe that S1-13 wires up, so until then it is a file no browser has
// opened; without this check a typo in it would sit undetected until someone
// noticed the reader looked wrong.
const CONSUMERS = [
  'public/reader.css',
  'src/app.css',
  'index.html',
  // Every Svelte component's `<style>` block. Scoped styles are still styles,
  // and a component is the most likely place for a token name to be typed
  // from memory.
  ...readdirSync(join(root, 'src'), { recursive: true })
    .filter((name) => String(name).endsWith('.svelte'))
    .map((name) => join('src', String(name))),
];

if (failures.length === 0) {
  // Every custom property DEFINED anywhere in the token file — the theme
  // blocks plus the type scale, spacing, and motion in the plain `:root`.
  // Matched by "name followed by a colon", which a `var(--name)` usage never
  // is (it is followed by `)` or `,`). Splitting the whole file on `;` the
  // way the per-block parser does would not work here: a chunk spanning a
  // `:root {` selector finds that colon first and skips the declaration.
  const definitionsIn = (source) =>
    new Set(
      [...source.replace(/\/\*[\s\S]*?\*\//g, '').matchAll(/(--[a-z0-9-]+)\s*:/gi)].map(
        (m) => m[1],
      ),
    );

  const defined = definitionsIn(css);

  for (const relative of CONSUMERS) {
    let source;
    try {
      source = readFileSync(join(root, relative), 'utf8');
    } catch {
      continue; // Optional consumer; a missing file is not a token problem.
    }
    // Names the file defines for itself are legitimate too.
    const local = definitionsIn(source);
    for (const match of source.matchAll(/var\(\s*(--[a-z0-9-]+)/gi)) {
      const name = match[1];
      if (!defined.has(name) && !local.has(name)) {
        failures.push(`${relative} references ${name}, which no token block defines`);
      }
    }
  }
}

if (failures.length > 0) {
  console.error(`design tokens: ${failures.length} problem(s)\n`);
  for (const failure of failures) console.error(`  · ${failure}`);
  console.error('');
  process.exit(1);
}

const checked = Object.keys(themes).length * PAIRS.length;
console.log(
  `design tokens: ${checked} contrast pairs pass, light and dark are in parity, ` +
    `every var() in ${CONSUMERS.length} consumers resolves.`,
);
