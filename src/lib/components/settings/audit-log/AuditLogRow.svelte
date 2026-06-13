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
	import type { AuditDecision, RiskLevel, ValidationAuditEntry } from '$types/validation';
	import { Badge } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';

	interface Props {
		entry: ValidationAuditEntry;
	}
	let { entry }: Props = $props();

	/**
	 * Format an ISO 8601 timestamp as `YYYY-MM-DD HH:mm`. Returns the raw
	 * value if parsing fails so the user always sees something.
	 */
	function formatTimestamp(iso: string): string {
		const date = new Date(iso);
		if (Number.isNaN(date.getTime())) return iso;
		const pad = (n: number) => String(n).padStart(2, '0');
		return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
	}

	const decisionKeys = {
		approved: 'audit_decision_approved',
		rejected: 'audit_decision_rejected',
		skipped: 'audit_decision_skipped',
		timeout: 'audit_decision_timeout',
		blocked: 'audit_decision_blocked'
	} as const;

	const decidedByKeys = {
		user: 'audit_decided_by_user',
		auto: 'audit_decided_by_auto',
		timeout: 'audit_decided_by_timeout',
		policy: 'audit_decided_by_policy',
		pre_approved: 'audit_decided_by_pre_approved'
	} as const;

	const riskKeys = {
		low: 'audit_risk_low',
		medium: 'audit_risk_medium',
		high: 'audit_risk_high',
		critical: 'audit_risk_critical'
	} as const;

	type BadgeVariant = 'primary' | 'success' | 'warning' | 'error' | 'neutral';

	const decisionVariants: Record<AuditDecision, BadgeVariant> = {
		approved: 'success',
		rejected: 'error',
		skipped: 'neutral',
		timeout: 'warning',
		blocked: 'neutral'
	};

	const riskVariants: Record<RiskLevel, BadgeVariant> = {
		low: 'primary',
		medium: 'warning',
		high: 'warning',
		critical: 'error'
	};
</script>

<tr>
	<td class="cell-time mono">{formatTimestamp(entry.decidedAt)}</td>
	<td class="cell-tool mono" title={entry.toolName}>{entry.toolName}</td>
	<td>
		<Badge variant={decisionVariants[entry.decision]}>
			{$i18n(decisionKeys[entry.decision])}
		</Badge>
	</td>
	<td class="cell-by">{$i18n(decidedByKeys[entry.decidedBy])}</td>
	<td>
		<Badge variant={riskVariants[entry.riskLevel]}>
			{$i18n(riskKeys[entry.riskLevel])}
		</Badge>
	</td>
	<td class="cell-preview" title={entry.promptPreview ?? ''}>
		{entry.promptPreview ?? '-'}
	</td>
</tr>

<style>
	.mono {
		font-family: var(--font-mono);
		font-size: var(--font-size-xs);
	}

	.cell-time {
		white-space: nowrap;
		color: var(--color-text-secondary);
	}

	.cell-tool {
		max-width: 200px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.cell-by {
		font-size: var(--font-size-xs);
		white-space: nowrap;
		color: var(--color-text-secondary);
	}

	.cell-preview {
		max-width: 280px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}
</style>
