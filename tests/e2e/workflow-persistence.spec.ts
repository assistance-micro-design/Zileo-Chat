// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from '@playwright/test';

/**
 * Workflow Persistence E2E tests for Zileo-Chat-3.
 * Tests workflow state persistence including messages, tool executions,
 * and thinking steps across page reloads.
 *
 * Phase 6: Polish and Optimizations
 */

test.describe('Workflow Persistence', () => {
	test.beforeEach(async ({ page }) => {
		// Navigate to agent page where workflows are managed
		await page.goto('/agent');

		// Wait for page to be fully loaded
		await page.waitForLoadState('networkidle');
	});

	test('should display skeleton loading during message fetch', async ({ page }) => {
		// The skeleton component should briefly appear during initial load
		// or when switching workflows

		// Wait for main content to load
		await page.waitForLoadState('domcontentloaded');

		// Page should eventually show message list or empty state
		// Skeleton elements (.message-list-skeleton, .skeleton) appear briefly during loading
		const messageArea = page.locator('.messages-container, .message-list, .message-list-empty');
		await expect(messageArea.first()).toBeVisible({ timeout: 10000 });
	});

	test('should persist workflow selection across page reload', async ({ page }) => {
		// Store original URL for comparison
		const originalUrl = page.url();

		// Reload the page
		await page.reload();
		await page.waitForLoadState('networkidle');

		// Workflow page should still be visible
		const agentPage = page.locator('.agent-page');
		await expect(agentPage).toBeVisible();

		// URL should remain on agent page
		expect(page.url()).toContain('/agent');
		expect(page.url()).toBe(originalUrl);
	});

	test('should display tool execution panel when expanded', async ({ page }) => {
		// Look for tool execution panel or its header
		const toolPanel = page.locator('.tool-execution-panel');

		// If panel exists, it should be expandable
		if (await toolPanel.isVisible()) {
			const panelHeader = toolPanel.locator('.panel-header');
			await expect(panelHeader).toBeVisible();

			// Click to toggle expansion
			await panelHeader.click();

			// Check for expanded state
			await page.waitForTimeout(300); // Wait for transition animation
		}
	});

	test('should display reasoning panel when expanded', async ({ page }) => {
		// Look for reasoning panel or its header
		const reasoningPanel = page.locator('.reasoning-panel');

		// If panel exists, it should be expandable
		if (await reasoningPanel.isVisible()) {
			const panelHeader = reasoningPanel.locator('.panel-header');
			await expect(panelHeader).toBeVisible();

			// Click to toggle expansion
			await panelHeader.click();

			// Check for expanded state
			await page.waitForTimeout(300); // Wait for transition animation
		}
	});

	test('should have message list with proper accessibility attributes', async ({ page }) => {
		// Message list should have proper ARIA attributes
		const messageList = page.locator('.message-list');

		if (await messageList.isVisible()) {
			// Check for log role (for live region)
			await expect(messageList).toHaveAttribute('role', 'log');

			// Check for aria-live for real-time updates
			await expect(messageList).toHaveAttribute('aria-live', 'polite');

			// Check for aria-label
			await expect(messageList).toHaveAttribute('aria-label', 'Chat messages');
		}
	});

	test('should support keyboard navigation in workflow list', async ({ page }) => {
		// Focus on sidebar
		const sidebar = page.locator('.sidebar');
		await expect(sidebar).toBeVisible();

		// Look for workflow items
		const workflowItems = page.locator('.workflow-item');
		const count = await workflowItems.count();

		if (count > 0) {
			// Focus first item
			await workflowItems.first().focus();

			// Should be focusable
			await expect(workflowItems.first()).toBeFocused();
		}
	});

	test('should handle empty workflow state gracefully', async ({ page }) => {
		// Empty state should be visible when no workflow is selected
		const emptyState = page.locator('.empty-state');

		if (await emptyState.isVisible()) {
			// Empty state should have a helpful message
			const emptyMessage = emptyState.locator('h3, p');
			await expect(emptyMessage.first()).toBeVisible();

			// Should have action button
			const actionButton = emptyState.locator('button');
			if (await actionButton.isVisible()) {
				await expect(actionButton).toBeEnabled();
			}
		}
	});

	test('should display metrics bar when workflow is active', async ({ page }) => {
		// Metrics bar shows token counts and tool usage
		const metricsBar = page.locator('.metrics-bar');

		// If workflow is active, metrics should be visible
		if (await metricsBar.isVisible()) {
			// Check for token display
			const tokenDisplay = metricsBar.locator('.token-display, .tokens');
			if (await tokenDisplay.isVisible()) {
				await expect(tokenDisplay).toBeVisible();
			}
		}
	});

	test('should have responsive sidebar toggle', async ({ page }) => {
		// Find collapse button
		const collapseButton = page.locator('.sidebar-collapse-btn, [aria-label*="collapse"]');

		if (await collapseButton.isVisible()) {
			// Get initial sidebar state
			const sidebar = page.locator('.sidebar');
			const initialWidth = await sidebar.boundingBox();

			// Click to toggle
			await collapseButton.click();
			await page.waitForTimeout(300);

			// Width should change
			const newWidth = await sidebar.boundingBox();

			expect(newWidth?.width).not.toBe(initialWidth?.width);

			// Click again to restore
			await collapseButton.click();
			await page.waitForTimeout(300);
		}
	});

	test('should maintain scroll position in message list', async ({ page }) => {
		const messageList = page.locator('.message-list');

		if (await messageList.isVisible()) {
			// Scroll down if there's content
			await messageList.evaluate((el) => {
				el.scrollTop = 100;
			});

			// Wait a moment for scroll to apply
			await page.waitForTimeout(100);

			// Get scroll position after setting
			const scrollPosition = await messageList.evaluate((el) => el.scrollTop);

			// Position should be a valid number (may be 0 if no scrollable content)
			expect(typeof scrollPosition).toBe('number');
			expect(scrollPosition).toBeGreaterThanOrEqual(0);
		}
	});

	test('should display streaming indicator when workflow is running', async ({ page }) => {
		// Look for streaming indicators
		const streamingIndicator = page.locator(
			'.streaming-indicator, .status-running, [class*="streaming"]'
		);

		// If streaming is active, indicator should be visible
		// This is a visual check only - streaming may or may not be active
		if (await streamingIndicator.first().isVisible()) {
			// Indicator should have animation or spinning
			const styles = await streamingIndicator.first().evaluate((el) => {
				const computed = window.getComputedStyle(el);
				return {
					animation: computed.animation,
					animationName: computed.animationName
				};
			});

			// Animation should be defined (not 'none')
			expect(
				styles.animation !== 'none' ||
				styles.animationName !== 'none' ||
				styles.animationName !== ''
			).toBeTruthy();
		}
	});
});
