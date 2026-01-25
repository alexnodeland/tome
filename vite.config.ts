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
      reporter: ['text', 'lcov'],
      reportsDirectory: './coverage',
      include: ['src/lib/**/*.ts'],
      exclude: [
        'src/lib/**/*.test.ts',
        'src/lib/**/*.spec.ts',
        'src/lib/test-setup.ts',
        'src/lib/**/index.ts',
        'src/lib/types/**/*',
      ],
      // Coverage thresholds - temporarily lower for scaffold
      // TODO: Increase to 90% when more code is implemented
      thresholds: {
        global: {
          branches: 50,
          functions: 50,
          lines: 50,
          statements: 50,
        },
      },
    },
    // Mock Tauri APIs in tests
    server: {
      deps: {
        inline: ['@tauri-apps/api'],
      },
    },
  },
});
