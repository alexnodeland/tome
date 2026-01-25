// ESLint configuration
// https://eslint.org/docs/user-guide/configuring

/** @type {import('eslint').Linter.Config} */
module.exports = {
  root: true,

  // Parser configuration
  parser: '@typescript-eslint/parser',
  parserOptions: {
    ecmaVersion: 'latest',
    sourceType: 'module',
    project: './tsconfig.json',
    extraFileExtensions: ['.svelte'],
  },

  // Plugins
  plugins: ['@typescript-eslint', 'import'],

  // Extends - order matters (later overrides earlier)
  extends: [
    'eslint:recommended',
    'plugin:@typescript-eslint/recommended',
    'plugin:@typescript-eslint/recommended-requiring-type-checking',
    'plugin:@typescript-eslint/strict',
    'plugin:svelte/recommended',
    'plugin:import/recommended',
    'plugin:import/typescript',
    'prettier', // Must be last - disables rules that conflict with Prettier
  ],

  // Environment
  env: {
    browser: true,
    es2022: true,
    node: true,
  },

  // Global settings
  settings: {
    'import/resolver': {
      typescript: {
        project: './tsconfig.json',
      },
    },
  },

  // Rules
  rules: {
    // === TypeScript Rules ===

    // Enforce explicit return types on functions
    // Why: Improves readability and catches errors
    '@typescript-eslint/explicit-function-return-type': [
      'warn',
      {
        allowExpressions: true,
        allowTypedFunctionExpressions: true,
        allowHigherOrderFunctions: true,
      },
    ],

    // Enforce explicit accessibility modifiers
    // Why: Makes class interfaces clear
    '@typescript-eslint/explicit-member-accessibility': [
      'warn',
      { accessibility: 'no-public' },
    ],

    // No any type
    // Why: Defeats the purpose of TypeScript
    '@typescript-eslint/no-explicit-any': 'error',

    // No non-null assertions
    // Why: Can cause runtime errors
    '@typescript-eslint/no-non-null-assertion': 'warn',

    // Prefer nullish coalescing
    // Why: Safer than || for falsy values
    '@typescript-eslint/prefer-nullish-coalescing': 'warn',

    // Prefer optional chaining
    // Why: Cleaner and safer
    '@typescript-eslint/prefer-optional-chain': 'warn',

    // No unused vars
    // Why: Clean code, but allow _ prefix for intentionally unused
    '@typescript-eslint/no-unused-vars': [
      'error',
      {
        argsIgnorePattern: '^_',
        varsIgnorePattern: '^_',
      },
    ],

    // Consistent type imports
    // Why: Better tree-shaking and clearer intent
    '@typescript-eslint/consistent-type-imports': [
      'warn',
      { prefer: 'type-imports' },
    ],

    // === Import Rules ===

    // Enforce import order
    // Why: Consistent, readable imports
    'import/order': [
      'warn',
      {
        groups: [
          'builtin',
          'external',
          'internal',
          ['parent', 'sibling'],
          'index',
          'type',
        ],
        'newlines-between': 'always',
        alphabetize: { order: 'asc' },
      },
    ],

    // No default exports
    // Why: Named exports are more refactor-friendly
    // Disabled for Svelte components which use default exports
    'import/no-default-export': 'off',

    // No circular dependencies
    // Why: Can cause subtle bugs
    'import/no-cycle': 'error',

    // === Architecture Boundaries ===
    // These rules enforce the module boundaries defined in CLAUDE.md

    'import/no-restricted-paths': [
      'error',
      {
        zones: [
          // utils cannot import from other lib directories
          {
            target: './src/lib/utils',
            from: './src/lib/components',
            message: 'utils cannot import from components',
          },
          {
            target: './src/lib/utils',
            from: './src/lib/stores',
            message: 'utils cannot import from stores',
          },
          {
            target: './src/lib/utils',
            from: './src/lib/services',
            message: 'utils cannot import from services',
          },
          // types cannot import from other lib directories
          {
            target: './src/lib/types',
            from: './src/lib/components',
            message: 'types cannot import from components',
          },
          {
            target: './src/lib/types',
            from: './src/lib/stores',
            message: 'types cannot import from stores',
          },
          {
            target: './src/lib/types',
            from: './src/lib/services',
            message: 'types cannot import from services',
          },
          {
            target: './src/lib/types',
            from: './src/lib/utils',
            message: 'types cannot import from utils',
          },
          // services cannot import from components or stores
          {
            target: './src/lib/services',
            from: './src/lib/components',
            message: 'services cannot import from components',
          },
          {
            target: './src/lib/services',
            from: './src/lib/stores',
            message: 'services cannot import from stores',
          },
          // stores cannot import from components
          {
            target: './src/lib/stores',
            from: './src/lib/components',
            message: 'stores cannot import from components',
          },
          // No component can import from routes
          {
            target: './src/lib',
            from: './src/routes',
            message: 'lib cannot import from routes',
          },
        ],
      },
    ],

    // === General Rules ===

    // Prefer const
    'prefer-const': 'error',

    // No console in production code
    // Why: Use proper logging
    'no-console': ['warn', { allow: ['warn', 'error'] }],

    // === Disabled Rules ===
    // Rules that conflict with our patterns or are too strict

    // Allow empty functions for placeholders
    '@typescript-eslint/no-empty-function': 'off',
  },

  // Svelte-specific overrides
  overrides: [
    {
      files: ['*.svelte'],
      parser: 'svelte-eslint-parser',
      parserOptions: {
        parser: '@typescript-eslint/parser',
      },
      rules: {
        // Svelte-specific adjustments
        '@typescript-eslint/no-unsafe-assignment': 'off',
        '@typescript-eslint/no-unsafe-member-access': 'off',
        '@typescript-eslint/no-unsafe-call': 'off',
      },
    },
    {
      // Test files
      files: ['**/*.test.ts', '**/*.spec.ts'],
      rules: {
        // Allow any in tests for mocking
        '@typescript-eslint/no-explicit-any': 'off',
        // Allow non-null assertions in tests
        '@typescript-eslint/no-non-null-assertion': 'off',
      },
    },
    {
      // Config files (CommonJS)
      files: ['*.cjs'],
      env: {
        node: true,
      },
      rules: {
        '@typescript-eslint/no-var-requires': 'off',
      },
    },
    {
      // Scripts and config files - allow console and relax type checking
      files: ['scripts/**/*.ts', '*.config.ts', '*.config.js', 'e2e/**/*.ts'],
      rules: {
        'no-console': 'off',
        '@typescript-eslint/no-require-imports': 'off',
      },
    },
  ],

  // Ignored files
  ignorePatterns: [
    'dist/',
    '.svelte-kit/',
    'node_modules/',
    'coverage/',
    'src-tauri/',
    '*.cjs', // Will be linted with override
  ],
};
