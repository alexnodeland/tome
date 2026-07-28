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
