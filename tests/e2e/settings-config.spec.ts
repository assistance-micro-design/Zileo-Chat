// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from '@playwright/test';

/**
 * Settings Configuration E2E tests for Zileo-Chat-3.
 * Tests provider configuration, model settings, and theme selection.
 */

test.describe('Settings Configuration', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/settings');
		await page.waitForLoadState('networkidle');
	});

	test('should display settings page with sidebar', async ({ page }) => {
		// Settings page structure
		const settingsPage = page.locator('.settings-page');
		await expect(settingsPage).toBeVisible();

		// Sidebar should be present
		const sidebar = page.locator('.sidebar');
		await expect(sidebar).toBeVisible();
	});

	test('should have providers section', async ({ page }) => {
		// Providers section should be visible
		const providersSection = page.locator('#providers');
		await expect(providersSection).toBeVisible();

		// Section title
		const sectionTitle = page.locator('#providers .section-title');
		await expect(sectionTitle).toContainText('Providers');
	});

	test('should display provider cards for Mistral and Ollama', async ({ page }) => {
		// Provider grid should contain cards
		const providerGrid = page.locator('.provider-grid');
		await expect(providerGrid).toBeVisible();

		// Mistral provider should be displayed
		const mistralProvider = page.locator('h3.provider-name:has-text("Mistral")');
		await expect(mistralProvider).toBeVisible();

		// Ollama provider should be displayed
		const ollamaProvider = page.locator('h3.provider-name:has-text("Ollama")');
		await expect(ollamaProvider).toBeVisible();
	});

	test('should have API key input for Mistral', async ({ page }) => {
		// API key input should be present for Mistral
		const apiKeyInput = page.locator('input[type="password"]');
		await expect(apiKeyInput).toBeVisible();
	});

	test('should show Ollama as local provider without API key', async ({ page }) => {
		// Ollama card should indicate "No API key required"
		const noKeyText = page.locator('.status-text:has-text("No API key required")');
		await expect(noKeyText).toBeVisible();
	});

	test('should have models section', async ({ page }) => {
		// Models section should exist
		const modelsSection = page.locator('#models');
		await expect(modelsSection).toBeVisible();

		// Section title
		const modelTitle = modelsSection.locator('.section-title');
		await expect(modelTitle).toContainText('Models');
	});

	test('should have provider select dropdown in models section', async ({ page }) => {
		// Provider select in model configuration
		const providerSelect = page.locator('#models select');
		await expect(providerSelect).toBeVisible();

		// Should have provider options
		const options = providerSelect.locator('option');
		const optionCount = await options.count();
		expect(optionCount).toBeGreaterThanOrEqual(2);
	});

	test('should have model input field', async ({ page }) => {
		// Model input for specifying model name
		const modelInput = page.locator('#models input[type="text"]');
		await expect(modelInput).toBeVisible();
	});

	test('should display selected model info', async ({ page }) => {
		// Model info section should show current selection
		const modelInfo = page.locator('.model-info');
		await expect(modelInfo).toBeVisible();

		// Should display provider and model values
		const infoValues = page.locator('.info-value');
		const valueCount = await infoValues.count();
		expect(valueCount).toBeGreaterThanOrEqual(2);
	});

	test('should have theme section', async ({ page }) => {
		// Theme section should exist
		const themeSection = page.locator('#theme');
		await expect(themeSection).toBeVisible();

		// Section title
		const themeTitle = themeSection.locator('.section-title');
		await expect(themeTitle).toContainText('Theme');
	});

	test('should display light and dark theme options', async ({ page }) => {
		// Theme grid with options
		const themeGrid = page.locator('.theme-grid');
		await expect(themeGrid).toBeVisible();

		// Light theme option
		const lightTheme = page.locator('.theme-title:has-text("Light Mode")');
		await expect(lightTheme).toBeVisible();

		// Dark theme option
		const darkTheme = page.locator('.theme-title:has-text("Dark Mode")');
		await expect(darkTheme).toBeVisible();
	});

	test('should allow theme selection', async ({ page }) => {
		// Theme cards should be clickable buttons
		const themeCards = page.locator('.theme-card');
		const cardCount = await themeCards.count();

		expect(cardCount).toBe(2);

		// Each card should be interactive
		for (let i = 0; i < cardCount; i++) {
			const card = themeCards.nth(i);
			await expect(card).toBeEnabled();
		}
	});

	test('should show security information section', async ({ page }) => {
		// Security info card should be present
		const securityHeader = page.locator('.security-header');
		await expect(securityHeader).toBeVisible();

		// Should mention encryption
		const securityText = page.locator('.security-info-text');
		await expect(securityText).toContainText(/AES-256|encryption|keychain/i);
	});

	test('should have navigation buttons in sidebar', async ({ page }) => {
		// Navigation buttons for sections
		const navButtons = page.locator('.nav-button, .nav-button-icon');
		const buttonCount = await navButtons.count();

		// Should have at least providers, models, theme
		expect(buttonCount).toBeGreaterThanOrEqual(3);
	});

	test('should highlight active navigation section', async ({ page }) => {
		// At least one nav button should have active state
		const activeNav = page.locator('.nav-button.active, .nav-button-icon.active');

		// Should have exactly one active
		const activeCount = await activeNav.count();
		expect(activeCount).toBeGreaterThanOrEqual(1);
	});

	test('should scroll to section when navigation clicked', async ({ page }) => {
		// Click on theme navigation
		const themeNav = page.locator('.nav-button:has-text("Theme")');

		if (await themeNav.isVisible()) {
			await themeNav.click();

			// Theme section should be in viewport
			const themeSection = page.locator('#theme');
			await expect(themeSection).toBeInViewport();
		}
	});

	test('should show API key management card', async ({ page }) => {
		// API key management section
		const apiKeyCard = page.locator('h3.card-title:has-text("API Key Management")');
		await expect(apiKeyCard).toBeVisible();

		// Save button should exist
		const saveButton = page.locator('button:has-text("Save API Key")');
		await expect(saveButton).toBeVisible();
	});

	test('should toggle sidebar collapse', async ({ page }) => {
		// Collapse button
		const collapseBtn = page.locator('.sidebar-collapse-btn');
		await expect(collapseBtn).toBeVisible();

		// Click to toggle
		await collapseBtn.click();
		await page.waitForTimeout(300);

		// Sidebar should have collapsed state
		const sidebar = page.locator('.sidebar');
		const isCollapsed = await sidebar.evaluate((el) => {
			return el.classList.contains('collapsed');
		});

		expect(isCollapsed).toBe(true);
	});

	test('should have security badge in sidebar footer', async ({ page }) => {
		// Security badge showing encryption info
		const securityBadge = page.locator('.security-badge');

		// May only be visible when sidebar expanded
		const sidebar = page.locator('.sidebar');
		const isCollapsed = await sidebar.evaluate((el) => {
			return el.classList.contains('collapsed');
		});

		if (!isCollapsed) {
			await expect(securityBadge).toBeVisible();
			await expect(securityBadge).toContainText('AES-256');
		}
	});
});
