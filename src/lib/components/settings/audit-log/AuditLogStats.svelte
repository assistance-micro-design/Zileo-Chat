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
  AuditLogStats - aggregate stats card row.
  Shows total + decision buckets returned by `get_validation_audit_stats`.
-->
<script lang="ts">
	import type { AuditStats, AuditDecision } from '$types/validation';
	import { i18n } from '$lib/i18n';

	interface Props {
		stats: AuditStats | null;
	}
	let { stats }: Props = $props();

	/**
	 * Map a decision label coming from the backend to its i18n key.
	 * Unknown labels fall back to the literal value.
	 */
	function decisionLabel(label: string): string {
		const known: Record<AuditDecision, string> = {
			approved: 'audit_decision_approved',
			rejected: 'audit_decision_rejected',
			skipped: 'audit_decision_skipped',
			timeout: 'audit_decision_timeout'
		};
		const key = known[label as AuditDecision];
		return key ? $i18n(key) : label;
	}
</script>

<div class="stats-row" role="group" aria-label={$i18n('audit_stats_aria_label')}>
	<div class="stat-card stat-total">
		<span class="stat-label">{$i18n('audit_stats_total')}</span>
		<span class="stat-value">{stats?.total ?? 0}</span>
	</div>

	{#if stats}
		{#each stats.byDecision as bucket (bucket.label)}
			<div class="stat-card stat-decision-{bucket.label}">
				<span class="stat-label">{decisionLabel(bucket.label)}</span>
				<span class="stat-value">{bucket.count}</span>
			</div>
		{/each}
	{/if}
</div>

<style>
	.stats-row {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
		gap: var(--spacing-md);
		margin-bottom: var(--spacing-lg);
	}

	.stat-card {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
	}

	.stat-label {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.stat-value {
		font-size: var(--font-size-xl);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.stat-decision-approved .stat-value {
		color: var(--color-success);
	}

	.stat-decision-rejected .stat-value {
		color: var(--color-danger);
	}

	.stat-decision-timeout .stat-value {
		color: var(--color-warning);
	}

	.stat-decision-skipped .stat-value {
		color: var(--color-text-secondary);
	}
</style>
