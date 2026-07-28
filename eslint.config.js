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
    // The SPIKE-002 frame bootstrap runs inside a sandboxed iframe, plain JS
    // with no bundler in front of it, so its globals are the browser's.
    files: ['public/**/*.js'],
    languageOptions: {
      globals: {
        window: 'readonly',
        document: 'readonly',
        performance: 'readonly',
        requestAnimationFrame: 'readonly',
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
