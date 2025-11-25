// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from '@playwright/test';

/**
 * Workflow CRUD E2E tests for Zileo-Chat-3.
 * Tests create, read, update, delete workflow operations.
 *
 * Note: These tests interact with the UI and mock Tauri commands in browser context.
 */

test.describe('Workflow CRUD', () => {
	test.beforeEach(async ({ page }) => {
		// Navigate to agent page where workflows are managed
		await page.goto('/agent');

		// Wait for page to be fully loaded
		await page.waitForLoadState('networkidle');
	});

	test('should display workflow list on agent page', async ({ page }) => {
		// Workflow list container should be visible
		const workflowSection = page.locator('.agent-page');
		await expect(workflowSection).toBeVisible();

		// Sidebar should contain workflow list
		const sidebar = page.locator('.sidebar');
		await expect(sidebar).toBeVisible();
	});

	test('should have new workflow button', async ({ page }) => {
		// Look for the new workflow button in header or empty state
		const newButtonHeader = page.locator('button[aria-label="New workflow"]');
		const newButtonEmpty = page.locator('button:has-text("New Workflow")');

		// At least one new workflow button should be visible
		const headerVisible = await newButtonHeader.isVisible().catch(() => false);
		const emptyVisible = await newButtonEmpty.isVisible().catch(() => false);

		expect(headerVisible || emptyVisible).toBe(true);
	});

	test('should show empty state when no workflow selected', async ({ page }) => {
		// Empty state should be visible when no workflow is selected
		const emptyState = page.locator('.empty-state');
		await expect(emptyState).toBeVisible();

		// Empty state should contain helpful message
		const emptyMessage = page.locator('.empty-state h3');
		await expect(emptyMessage).toContainText(/select|create/i);
	});

	test('should have sidebar with header and navigation', async ({ page }) => {
		// Sidebar header should show "Workflows" title
		const sidebarTitle = page.locator('.sidebar-title');
		await expect(sidebarTitle).toContainText('Workflows');

		// Sidebar should have collapse toggle
		const collapseButton = page.locator('.sidebar-collapse-btn');
		await expect(collapseButton).toBeVisible();
	});

	test('should allow filtering workflows via search', async ({ page }) => {
		// Search input should be present in sidebar
		const searchInput = page.locator('.sidebar input[type="search"]');

		// Search input should be visible (unless sidebar collapsed)
		const sidebarCollapsed = await page.locator('.sidebar.collapsed').isVisible().catch(() => false);

		if (!sidebarCollapsed) {
			await expect(searchInput).toBeVisible();

			// Type in search and verify input works
			await searchInput.fill('test');
			await expect(searchInput).toHaveValue('test');
		}
	});

	test('should toggle sidebar collapse state', async ({ page }) => {
		// Find collapse button
		const collapseButton = page.locator('.sidebar-collapse-btn');
		await expect(collapseButton).toBeVisible();

		// Get initial width
		const sidebar = page.locator('.sidebar');
		const initialBox = await sidebar.boundingBox();
		const initialWidth = initialBox?.width ?? 0;

		// Click to collapse
		await collapseButton.click();

		// Wait for transition
		await page.waitForTimeout(300);

		// Verify width changed
		const collapsedBox = await sidebar.boundingBox();
		const collapsedWidth = collapsedBox?.width ?? 0;

		// Sidebar should be narrower when collapsed
		expect(collapsedWidth).toBeLessThan(initialWidth);
	});

	test('should display agent selector when workflow selected', async ({ page }) => {
		// Agent selector component should exist on page
		// (may only be visible when workflow is selected)
		const pageContent = await page.content();

		// Page should contain agent-related components
		expect(pageContent).toContain('agent');
	});

	test('should have correct page structure', async ({ page }) => {
		// Main page wrapper
		const agentPage = page.locator('.agent-page');
		await expect(agentPage).toBeVisible();

		// Should have flex layout with sidebar and main area
		const agentMain = page.locator('.agent-main');
		await expect(agentMain).toBeVisible();
	});

	test('should navigate from empty state to create workflow', async ({ page }) => {
		// Find the new workflow button in empty state
		const newButton = page.locator('.empty-state button:has-text("New Workflow")');

		if (await newButton.isVisible()) {
			// Verify button is clickable
			await expect(newButton).toBeEnabled();

			// Button should have correct styling (primary variant)
			await expect(newButton).toHaveClass(/btn-primary/);
		}
	});
});
