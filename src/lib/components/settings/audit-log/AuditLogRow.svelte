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
  AuditLogRow - one row of the validation audit log table.
-->
<script lang="ts">
	import type { ValidationAuditEntry } from '$types/validation';
	import { i18n } from '$lib/i18n';

	interface Props {
		entry: ValidationAuditEntry;
	}
	let { entry }: Props = $props();

	/**
	 * Format an ISO 8601 timestamp as `YYYY-MM-DD HH:mm:ss`. Returns the raw
	 * value if parsing fails so the user always sees something.
	 */
	function formatTimestamp(iso: string): string {
		const date = new Date(iso);
		if (Number.isNaN(date.getTime())) return iso;
		const pad = (n: number) => String(n).padStart(2, '0');
		return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
	}

	const decisionKeys = {
		approved: 'audit_decision_approved',
		rejected: 'audit_decision_rejected',
		skipped: 'audit_decision_skipped',
		timeout: 'audit_decision_timeout'
	} as const;

	const decidedByKeys = {
		user: 'audit_decided_by_user',
		auto: 'audit_decided_by_auto',
		timeout: 'audit_decided_by_timeout'
	} as const;

	const riskKeys = {
		low: 'audit_risk_low',
		medium: 'audit_risk_medium',
		high: 'audit_risk_high',
		critical: 'audit_risk_critical'
	} as const;
</script>

<tr>
	<td class="cell-time">{formatTimestamp(entry.decidedAt)}</td>
	<td class="cell-tool" title={entry.toolName}>{entry.toolName}</td>
	<td class="cell-decision">
		<span class="badge badge-{entry.decision}">
			{$i18n(decisionKeys[entry.decision])}
		</span>
	</td>
	<td class="cell-by">{$i18n(decidedByKeys[entry.decidedBy])}</td>
	<td class="cell-risk">
		<span class="risk-pill risk-{entry.riskLevel}">
			{$i18n(riskKeys[entry.riskLevel])}
		</span>
	</td>
	<td class="cell-preview" title={entry.promptPreview ?? ''}>
		{entry.promptPreview ?? '-'}
	</td>
</tr>

<style>
	tr:hover {
		background: var(--color-bg-hover);
	}

	td {
		padding: var(--spacing-sm) var(--spacing-md);
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
		border-bottom: 1px solid var(--color-border);
		vertical-align: middle;
	}

	.cell-time {
		font-family: var(--font-mono, monospace);
		white-space: nowrap;
		color: var(--color-text-secondary);
	}

	.cell-tool {
		font-weight: var(--font-weight-medium);
		max-width: 200px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.cell-preview {
		max-width: 320px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--color-text-secondary);
	}

	.badge {
		display: inline-block;
		padding: 2px var(--spacing-sm);
		border-radius: var(--border-radius-sm);
		font-size: var(--font-size-xs);
		font-weight: var(--font-weight-medium);
	}

	.badge-approved {
		background: var(--color-success-light);
		color: var(--color-success);
	}

	.badge-rejected {
		background: var(--color-danger-light);
		color: var(--color-danger);
	}

	.badge-timeout {
		background: var(--color-warning-light);
		color: var(--color-warning);
	}

	.badge-skipped {
		background: var(--color-bg-secondary);
		color: var(--color-text-secondary);
	}

	.risk-pill {
		display: inline-block;
		padding: 2px var(--spacing-sm);
		border-radius: var(--border-radius-sm);
		font-size: var(--font-size-xs);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.risk-low {
		background: var(--color-bg-secondary);
		color: var(--color-text-secondary);
	}

	.risk-medium {
		background: var(--color-info-light, var(--color-bg-secondary));
		color: var(--color-info, var(--color-text-primary));
	}

	.risk-high {
		background: var(--color-warning-light);
		color: var(--color-warning);
	}

	.risk-critical {
		background: var(--color-danger-light);
		color: var(--color-danger);
	}
</style>
