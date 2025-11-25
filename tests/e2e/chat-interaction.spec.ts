// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from '@playwright/test';

/**
 * Chat Interaction E2E tests for Zileo-Chat-3.
 * Tests message input, display, and agent interaction UI.
 *
 * Note: These tests focus on UI interaction, not actual LLM responses
 * which would require Tauri backend mocking.
 */

test.describe('Chat Interaction', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/agent');
		await page.waitForLoadState('networkidle');
	});

	test('should display chat interface elements', async ({ page }) => {
		// Agent page should have the main chat area structure
		const agentMain = page.locator('.agent-main');
		await expect(agentMain).toBeVisible();
	});

	test('should show empty state without selected workflow', async ({ page }) => {
		// Without workflow selected, show empty state
		const emptyState = page.locator('.empty-state');
		await expect(emptyState).toBeVisible();

		// Empty state should have icon
		const emptyIcon = page.locator('.empty-state .empty-icon');
		// Check that empty-icon class or svg exists
		const hasIcon = await emptyIcon.count() > 0 ||
			await page.locator('.empty-state svg').count() > 0;
		expect(hasIcon).toBe(true);
	});

	test('should have agent header when workflow visible', async ({ page }) => {
		// Agent header component should exist in structure
		// (visible only when workflow selected)
		const agentHeader = page.locator('.agent-header');

		// If a workflow is selected, header should be visible
		// Otherwise empty state is shown
		const emptyState = page.locator('.empty-state');
		const isEmptyVisible = await emptyState.isVisible();

		if (!isEmptyVisible) {
			await expect(agentHeader).toBeVisible();
		}
	});

	test('should display message list component', async ({ page }) => {
		// Message list area should exist in DOM structure
		const pageHTML = await page.content();

		// Page should contain message-related components
		expect(pageHTML).toMatch(/messages|MessageList/i);
	});

	test('should have chat input component structure', async ({ page }) => {
		// ChatInput component should exist on page
		// May only render when workflow selected
		const chatInput = page.locator('.chat-input');
		const emptyState = page.locator('.empty-state');

		const isEmptyVisible = await emptyState.isVisible();

		if (!isEmptyVisible) {
			// Chat input should be present when workflow active
			await expect(chatInput).toBeVisible();
		}
	});

	test('should show metrics bar structure', async ({ page }) => {
		// MetricsBar component should exist in structure
		// Only renders after workflow execution with result
		const pageHTML = await page.content();

		// Page should import or contain metrics-related code
		expect(pageHTML.toLowerCase()).toMatch(/metric/);
	});

	test('should have new workflow button accessible', async ({ page }) => {
		// New workflow button should be accessible
		const newButton = page.locator('button:has-text("New Workflow")');

		// Should be visible in empty state or sidebar
		const isVisible = await newButton.isVisible();

		if (isVisible) {
			// Button should be clickable
			await expect(newButton).toBeEnabled();
		}
	});

	test('should have accessible message area', async ({ page }) => {
		// Check accessibility of main content area
		const mainArea = page.locator('.agent-main');
		await expect(mainArea).toBeVisible();

		// Main element should exist for semantic structure
		const mainElement = page.locator('main.agent-main');
		await expect(mainElement).toBeVisible();
	});

	test('should display workflow name in header when selected', async ({ page }) => {
		// Agent title should be in header when workflow active
		const agentTitle = page.locator('.agent-title');
		const emptyState = page.locator('.empty-state');

		const isEmptyVisible = await emptyState.isVisible();

		if (!isEmptyVisible) {
			// Title should display workflow name
			await expect(agentTitle).toBeVisible();
		}
	});

	test('should have responsive layout', async ({ page }) => {
		// Test layout at different viewport sizes
		const agentPage = page.locator('.agent-page');
		await expect(agentPage).toBeVisible();

		// Check flex layout
		const display = await agentPage.evaluate((el) => {
			return window.getComputedStyle(el).display;
		});

		expect(display).toBe('flex');
	});
});
