/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import {
	auditLogStore,
	auditEntries,
	auditStats,
	auditFilter,
	auditPage,
	auditHasMore,
	AUDIT_LOG_PAGE_SIZE
} from '../audit-log';
import type { AuditStats, ValidationAuditEntry } from '$types/validation';

vi.mock('$lib/tauri', () => ({
	tauriInvoke: vi.fn()
}));

import { tauriInvoke as invoke } from '$lib/tauri';

function makeEntry(id: string, decision: ValidationAuditEntry['decision']): ValidationAuditEntry {
	return {
		id,
		validationId: `vr-${id}`,
		toolName: 'delete_file',
		decision,
		decidedAt: '2026-04-25T12:00:00Z',
		decidedBy: decision === 'timeout' ? 'timeout' : 'user',
		riskLevel: 'medium'
	};
}

function pageOfEntries(count: number, prefix: string): ValidationAuditEntry[] {
	return Array.from({ length: count }, (_, i) => makeEntry(`${prefix}-${i}`, 'approved'));
}

describe('auditLogStore', () => {
	beforeEach(() => {
		auditLogStore.reset();
		vi.resetAllMocks();
	});

	describe('audit_log_list_paginates_correctly', () => {
		it('moves to next page only when the previous page was full, then back', async () => {
			const fullPage = pageOfEntries(AUDIT_LOG_PAGE_SIZE, 'a');
			const halfPage = pageOfEntries(10, 'b');

			vi.mocked(invoke)
				.mockResolvedValueOnce(fullPage)
				.mockResolvedValueOnce(halfPage)
				.mockResolvedValueOnce(fullPage);

			await auditLogStore.reload();
			expect(get(auditPage)).toBe(0);
			expect(get(auditEntries)).toHaveLength(AUDIT_LOG_PAGE_SIZE);
			expect(get(auditHasMore)).toBe(true);

			await auditLogStore.nextPage();
			expect(get(auditPage)).toBe(1);
			expect(get(auditEntries)).toHaveLength(10);
			expect(get(auditHasMore)).toBe(false);

			// nextPage is a no-op when hasMore is false
			await auditLogStore.nextPage();
			expect(get(auditPage)).toBe(1);

			await auditLogStore.prevPage();
			expect(get(auditPage)).toBe(0);
			expect(get(auditEntries)).toHaveLength(AUDIT_LOG_PAGE_SIZE);

			// prevPage is a no-op when already on page 0
			await auditLogStore.prevPage();
			expect(get(auditPage)).toBe(0);

			// Verify the offset was actually computed against pageSize
			expect(invoke).toHaveBeenNthCalledWith(2, 'list_validation_audit', {
				params: {
					filter: {},
					limit: AUDIT_LOG_PAGE_SIZE,
					offset: AUDIT_LOG_PAGE_SIZE
				}
			});
		});
	});

	describe('audit_log_filter_by_decision', () => {
		it('forwards the new filter to invoke and resets to page 0', async () => {
			vi.mocked(invoke)
				.mockResolvedValueOnce(pageOfEntries(AUDIT_LOG_PAGE_SIZE, 'a'))
				.mockResolvedValueOnce(pageOfEntries(AUDIT_LOG_PAGE_SIZE, 'b'))
				.mockResolvedValueOnce([makeEntry('1', 'rejected')]);

			await auditLogStore.reload();
			await auditLogStore.nextPage();
			expect(get(auditPage)).toBe(1);

			await auditLogStore.applyFilter({ decision: 'rejected' });

			expect(get(auditPage)).toBe(0);
			expect(get(auditFilter)).toEqual({ decision: 'rejected' });
			expect(get(auditEntries)).toEqual([makeEntry('1', 'rejected')]);
			expect(invoke).toHaveBeenLastCalledWith('list_validation_audit', {
				params: { filter: { decision: 'rejected' }, limit: AUDIT_LOG_PAGE_SIZE, offset: 0 }
			});
		});
	});

	describe('audit_log_export_csv_downloads', () => {
		it('returns the CSV string from the backend with the current filter', async () => {
			const csv = 'id,decision\nrow1,approved\n';
			vi.mocked(invoke)
				.mockResolvedValueOnce([makeEntry('1', 'approved')])
				.mockResolvedValueOnce(csv);

			await auditLogStore.applyFilter({ toolName: 'delete_file' });

			const result = await auditLogStore.exportCsv();

			expect(result).toBe(csv);
			expect(invoke).toHaveBeenLastCalledWith('export_validation_audit_csv', {
				filter: { toolName: 'delete_file' }
			});
		});

		it('surfaces export errors via the store error state', async () => {
			vi.mocked(invoke).mockResolvedValueOnce([]).mockRejectedValueOnce(new Error('disk full'));

			await auditLogStore.reload();

			await expect(auditLogStore.exportCsv()).rejects.toThrow('disk full');
			expect(auditLogStore.getState().error).toBe('disk full');
		});
	});

	describe('audit_log_stats_displays_counts', () => {
		it('loads the aggregate stats payload exposed by the backend', async () => {
			const stats: AuditStats = {
				total: 1234,
				byDecision: [
					{ label: 'approved', count: 890 },
					{ label: 'rejected', count: 234 },
					{ label: 'timeout', count: 110 }
				],
				byTool: [{ label: 'delete_file', count: 500 }]
			};
			vi.mocked(invoke).mockResolvedValueOnce(stats);

			await auditLogStore.loadStats();

			expect(get(auditStats)).toEqual(stats);
			expect(invoke).toHaveBeenCalledWith('get_validation_audit_stats');
		});

		it('surfaces stats load errors via the store error state', async () => {
			vi.mocked(invoke).mockRejectedValueOnce(new Error('db offline'));

			await expect(auditLogStore.loadStats()).rejects.toThrow('db offline');
			expect(auditLogStore.getState().error).toBe('db offline');
			expect(get(auditStats)).toBeNull();
		});
	});

	describe('audit_log_purge_now_refreshes_entries', () => {
		it('returns the deleted count and reloads the current page', async () => {
			vi.mocked(invoke)
				.mockResolvedValueOnce(pageOfEntries(AUDIT_LOG_PAGE_SIZE, 'a')) // initial reload
				.mockResolvedValueOnce(7) // purge_validation_audit_now
				.mockResolvedValueOnce([makeEntry('after', 'approved')]); // post-purge reload

			await auditLogStore.reload();
			const purged = await auditLogStore.purgeNow();

			expect(purged).toBe(7);
			expect(invoke).toHaveBeenNthCalledWith(2, 'purge_validation_audit_now');
			// After purge, the store reloads the page with the active filter.
			expect(invoke).toHaveBeenLastCalledWith('list_validation_audit', {
				params: { filter: {}, limit: AUDIT_LOG_PAGE_SIZE, offset: 0 }
			});
			expect(get(auditEntries)).toEqual([makeEntry('after', 'approved')]);
		});

		it('surfaces purge errors via the store error state', async () => {
			vi.mocked(invoke).mockRejectedValueOnce(new Error('db locked'));

			await expect(auditLogStore.purgeNow()).rejects.toThrow('db locked');
			expect(auditLogStore.getState().error).toBe('db locked');
			expect(auditLogStore.getState().purging).toBe(false);
		});
	});
});
