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

/**
 * Store for the validation audit log page.
 * Backed by the commands (list_validation_audit, get_validation_audit_stats,
 * purge_validation_audit_now, export_validation_audit_csv).
 */

import { writable, derived, get } from 'svelte/store';
import { tauriInvoke as invoke } from '$lib/tauri';
import type {
	AuditFilter,
	AuditStats,
	ListAuditParams,
	ValidationAuditEntry
} from '$types/validation';
import { getErrorMessage } from '$lib/utils/error';

/** Default page size for the audit log table. */
export const AUDIT_LOG_PAGE_SIZE = 50;

/** Internal store state. */
interface AuditLogState {
	entries: ValidationAuditEntry[];
	stats: AuditStats | null;
	filter: AuditFilter;
	page: number;
	pageSize: number;
	/** True when the latest list_validation_audit call returned a full page (more rows likely). */
	hasMore: boolean;
	loading: boolean;
	loadingStats: boolean;
	exporting: boolean;
	purging: boolean;
	error: string | null;
}

const initialState: AuditLogState = {
	entries: [],
	stats: null,
	filter: {},
	page: 0,
	pageSize: AUDIT_LOG_PAGE_SIZE,
	hasMore: false,
	loading: false,
	loadingStats: false,
	exporting: false,
	purging: false,
	error: null
};

function createAuditLogStore() {
	const store = writable<AuditLogState>({ ...initialState, filter: {} });

	/** Reload the current page using the current filter. */
	async function reloadPage(): Promise<void> {
		const state = get(store);
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const params: ListAuditParams = {
				filter: state.filter,
				limit: state.pageSize,
				offset: state.page * state.pageSize
			};
			const entries = await invoke<ValidationAuditEntry[]>('list_validation_audit', { params });
			store.update((s) => ({
				...s,
				entries,
				hasMore: entries.length === s.pageSize,
				loading: false
			}));
		} catch (err) {
			store.update((s) => ({ ...s, error: getErrorMessage(err), loading: false }));
			throw err;
		}
	}

	return {
		subscribe: store.subscribe,

		/**
		 * Load the audit log stats (aggregate counts).
		 */
		async loadStats(): Promise<void> {
			store.update((s) => ({ ...s, loadingStats: true, error: null }));
			try {
				const stats = await invoke<AuditStats>('get_validation_audit_stats');
				store.update((s) => ({ ...s, stats, loadingStats: false }));
			} catch (err) {
				store.update((s) => ({ ...s, error: getErrorMessage(err), loadingStats: false }));
				throw err;
			}
		},

		/**
		 * Reload the current page (preserves filter and pagination).
		 */
		reload: reloadPage,

		/**
		 * Apply a new filter and reset to page 0.
		 * Pass an empty object to clear the filter.
		 */
		async applyFilter(filter: AuditFilter): Promise<void> {
			store.update((s) => ({ ...s, filter, page: 0 }));
			await reloadPage();
		},

		/**
		 * Move to the next page (no-op if `hasMore` is false).
		 */
		async nextPage(): Promise<void> {
			const state = get(store);
			if (!state.hasMore) return;
			store.update((s) => ({ ...s, page: s.page + 1 }));
			await reloadPage();
		},

		/**
		 * Move to the previous page (no-op if already on page 0).
		 */
		async prevPage(): Promise<void> {
			const state = get(store);
			if (state.page === 0) return;
			store.update((s) => ({ ...s, page: s.page - 1 }));
			await reloadPage();
		},

		/**
		 * Export the current filter as CSV. The backend returns the CSV string;
		 * the caller is responsible for saving it to disk.
		 */
		async exportCsv(): Promise<string> {
			store.update((s) => ({ ...s, exporting: true, error: null }));
			try {
				const state = get(store);
				const csv = await invoke<string>('export_validation_audit_csv', { filter: state.filter });
				store.update((s) => ({ ...s, exporting: false }));
				return csv;
			} catch (err) {
				store.update((s) => ({ ...s, error: getErrorMessage(err), exporting: false }));
				throw err;
			}
		},

		/**
		 * Purge audit entries older than the configured retention period.
		 * Returns the number of rows deleted.
		 */
		async purgeNow(): Promise<number> {
			store.update((s) => ({ ...s, purging: true, error: null }));
			try {
				const purged = await invoke<number>('purge_validation_audit_now');
				store.update((s) => ({ ...s, purging: false }));
				await reloadPage();
				return purged;
			} catch (err) {
				store.update((s) => ({ ...s, error: getErrorMessage(err), purging: false }));
				throw err;
			}
		},

		/**
		 * Clear the error message without affecting other state.
		 */
		clearError(): void {
			store.update((s) => ({ ...s, error: null }));
		},

		/**
		 * Reset the store to its initial empty state. Test-only convenience.
		 */
		reset(): void {
			store.set({ ...initialState, filter: {} });
		},

		/** Synchronous access for tests and outside-component callers. */
		getState(): AuditLogState {
			return get(store);
		}
	};
}

/** Singleton audit log store. */
export const auditLogStore = createAuditLogStore();

/** Derived: current entries. */
export const auditEntries = derived(auditLogStore, ($s) => $s.entries);

/** Derived: current stats (null until loaded). */
export const auditStats = derived(auditLogStore, ($s) => $s.stats);

/** Derived: current filter. */
export const auditFilter = derived(auditLogStore, ($s) => $s.filter);

/** Derived: current page index (zero-based). */
export const auditPage = derived(auditLogStore, ($s) => $s.page);

/** Derived: whether more pages are likely available. */
export const auditHasMore = derived(auditLogStore, ($s) => $s.hasMore);

/** Derived: any in-flight operation. */
export const auditLoading = derived(
	auditLogStore,
	($s) => $s.loading || $s.loadingStats || $s.exporting || $s.purging
);

/** Derived: latest error, if any. */
export const auditError = derived(auditLogStore, ($s) => $s.error);
