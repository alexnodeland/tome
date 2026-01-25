// Playwright E2E test configuration
// https://playwright.dev/docs/test-configuration

import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  // Test directory
  testDir: './e2e',

  // Test file pattern
  testMatch: '**/*.spec.ts',

  // Timeout for each test
  timeout: 30_000,

  // Timeout for expect assertions
  expect: {
    timeout: 5_000,
  },

  // Run tests in parallel
  fullyParallel: true,

  // Fail fast - stop on first failure in CI
  forbidOnly: !!process.env.CI,

  // Retry failed tests
  retries: process.env.CI ? 2 : 0,

  // Limit workers in CI
  workers: process.env.CI ? 1 : undefined,

  // Reporter configuration
  reporter: [
    ['list'],
    ['html', { open: 'never' }],
    ...(process.env.CI ? [['github' as const]] : []),
  ],

  // Shared settings for all projects
  use: {
    // Base URL for navigation
    // For Tauri apps, this might be different
    baseURL: 'tauri://localhost',

    // Collect trace on failure
    trace: 'on-first-retry',

    // Collect video on failure
    video: 'on-first-retry',

    // Screenshot on failure
    screenshot: 'only-on-failure',

    // Viewport size
    viewport: { width: 1280, height: 720 },

    // Ignore HTTPS errors (for local dev)
    ignoreHTTPSErrors: true,
  },

  // Test projects - WebKit only since we target macOS
  projects: [
    {
      name: 'webkit',
      use: {
        ...devices['Desktop Safari'],
      },
    },
  ],

  // Global setup/teardown
  // globalSetup: './e2e/global-setup.ts',
  // globalTeardown: './e2e/global-teardown.ts',

  // Output directory for test artifacts
  outputDir: './e2e/test-results',

  // Web server for development
  // For Tauri apps, you typically run the app separately
  webServer: process.env.CI
    ? undefined
    : {
        command: 'npm run dev:vite',
        port: 5173,
        reuseExistingServer: true,
      },
});
