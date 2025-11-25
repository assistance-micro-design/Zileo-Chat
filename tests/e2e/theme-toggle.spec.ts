// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from '@playwright/test';

/**
 * Theme Toggle E2E tests for Zileo-Chat-3.
 * Tests light/dark mode switching and persistence.
 */

test.describe('Theme Toggle', () => {
	test.beforeEach(async ({ page }) => {
		// Clear localStorage before each test for clean state
		await page.goto('/');
		await page.evaluate(() => {
			localStorage.removeItem('theme');
		});
		await page.reload();
		await page.waitForLoadState('networkidle');
	});

	test('should display theme toggle button in navigation', async ({ page }) => {
		// Theme toggle button should be in floating menu
		const themeButton = page.locator('button[aria-label*="mode"]');
		await expect(themeButton).toBeVisible();
	});

	test('should have correct initial theme based on system preference', async ({ page }) => {
		// Get current theme attribute on document
		const theme = await page.evaluate(() => {
			return document.documentElement.getAttribute('data-theme');
		});

		// Should be either light or dark
		expect(['light', 'dark', null]).toContain(theme);
	});

	test('should toggle theme when button clicked', async ({ page }) => {
		// Get initial theme
		const initialTheme = await page.evaluate(() => {
			return document.documentElement.getAttribute('data-theme') || 'light';
		});

		// Click theme toggle
		const themeButton = page.locator('button[aria-label*="mode"]');
		await themeButton.click();

		// Wait for transition
		await page.waitForTimeout(200);

		// Theme should have changed
		const newTheme = await page.evaluate(() => {
			return document.documentElement.getAttribute('data-theme');
		});

		expect(newTheme).not.toBe(initialTheme);
	});

	test('should persist theme preference in localStorage', async ({ page }) => {
		// Toggle theme
		const themeButton = page.locator('button[aria-label*="mode"]');
		await themeButton.click();

		await page.waitForTimeout(200);

		// Check localStorage
		const storedTheme = await page.evaluate(() => {
			return localStorage.getItem('theme');
		});

		expect(storedTheme).toBeTruthy();
		expect(['light', 'dark']).toContain(storedTheme);
	});

	test('should restore theme from localStorage on page load', async ({ page }) => {
		// Set theme to dark in localStorage
		await page.evaluate(() => {
			localStorage.setItem('theme', 'dark');
		});

		// Reload page
		await page.reload();
		await page.waitForLoadState('networkidle');

		// Theme should be dark
		const theme = await page.evaluate(() => {
			return document.documentElement.getAttribute('data-theme');
		});

		expect(theme).toBe('dark');
	});

	test('should update toggle button icon based on theme', async ({ page }) => {
		// Get initial button content/aria-label
		const themeButton = page.locator('button[aria-label*="mode"]');
		const initialLabel = await themeButton.getAttribute('aria-label');

		// Toggle theme
		await themeButton.click();
		await page.waitForTimeout(200);

		// Label should change
		const newLabel = await themeButton.getAttribute('aria-label');
		expect(newLabel).not.toBe(initialLabel);
	});

	test('should apply theme on settings page', async ({ page }) => {
		// Navigate to settings
		await page.goto('/settings');
		await page.waitForLoadState('networkidle');

		// Settings page should respect theme
		const theme = await page.evaluate(() => {
			return document.documentElement.getAttribute('data-theme');
		});

		expect(['light', 'dark', null]).toContain(theme);
	});

	test('should allow theme selection from settings page', async ({ page }) => {
		await page.goto('/settings');
		await page.waitForLoadState('networkidle');

		// Find theme cards
		const darkThemeCard = page.locator('.theme-card').filter({
			has: page.locator('.theme-title:has-text("Dark Mode")')
		});

		if (await darkThemeCard.isVisible()) {
			// Click dark theme
			await darkThemeCard.click();
			await page.waitForTimeout(200);

			// Theme should be dark
			const theme = await page.evaluate(() => {
				return document.documentElement.getAttribute('data-theme');
			});

			expect(theme).toBe('dark');
		}
	});

	test('should highlight selected theme card in settings', async ({ page }) => {
		await page.goto('/settings');
		await page.waitForLoadState('networkidle');

		// One theme card should have selected class
		const selectedCard = page.locator('.theme-card.selected');
		await expect(selectedCard).toBeVisible();
	});

	test('should apply correct CSS variables for light theme', async ({ page }) => {
		// Set light theme
		await page.evaluate(() => {
			document.documentElement.setAttribute('data-theme', 'light');
		});

		// Check background color variable
		const bgColor = await page.evaluate(() => {
			return getComputedStyle(document.documentElement).getPropertyValue('--color-bg-primary').trim();
		});

		// Light theme should have white-ish background
		expect(bgColor).toMatch(/#fff|#ffffff|white|rgb\(255/i);
	});

	test('should apply correct CSS variables for dark theme', async ({ page }) => {
		// Set dark theme
		await page.evaluate(() => {
			document.documentElement.setAttribute('data-theme', 'dark');
		});

		// Check background color variable
		const bgColor = await page.evaluate(() => {
			return getComputedStyle(document.documentElement).getPropertyValue('--color-bg-primary').trim();
		});

		// Dark theme should have darker background
		expect(bgColor).toMatch(/#2b2d31|#1e1f22|rgb\(43/i);
	});

	test('should maintain theme across navigation', async ({ page }) => {
		// Set dark theme
		const themeButton = page.locator('button[aria-label*="mode"]');

		// Check initial and toggle if needed
		const initialTheme = await page.evaluate(() => {
			return document.documentElement.getAttribute('data-theme');
		});

		if (initialTheme !== 'dark') {
			await themeButton.click();
			await page.waitForTimeout(200);
		}

		// Navigate to agent page
		const agentLink = page.locator('a[href="/agent"]');
		await agentLink.click();
		await page.waitForLoadState('networkidle');

		// Theme should still be dark
		const currentTheme = await page.evaluate(() => {
			return document.documentElement.getAttribute('data-theme');
		});

		expect(currentTheme).toBe('dark');

		// Navigate to settings
		const settingsLink = page.locator('a[href="/settings"]');
		await settingsLink.click();
		await page.waitForLoadState('networkidle');

		// Theme should still be dark
		const settingsTheme = await page.evaluate(() => {
			return document.documentElement.getAttribute('data-theme');
		});

		expect(settingsTheme).toBe('dark');
	});

	test('should have accessible theme toggle', async ({ page }) => {
		// Button should have aria-label
		const themeButton = page.locator('button[aria-label*="mode"]');
		const ariaLabel = await themeButton.getAttribute('aria-label');

		expect(ariaLabel).toBeTruthy();
		expect(ariaLabel).toMatch(/switch|toggle|mode/i);
	});
});
