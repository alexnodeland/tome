import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath, URL } from 'node:url';

// Vitest reuses this config, which is why Svelte components compile in tests
// with no extra setup. (Jest could not: `ts-jest` has no Svelte transform.)
export default defineConfig({
  plugins: [svelte()],

  // Must mirror `paths` in tsconfig.json. TypeScript's `paths` only informs the
  // type checker; the bundler needs its own alias or imports resolve at type
  // time and fail at run time.
  resolve: {
    alias: { $lib: fileURLToPath(new URL('./src/lib', import.meta.url)) },

    // Svelte 5 ships separate browser and server builds. Under Vitest the
    // server build resolves by default, and `mount()` throws
    // `lifecycle_function_unavailable`. Scoped to tests so the app build is
    // untouched.
    ...(process.env.VITEST ? { conditions: ['browser'] } : {}),
  },

  // Tauri expects a fixed port and should fail rather than silently pick another.
  clearScreen: false,
  server: { port: 1420, strictPort: true },

  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    coverage: {
      provider: 'v8',
      include: ['src/**/*.{ts,svelte}'],
      exclude: ['src/**/*.d.ts', 'src/test/**', 'src/main.ts'],
      // Per-module thresholds. A single global number is the wrong shape:
      // state management and UI components do not deserve the same bar.
      // See docs/plans/08-testing-strategy.md.
      thresholds: {
        'src/lib/stores/**': { lines: 90, branches: 85 },
        'src/lib/utils/**': { lines: 90, branches: 85 },
        'src/lib/components/**': { lines: 80, branches: 70 },
      },
    },
  },
});
