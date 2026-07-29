import js from '@eslint/js';
import ts from 'typescript-eslint';
import svelte from 'eslint-plugin-svelte';

export default [
  js.configs.recommended,
  ...ts.configs.recommended,
  ...svelte.configs['flat/recommended'],
  {
    files: ['**/*.svelte'],
    languageOptions: { parserOptions: { parser: ts.parser } },
  },
  {
    // `no-undef` is redundant and actively wrong on TypeScript: the compiler
    // already resolves every identifier, and `no-undef` cannot see DOM lib
    // types, so `HTMLIFrameElement` in a type position reads as undefined.
    // typescript-eslint's own guidance is to turn it off for TS.
    files: ['**/*.ts', '**/*.svelte'],
    rules: { 'no-undef': 'off' },
  },
  {
    // The SPIKE-002 frame bootstrap runs inside a sandboxed iframe, plain JS
    // with no bundler in front of it, so its globals are the browser's.
    files: ['public/**/*.js'],
    languageOptions: {
      globals: {
        window: 'readonly',
        document: 'readonly',
        performance: 'readonly',
        requestAnimationFrame: 'readonly',
        NodeFilter: 'readonly',
        // In-page find (S2-8) paints ranges through the CSS Custom Highlight
        // API rather than mutating the DOM. Both globals are feature-detected
        // at run time — see `canHighlight` — so listing them here asserts
        // only that they are browser globals, not that they always exist.
        CSS: 'readonly',
        Highlight: 'readonly',
      },
    },
  },
  {
    // Build/gate scripts run under Node, not in a webview, so their globals
    // are Node's. `scripts/` is not shipped and is not bundled.
    files: ['scripts/**/*.mjs'],
    languageOptions: {
      globals: {
        console: 'readonly',
        process: 'readonly',
        // Node 18+ ships these as globals; the corpus fetcher uses them
        // rather than pulling in a dependency for one script.
        fetch: 'readonly',
        URL: 'readonly',
        setTimeout: 'readonly',
      },
    },
  },
  {
    // Test fixtures are data. `searchindex.js` is a miniature of Sphinx's own
    // output and calls a global the real page defines; linting it as project
    // source reports that global as undefined, which is true and irrelevant.
    ignores: [
      'dist/',
      'target/',
      'src-tauri/gen/',
      'node_modules/',
      'crates/*/fixtures/',
      'crates/*/corpus/',
    ],
  },
];
