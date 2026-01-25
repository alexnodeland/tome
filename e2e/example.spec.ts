/**
 * Example E2E Test
 *
 * This demonstrates the basic structure of an E2E test.
 * Real tests should be in separate spec files by feature.
 */

import { test, expect } from '@playwright/test';

import { launchApp } from './fixtures/app';

test.describe('Application Launch', () => {
  test('should display welcome screen', async ({ page }) => {
    await launchApp(page);

    // Check that the app title is visible
    await expect(page.getByRole('heading', { name: /Welcome to Tome/i })).toBeVisible();
  });

  test('should show keyboard shortcuts', async ({ page }) => {
    await launchApp(page);

    // Check that keyboard shortcuts section is visible
    await expect(page.getByText('Keyboard Shortcuts')).toBeVisible();
    await expect(page.getByText('Global search')).toBeVisible();
  });
});
