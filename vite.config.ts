// Vite configuration
// https://vitejs.dev/config/

import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [sveltekit()],

  // Development server configuration
  server: {
    port: 5173,
    strictPort: true,
    // Allow Tauri to connect
    fs: {
      allow: ['..'],
    },
  },

  // Build configuration
  build: {
    // Target modern browsers only (macOS 12+)
    target: 'safari15',
    // Generate source maps for debugging
    sourcemap: true,
    // Minify for production
    minify: 'esbuild',
    // Chunk size warnings
    chunkSizeWarningLimit: 500,
  },

  // Dependency optimization
  optimizeDeps: {
    // Pre-bundle Tauri API
    include: ['@tauri-apps/api'],
  },

  // Resolve configuration
  resolve: {
    alias: {
      $lib: '/src/lib',
    },
  },

  // Environment variables
  // VITE_* prefixed vars are exposed to client
  envPrefix: 'VITE_',

  // Test configuration (Vitest)
  test: {
    // Use jsdom for DOM testing
    environment: 'jsdom',
    // Global test APIs (describe, it, expect)
    globals: true,
    // Setup files
    setupFiles: ['./src/lib/test-setup.ts'],
    // Include patterns
    include: ['src/**/*.{test,spec}.ts'],
    // Coverage configuration
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov', 'html'],
      reportsDirectory: './coverage',
      exclude: [
        'node_modules/',
        'dist/',
        '.svelte-kit/',
        '**/*.d.ts',
        '**/*.test.ts',
        '**/*.spec.ts',
        'src/lib/test-setup.ts',
      ],
      // Enforce coverage thresholds
      thresholds: {
        global: {
          branches: 90,
          functions: 90,
          lines: 90,
          statements: 90,
        },
      },
    },
    // Mock Tauri APIs in tests
    deps: {
      inline: ['@tauri-apps/api'],
    },
  },
});
