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
  MetricsBar Component
  Displays workflow execution metrics in a horizontal bar.
  Shows duration, tokens, provider, and cost.

  @example
  <MetricsBar metrics={workflowMetrics} />
-->
<script lang="ts">
	import type { WorkflowMetrics } from '$types/workflow';
	import { Clock, Hash, Server, DollarSign } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * MetricsBar props
	 */
	interface Props {
		/** Workflow metrics data */
		metrics: WorkflowMetrics;
		/** Whether to show compact view */
		compact?: boolean;
	}

	let { metrics, compact = false }: Props = $props();

	/**
	 * Format duration from milliseconds
	 */
	function formatDuration(ms: number): string {
		if (ms < 1000) return `${ms}ms`;
		if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
		return `${Math.floor(ms / 60000)}m ${Math.round((ms % 60000) / 1000)}s`;
	}

	/**
	 * Format cost in USD
	 */
	function formatCost(usd: number): string {
		if (usd === 0) return $i18n('workflow_metrics_free');
		if (usd < 0.01) return '<$0.01';
		return `$${usd.toFixed(4)}`;
	}

	/**
	 * Format token count
	 */
	function formatTokens(input: number, output: number): string {
		return `${input.toLocaleString()} / ${output.toLocaleString()}`;
	}
</script>

<div class="metrics-bar" class:compact role="status" aria-label={$i18n('workflow_token_arialabel')}>
	<div class="metric" title={$i18n('workflow_metrics_duration')}>
		<Clock size={14} />
		<span class="metric-value">{formatDuration(metrics.duration_ms)}</span>
		{#if !compact}
			<span class="metric-label">{$i18n('workflow_metrics_duration')}</span>
		{/if}
	</div>

	<div class="metric" title={$i18n('workflow_metrics_tokens_title')}>
		<Hash size={14} />
		<span class="metric-value">{formatTokens(metrics.tokens_input, metrics.tokens_output)}</span>
		{#if !compact}
			<span class="metric-label">{$i18n('workflow_metrics_tokens')}</span>
		{/if}
	</div>

	<div class="metric" title={$i18n('workflow_metrics_provider_title')}>
		<Server size={14} />
		<span class="metric-value">{metrics.provider}</span>
		{#if !compact}
			<span class="metric-label">{metrics.model}</span>
		{/if}
	</div>

	{#if metrics.cost_usd > 0 || !compact}
		<div class="metric" title={$i18n('workflow_metrics_cost_title')}>
			<DollarSign size={14} />
			<span class="metric-value">{formatCost(metrics.cost_usd)}</span>
			{#if !compact}
				<span class="metric-label">{$i18n('workflow_metrics_cost')}</span>
			{/if}
		</div>
	{/if}
</div>

<style>
	.metrics-bar {
		display: flex;
		align-items: center;
		gap: var(--spacing-lg);
		padding: var(--spacing-sm) var(--spacing-md);
		background: var(--color-bg-secondary);
		border-top: 1px solid var(--color-border);
		font-size: var(--font-size-xs);
		flex-wrap: wrap;
	}

	.metrics-bar.compact {
		gap: var(--spacing-md);
		padding: var(--spacing-xs) var(--spacing-sm);
	}

	.metric {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		color: var(--color-text-secondary);
	}

	.metric-value {
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
		font-family: var(--font-mono);
	}

	.metric-label {
		color: var(--color-text-tertiary);
	}
</style>
