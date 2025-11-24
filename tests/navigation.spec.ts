// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from '@playwright/test';

/**
 * Navigation E2E tests for Zileo-Chat-3.
 * Tests basic page routing and navigation functionality.
 */

test.describe('Navigation', () => {
	test('should load the home page', async ({ page }) => {
		await page.goto('/');
		await expect(page).toHaveTitle(/Zileo/i);
	});

	test('should navigate to Agent page', async ({ page }) => {
		await page.goto('/');

		// Click on Agent link in navigation
		await page.click('a[href="/agent"]');

		// Should be on agent page
		await expect(page).toHaveURL(/.*\/agent/);
	});

	test('should navigate to Settings page', async ({ page }) => {
		await page.goto('/');

		// Click on Settings link in navigation
		await page.click('a[href="/settings"]');

		// Should be on settings page
		await expect(page).toHaveURL(/.*\/settings/);
	});

	test('should display navigation menu', async ({ page }) => {
		await page.goto('/');

		// Navigation should contain Agent and Settings links
		const agentLink = page.locator('a[href="/agent"]');
		const settingsLink = page.locator('a[href="/settings"]');

		await expect(agentLink).toBeVisible();
		await expect(settingsLink).toBeVisible();
	});
});
