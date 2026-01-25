// Svelte configuration
// https://kit.svelte.dev/docs/configuration

import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  // Preprocess TypeScript and CSS
  preprocess: vitePreprocess(),

  kit: {
    // Static adapter for Tauri
    // Outputs to 'dist' directory which Tauri serves
    adapter: adapter({
      pages: 'dist',
      assets: 'dist',
      fallback: 'index.html',
      precompress: false,
      strict: true,
    }),

    // Alias configuration (also in tsconfig.json for TypeScript)
    alias: {
      $lib: './src/lib',
    },

    // Disable SSR - Tauri apps are client-side only
    prerender: {
      entries: [],
    },

    // Disable CSRF protection for Tauri
    // Tauri handles this at the IPC level
    csrf: {
      checkOrigin: false,
    },
  },

  // Compile-time warnings
  compilerOptions: {
    // Enable runtime checks in dev
    dev: process.env.NODE_ENV !== 'production',
  },

  // Extension handling
  extensions: ['.svelte'],

  // Svelte-specific options
  onwarn: (warning, handler) => {
    // Ignore certain warnings
    // A11y warnings are important but some are too strict
    if (warning.code === 'a11y-click-events-have-key-events') return;
    if (warning.code === 'a11y-no-static-element-interactions') return;

    // Handle other warnings normally
    handler(warning);
  },
};

export default config;
