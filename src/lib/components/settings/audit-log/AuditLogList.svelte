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
  AuditLogList - paginated table of validation audit entries.
-->
<script lang="ts">
	import type { ValidationAuditEntry } from '$types/validation';
	import { Button, Spinner } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import AuditLogRow from './AuditLogRow.svelte';

	interface Props {
		entries: ValidationAuditEntry[];
		page: number;
		hasMore: boolean;
		loading: boolean;
		onnext: () => void;
		onprev: () => void;
	}
	let { entries, page, hasMore, loading, onnext, onprev }: Props = $props();
</script>

<div class="list-wrapper">
	{#if loading}
		<div class="loading-overlay" aria-busy="true" aria-live="polite">
			<Spinner />
			<span>{$i18n('audit_log_loading')}</span>
		</div>
	{:else if entries.length === 0}
		<div class="empty-state">
			<p>{$i18n('audit_log_empty')}</p>
		</div>
	{:else}
		<div class="table-scroll">
			<table class="audit-table">
				<thead>
					<tr>
						<th scope="col">{$i18n('audit_col_time')}</th>
						<th scope="col">{$i18n('audit_col_tool')}</th>
						<th scope="col">{$i18n('audit_col_decision')}</th>
						<th scope="col">{$i18n('audit_col_decided_by')}</th>
						<th scope="col">{$i18n('audit_col_risk')}</th>
						<th scope="col">{$i18n('audit_col_prompt')}</th>
					</tr>
				</thead>
				<tbody>
					{#each entries as entry (entry.id)}
						<AuditLogRow {entry} />
					{/each}
				</tbody>
			</table>
		</div>
	{/if}

	<div class="pagination">
		<Button variant="ghost" type="button" onclick={onprev} disabled={loading || page === 0}>
			{$i18n('audit_pagination_prev')}
		</Button>
		<span class="page-indicator">
			{$i18n('audit_pagination_page')} {page + 1}
		</span>
		<Button variant="ghost" type="button" onclick={onnext} disabled={loading || !hasMore}>
			{$i18n('audit_pagination_next')}
		</Button>
	</div>
</div>

<style>
	.list-wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		position: relative;
	}

	.loading-overlay,
	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-2xl);
		color: var(--color-text-secondary);
	}

	.table-scroll {
		overflow-x: auto;
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
	}

	.audit-table {
		width: 100%;
		border-collapse: collapse;
	}

	.audit-table thead {
		background: var(--color-bg-secondary);
	}

	.audit-table th {
		text-align: left;
		font-weight: var(--font-weight-semibold);
		font-size: var(--font-size-xs);
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--color-text-secondary);
		padding: var(--spacing-sm) var(--spacing-md);
		border-bottom: 1px solid var(--color-border);
		white-space: nowrap;
	}

	.pagination {
		display: flex;
		justify-content: center;
		align-items: center;
		gap: var(--spacing-md);
		padding-top: var(--spacing-md);
	}

	.page-indicator {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}
</style>
