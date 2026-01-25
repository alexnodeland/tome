/// <reference types="vitest" />
import path from 'path';

import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [svelte({ hot: !process.env.VITEST })],
  test: {
    // Test environment
    environment: 'jsdom',

    // Include patterns
    include: ['src/**/*.{test,spec}.{js,ts}'],

    // Exclude patterns
    exclude: ['node_modules', 'e2e/**/*'],

    // Global test setup
    setupFiles: ['./src/lib/test-setup.ts'],

    // Coverage configuration
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov', 'html'],
      reportsDirectory: './coverage',

      // Coverage thresholds - 90% minimum as per planning docs
      thresholds: {
        lines: 90,
        functions: 90,
        branches: 85,
        statements: 90,
      },

      // Files to include in coverage
      include: ['src/lib/**/*.{ts,svelte}'],

      // Files to exclude from coverage
      exclude: [
        'src/lib/**/*.test.ts',
        'src/lib/**/*.spec.ts',
        'src/lib/test-setup.ts',
        'src/lib/**/index.ts', // barrel exports
        'src/lib/types/**/*', // type definitions only
      ],
    },

    // Globals
    globals: true,

    // Reporter
    reporters: ['default'],

    // Timeouts
    testTimeout: 10000,
    hookTimeout: 10000,

    // Watch mode options
    watch: true,
    watchExclude: ['node_modules', 'dist', '.svelte-kit'],
  },
  resolve: {
    alias: {
      $lib: path.resolve(__dirname, './src/lib'),
    },
  },
});
