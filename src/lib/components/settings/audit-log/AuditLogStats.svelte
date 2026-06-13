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
  AuditLogStats - aggregate stats chip row.
  Shows total + decision buckets returned by `get_validation_audit_stats`.
-->
<script lang="ts">
	import type { AuditStats, AuditDecision } from '$types/validation';
	import { i18n } from '$lib/i18n';
	import { CircleCheck, CircleX, CircleMinus, Clock, Ban } from '@lucide/svelte';

	interface Props {
		stats: AuditStats | null;
	}
	let { stats }: Props = $props();

	/** Per-decision icon + accent colour for the chip. */
	const decisionMeta: Record<AuditDecision, { icon: typeof CircleCheck; color: string }> = {
		approved: { icon: CircleCheck, color: 'var(--color-success)' },
		rejected: { icon: CircleX, color: 'var(--color-error)' },
		skipped: { icon: CircleMinus, color: 'var(--color-text-secondary)' },
		timeout: { icon: Clock, color: 'var(--color-warning)' },
		blocked: { icon: Ban, color: 'var(--color-text-secondary)' }
	};

	/**
	 * Map a decision label coming from the backend to its i18n key.
	 * Unknown labels fall back to the literal value.
	 */
	function decisionLabel(label: string): string {
		const known: Record<AuditDecision, string> = {
			approved: 'audit_decision_approved',
			rejected: 'audit_decision_rejected',
			skipped: 'audit_decision_skipped',
			timeout: 'audit_decision_timeout',
			blocked: 'audit_decision_blocked'
		};
		const key = known[label as AuditDecision];
		return key ? $i18n(key) : label;
	}
</script>

<div class="stats-row" role="group" aria-label={$i18n('audit_stats_aria_label')}>
	<span class="metric-chip metric-total">
		<strong>{(stats?.total ?? 0).toLocaleString()}</strong>
		{$i18n('audit_stats_total')}
	</span>

	{#if stats}
		{#each stats.byDecision as bucket (bucket.label)}
			{@const meta = decisionMeta[bucket.label as AuditDecision]}
			<span class="metric-chip">
				{#if meta}
					{@const Icon = meta.icon}
					<Icon size={14} color={meta.color} aria-hidden="true" />
				{/if}
				<strong style={meta ? `color:${meta.color}` : undefined}
					>{bucket.count.toLocaleString()}</strong
				>
				{decisionLabel(bucket.label)}
			</span>
		{/each}
	{/if}
</div>

<style>
	.stats-row {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-lg);
	}

	.metric-chip {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-xs);
		padding: 0.4rem 0.8rem;
		font-size: var(--font-size-xs);
		font-variant-numeric: tabular-nums;
		color: var(--color-text-secondary);
		background: var(--surface-2);
		border: 1px solid var(--color-border-light);
		border-radius: var(--border-radius-full);
		white-space: nowrap;
	}

	.metric-chip strong {
		color: var(--color-text-primary);
		font-weight: var(--font-weight-semibold);
	}

	.metric-total strong {
		color: var(--color-accent-deep);
	}
</style>
