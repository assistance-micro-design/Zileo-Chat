// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from '@playwright/test';

/**
 * Settings page E2E tests for Zileo-Chat-3.
 * Tests LLM provider configuration UI.
 */

test.describe('Settings Page', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/settings');
	});

	test('should display settings page title', async ({ page }) => {
		// Settings heading should be visible
		const heading = page.locator('h1:has-text("Settings")');
		await expect(heading).toBeVisible();
	});

	test('should display LLM provider section', async ({ page }) => {
		// LLM Provider section should be visible
		const section = page.locator('h2:has-text("LLM Provider")');
		await expect(section).toBeVisible();
	});

	test('should have provider select dropdown', async ({ page }) => {
		// Provider select should be visible
		const providerSelect = page.locator('select');
		await expect(providerSelect).toBeVisible();

		// Should have Mistral and Ollama options
		const options = providerSelect.locator('option');
		await expect(options).toHaveCount(2);
	});

	test('should have model input field', async ({ page }) => {
		// Model input should be visible
		const modelInput = page.locator('input[type="text"]').first();
		await expect(modelInput).toBeVisible();
	});

	test('should show API key input when Mistral selected', async ({ page }) => {
		// Select Mistral provider
		const providerSelect = page.locator('select');
		await providerSelect.selectOption('Mistral');

		// API key input should be visible
		const apiKeyInput = page.locator('input[type="password"]');
		await expect(apiKeyInput).toBeVisible();
	});

	test('should hide API key input when Ollama selected', async ({ page }) => {
		// Select Ollama provider
		const providerSelect = page.locator('select');
		await providerSelect.selectOption('Ollama');

		// API key input should not be visible
		const apiKeyInput = page.locator('input[type="password"]');
		await expect(apiKeyInput).not.toBeVisible();
	});

	test('should have save button', async ({ page }) => {
		// Save button should be visible
		const saveButton = page.locator('button:has-text("Save")');
		await expect(saveButton).toBeVisible();
	});
});
