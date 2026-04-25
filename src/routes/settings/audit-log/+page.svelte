<!--
  Copyright 2025 Assistance Micro Design

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

<!--
Settings > Audit Log Page
Browse, filter, export and purge the validation audit log (see commands/validation_audit.rs).
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { save } from '@tauri-apps/plugin-dialog';
	import { invoke } from '@tauri-apps/api/core';
	import SettingsSectionHeader from '$lib/components/settings/SettingsSectionHeader.svelte';
	import {
		AuditLogStats,
		AuditLogFilters,
		AuditLogList
	} from '$lib/components/settings/audit-log';
	import { ErrorBanner } from '$lib/components/ui';
	import {
		auditLogStore,
		auditEntries,
		auditStats,
		auditFilter,
		auditPage,
		auditHasMore,
		auditLoading,
		auditError
	} from '$lib/stores/audit-log';
	import type { AuditFilter } from '$types/validation';
	import { toastStore } from '$lib/stores/toast';
	import { i18n } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import { onSettingsRefresh } from '$lib/utils/settings-refresh';

	onMount(() => {
		void auditLogStore.loadStats();
		void auditLogStore.reload();
	});

	onSettingsRefresh(() => {
		void auditLogStore.loadStats();
		void auditLogStore.reload();
	});

	function showToast(type: 'success' | 'error' | 'info', text: string) {
		toastStore.add({ type, title: text, message: '', persistent: false, duration: 5000 });
	}

	async function handleApply(filter: AuditFilter) {
		try {
			await auditLogStore.applyFilter(filter);
		} catch (err) {
			showToast('error', getErrorMessage(err));
		}
	}

	async function handleExport() {
		try {
			const csv = await auditLogStore.exportCsv();
			const defaultFilename = `audit-log-${new Date().toISOString().slice(0, 10)}.csv`;
			const filePath = await save({
				defaultPath: defaultFilename,
				filters: [{ name: 'CSV', extensions: ['csv'] }],
				title: $i18n('audit_export_dialog_title')
			});
			if (!filePath) return;
			await invoke('save_export_to_file', { path: filePath, content: csv });
			showToast('success', $i18n('audit_export_success'));
		} catch (err) {
			showToast('error', getErrorMessage(err));
		}
	}

	async function handlePurge() {
		try {
			const purged = await auditLogStore.purgeNow();
			await auditLogStore.loadStats();
			showToast(
				'success',
				$i18n('validation_audit_purge_success').replace('{count}', String(purged))
			);
		} catch (err) {
			showToast('error', getErrorMessage(err));
		}
	}

	async function handleNext() {
		try {
			await auditLogStore.nextPage();
		} catch (err) {
			showToast('error', getErrorMessage(err));
		}
	}

	async function handlePrev() {
		try {
			await auditLogStore.prevPage();
		} catch (err) {
			showToast('error', getErrorMessage(err));
		}
	}
</script>

<section class="settings-section">
	<SettingsSectionHeader
		titleKey="settings_audit_log"
		descriptionKey="audit_log_description"
		helpTitleKey="audit_log_help_title"
		helpDescriptionKey="audit_log_help_description"
		helpTutorialKey="audit_log_help_tutorial"
	/>

	{#if $auditError}
		<ErrorBanner
			message={$auditError}
			variant="error"
			onDismiss={() => auditLogStore.clearError()}
		/>
	{/if}

	<AuditLogStats stats={$auditStats} />

	<AuditLogFilters
		filter={$auditFilter}
		busy={$auditLoading}
		onapply={handleApply}
		onexport={handleExport}
		onpurge={handlePurge}
	/>

	<AuditLogList
		entries={$auditEntries}
		page={$auditPage}
		hasMore={$auditHasMore}
		loading={$auditLoading}
		onnext={handleNext}
		onprev={handlePrev}
	/>
</section>
