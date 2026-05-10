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
  AuditLogFilters - filter bar for the audit log table.
  Emits the new filter via `onapply` whenever the user changes a control.
-->
<script lang="ts">
	import type { AuditDecision, AuditFilter, DecidedBy } from '$types/validation';
	import { Button, Select, Input } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';

	interface Props {
		filter: AuditFilter;
		busy?: boolean;
		onapply: (filter: AuditFilter) => void;
		onexport: () => void;
		onpurge: () => void;
	}
	let { filter, busy = false, onapply, onexport, onpurge }: Props = $props();

	/**
	 * Converts an ISO 8601 timestamp into the `YYYY-MM-DDTHH:mm` local-time
	 * format expected by `<input type="datetime-local">`. Returns `''` for
	 * empty/invalid input so the picker stays cleared.
	 */
	function isoToDatetimeLocal(iso: string): string {
		if (!iso) return '';
		const d = new Date(iso);
		if (Number.isNaN(d.getTime())) return '';
		const pad = (n: number) => String(n).padStart(2, '0');
		return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
	}

	// svelte-ignore state_referenced_locally
	let toolName = $state(filter.toolName ?? '');
	// svelte-ignore state_referenced_locally
	let decision = $state<AuditDecision | ''>(filter.decision ?? '');
	// svelte-ignore state_referenced_locally
	let decidedBy = $state<DecidedBy | ''>(filter.decidedBy ?? '');
	// svelte-ignore state_referenced_locally
	let since = $state(isoToDatetimeLocal(filter.since ?? ''));
	// svelte-ignore state_referenced_locally
	let until = $state(isoToDatetimeLocal(filter.until ?? ''));

	const decisionOptions: SelectOption[] = [
		{ value: '', label: $i18n('audit_filter_decision_any') },
		{ value: 'approved', label: $i18n('audit_decision_approved') },
		{ value: 'rejected', label: $i18n('audit_decision_rejected') },
		{ value: 'skipped', label: $i18n('audit_decision_skipped') },
		{ value: 'timeout', label: $i18n('audit_decision_timeout') }
	];

	const decidedByOptions: SelectOption[] = [
		{ value: '', label: $i18n('audit_filter_decided_by_any') },
		{ value: 'user', label: $i18n('audit_decided_by_user') },
		{ value: 'auto', label: $i18n('audit_decided_by_auto') },
		{ value: 'timeout', label: $i18n('audit_decided_by_timeout') }
	];

	/** Parses a `datetime-local` value into an ISO string, or `undefined` if blank/invalid. */
	function datetimeLocalToIso(value: string): string | undefined {
		if (!value) return undefined;
		const ts = new Date(value).getTime();
		if (Number.isNaN(ts)) return undefined;
		return new Date(ts).toISOString();
	}

	function buildFilter(): AuditFilter {
		const next: AuditFilter = {};
		const trimmedTool = toolName.trim();
		if (trimmedTool) next.toolName = trimmedTool;
		if (decision) next.decision = decision;
		if (decidedBy) next.decidedBy = decidedBy;
		const sinceIso = datetimeLocalToIso(since);
		if (sinceIso) next.since = sinceIso;
		const untilIso = datetimeLocalToIso(until);
		if (untilIso) next.until = untilIso;
		return next;
	}

	function handleApply() {
		onapply(buildFilter());
	}

	function handleReset() {
		toolName = '';
		decision = '';
		decidedBy = '';
		since = '';
		until = '';
		onapply({});
	}

	function handleSubmit(event: Event) {
		event.preventDefault();
		handleApply();
	}
</script>

<form class="filters" onsubmit={handleSubmit} aria-label={$i18n('audit_filters_aria_label')}>
	<div class="filter-grid">
		<Input
			label={$i18n('audit_filter_tool')}
			type="search"
			bind:value={toolName}
			placeholder={$i18n('audit_filter_tool_placeholder')}
		/>

		<Select
			label={$i18n('audit_filter_decision')}
			value={decision}
			options={decisionOptions}
			onchange={(e) => (decision = e.currentTarget.value as AuditDecision | '')}
		/>

		<Select
			label={$i18n('audit_filter_decided_by')}
			value={decidedBy}
			options={decidedByOptions}
			onchange={(e) => (decidedBy = e.currentTarget.value as DecidedBy | '')}
		/>

		<Input label={$i18n('audit_filter_since')} type="datetime-local" bind:value={since} />

		<Input label={$i18n('audit_filter_until')} type="datetime-local" bind:value={until} />
	</div>

	<div class="filter-actions">
		<Button variant="primary" type="submit" disabled={busy}>
			{$i18n('audit_filter_apply')}
		</Button>
		<Button variant="ghost" type="button" onclick={handleReset} disabled={busy}>
			{$i18n('audit_filter_reset')}
		</Button>
		<div class="spacer"></div>
		<Button variant="secondary" type="button" onclick={onexport} disabled={busy}>
			{$i18n('audit_export_csv')}
		</Button>
		<Button variant="danger" type="button" onclick={onpurge} disabled={busy}>
			{$i18n('audit_purge_now')}
		</Button>
	</div>
</form>

<style>
	.filters {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		margin-bottom: var(--spacing-lg);
	}

	.filter-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
		gap: var(--spacing-md);
	}

	.filter-actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-sm);
		align-items: center;
	}

	.spacer {
		flex: 1;
	}
</style>
