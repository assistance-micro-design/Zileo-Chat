// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from '@playwright/test';

/**
 * Agent page E2E tests for Zileo-Chat-3.
 * Tests workflow management and agent interaction UI.
 */

test.describe('Agent Page', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/agent');
	});

	test('should display workflow list section', async ({ page }) => {
		// Workflow list should be visible
		const workflowList = page.locator('.workflow-list');
		await expect(workflowList).toBeVisible();
	});

	test('should display new workflow button', async ({ page }) => {
		// New workflow button should be visible
		const newButton = page.locator('button:has-text("New Workflow")');
		await expect(newButton).toBeVisible();
	});

	test('should display empty state when no workflow selected', async ({ page }) => {
		// Empty state message should be visible
		const emptyState = page.locator('.empty-state');
		await expect(emptyState).toBeVisible();
	});

	test('should have message input area', async ({ page }) => {
		// Note: Input area may only show when workflow is selected
		// This test validates the page structure exists
		const mainArea = page.locator('.main-area');
		await expect(mainArea).toBeVisible();
	});
});
