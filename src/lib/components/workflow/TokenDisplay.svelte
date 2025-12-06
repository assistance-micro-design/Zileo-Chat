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
  TokenDisplay Component
  Displays real-time token usage with context window progress,
  cost estimation, and token generation speed.

  Design: Follows MetricsBar pattern with enhanced visual indicators.

  Features:
  - Context window progress bar with gradient colors (green -> yellow -> red)
  - Warning states at 75%, 90%, 100% with animated indicators
  - Token counts with input/output color differentiation
  - Cost display with accent color
  - Speed indicator during streaming with pulse animation

  @example
  <TokenDisplay data={tokenData} />
  <TokenDisplay data={tokenData} compact />
-->
<script lang="ts">
	import type { TokenDisplayData } from '$types/workflow';
	import {
		AlertTriangle,
		Gauge,
		ArrowDownToLine,
		TrendingUp,
		CircleDollarSign,
		Activity
	} from 'lucide-svelte';
	import { i18n } from '$lib/i18n';
	import { HelpButton } from '$lib/components/ui';

	/**
	 * TokenDisplay props
	 */
	interface Props {
		/** Token display data */
		data: TokenDisplayData;
		/** Compact mode (minimal display) */
		compact?: boolean;
	}

	let { data, compact = false }: Props = $props();

	/**
	 * Calculate context usage percentage
	 * Uses tokens_input which represents the actual context size at last API call
	 * (not cumulative, but the real context window usage)
	 */
	const contextUsed = $derived(data.tokens_input);
	const contextPercentage = $derived(
		data.context_max > 0 ? Math.min((contextUsed / data.context_max) * 100, 100) : 0
	);

	/**
	 * Determine warning level based on context usage
	 */
	type WarningLevel = 'normal' | 'warning' | 'critical' | 'full';
	const warningLevel = $derived<WarningLevel>(
		contextPercentage >= 100 ? 'full' :
		contextPercentage >= 90 ? 'critical' :
		contextPercentage >= 75 ? 'warning' :
		'normal'
	);

	/**
	 * Format token count with K/M suffix
	 */
	function formatTokens(count: number): string {
		if (count >= 1000000) {
			return `${(count / 1000000).toFixed(1)}M`;
		}
		if (count >= 1000) {
			return `${(count / 1000).toFixed(1)}K`;
		}
		return count.toLocaleString();
	}

	/**
	 * Format cost in USD
	 */
	function formatCost(usd: number): string {
		if (usd === 0) return $i18n('workflow_metrics_free');
		if (usd < 0.0001) return '<$0.0001';
		if (usd < 0.01) return `$${usd.toFixed(4)}`;
		return `$${usd.toFixed(2)}`;
	}

	/**
	 * Format speed in tokens per second
	 */
	function formatSpeed(tks: number | undefined): string {
		if (tks === undefined || tks === 0) return '-';
		return `${tks.toFixed(1)}`;
	}
</script>

<div
	class="token-display"
	class:compact
	class:warning={warningLevel === 'warning'}
	class:critical={warningLevel === 'critical'}
	class:full={warningLevel === 'full'}
	role="status"
	aria-label={$i18n('workflow_token_arialabel')}
>
	<!-- Help Button -->
	<HelpButton
		titleKey="help_token_display_title"
		descriptionKey="help_token_display_description"
		tutorialKey="help_token_display_tutorial"
	/>

	<!-- Context Progress Section -->
	<div class="metric context-metric">
		<div class="metric-icon context-icon" class:warning={warningLevel === 'warning'} class:critical={warningLevel === 'critical' || warningLevel === 'full'}>
			<Gauge size={16} />
			{#if warningLevel !== 'normal'}
				<span class="warning-badge">
					<AlertTriangle size={10} />
				</span>
			{/if}
		</div>
		<div class="context-content">
			<div class="context-progress">
				<div class="progress-bar">
					<div
						class="progress-fill"
						class:warning={warningLevel === 'warning'}
						class:critical={warningLevel === 'critical' || warningLevel === 'full'}
						style="width: {contextPercentage}%"
						role="progressbar"
						aria-valuenow={contextPercentage}
						aria-valuemin={0}
						aria-valuemax={100}
					></div>
				</div>
				<span class="percentage-value" class:warning={warningLevel === 'warning'} class:critical={warningLevel === 'critical' || warningLevel === 'full'}>
					{contextPercentage.toFixed(0)}%
				</span>
			</div>
			{#if !compact}
				<span class="context-label">{formatTokens(contextUsed)} / {formatTokens(data.context_max)} {$i18n('workflow_token_context')}</span>
			{/if}
		</div>
	</div>

	<!-- Separator -->
	<div class="separator"></div>

	<!-- Current Tokens -->
	<div class="metric tokens-metric">
		<div class="metric-icon input-icon">
			<ArrowDownToLine size={14} />
		</div>
		<div class="token-pair">
			<span class="token-value input-value">{formatTokens(data.tokens_input)}</span>
			<span class="token-separator">/</span>
			<span class="token-value output-value">{formatTokens(data.tokens_output)}</span>
		</div>
		{#if !compact}
			<span class="metric-label">{$i18n('workflow_token_in_out')}</span>
		{/if}
	</div>

	<!-- Cumulative Tokens -->
	{#if !compact}
		<div class="metric tokens-metric cumulative">
			<div class="metric-icon total-icon">
				<TrendingUp size={14} />
			</div>
			<div class="token-pair">
				<span class="token-value input-value">{formatTokens(data.cumulative_input)}</span>
				<span class="token-separator">/</span>
				<span class="token-value output-value">{formatTokens(data.cumulative_output)}</span>
			</div>
			<span class="metric-label">{$i18n('workflow_token_total')}</span>
		</div>
	{/if}

	<!-- Separator -->
	<div class="separator"></div>

	<!-- Cost -->
	<div class="metric cost-metric">
		<div class="metric-icon cost-icon">
			<CircleDollarSign size={14} />
		</div>
		<span class="cost-value">{formatCost(data.cost_usd)}</span>
		<span class="cost-estimate">{$i18n('workflow_cost_estimate')}</span>
		{#if !compact && data.cumulative_cost_usd > 0 && data.cumulative_cost_usd !== data.cost_usd}
			<span class="cost-total">({formatCost(data.cumulative_cost_usd)})</span>
		{/if}
	</div>

	<!-- Speed (only during streaming) -->
	{#if data.is_streaming}
		<div class="metric speed-metric">
			<div class="metric-icon speed-icon">
				<Activity size={14} />
			</div>
			<span class="speed-value">{formatSpeed(data.speed_tks)}</span>
			<span class="speed-unit">{$i18n('workflow_token_speed')}</span>
		</div>
	{/if}
</div>

<style>
	/* Main container - follows MetricsBar pattern */
	.token-display {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-sm) var(--spacing-md);
		background: var(--color-bg-secondary);
		border-top: 1px solid var(--color-border);
		font-size: var(--font-size-xs);
		flex-wrap: wrap;
	}

	.token-display.compact {
		gap: var(--spacing-sm);
		padding: var(--spacing-xs) var(--spacing-sm);
	}

	/* Warning States */
	.token-display.warning {
		border-left: 3px solid var(--color-warning);
		background: linear-gradient(90deg, color-mix(in srgb, var(--color-warning-light) 20%, transparent), transparent 30%);
	}

	.token-display.critical {
		border-left: 3px solid var(--color-error);
		background: linear-gradient(90deg, color-mix(in srgb, var(--color-error-light) 25%, transparent), transparent 30%);
	}

	.token-display.full {
		border-left: 3px solid var(--color-error);
		background: linear-gradient(90deg, color-mix(in srgb, var(--color-error-light) 40%, var(--color-bg-secondary)), var(--color-bg-secondary) 50%);
	}

	/* Separator */
	.separator {
		width: 1px;
		height: 24px;
		background: var(--color-border);
		flex-shrink: 0;
	}

	.compact .separator {
		height: 16px;
	}

	/* Metric Icons */
	.metric-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		position: relative;
	}

	.context-icon {
		color: var(--color-accent);
	}

	.context-icon.warning {
		color: var(--color-warning);
	}

	.context-icon.critical {
		color: var(--color-error);
		animation: pulse-icon 1.5s ease-in-out infinite;
	}

	.input-icon {
		color: var(--color-accent);
	}

	.total-icon {
		color: var(--color-text-tertiary);
	}

	.cost-icon {
		color: var(--color-secondary);
	}

	.speed-icon {
		color: var(--color-status-running);
		animation: pulse-icon 1s ease-in-out infinite;
	}

	/* Warning Badge */
	.warning-badge {
		position: absolute;
		top: -4px;
		right: -4px;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 14px;
		height: 14px;
		background: var(--color-warning);
		border-radius: var(--border-radius-full);
		color: white;
	}

	.critical .warning-badge,
	.full .warning-badge {
		background: var(--color-error);
		animation: pulse-badge 1s ease-in-out infinite;
	}

	/* Metric base */
	.metric {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.metric-label {
		color: var(--color-text-tertiary);
		font-size: 10px;
		text-transform: uppercase;
		letter-spacing: 0.5px;
	}

	/* Context Progress Section */
	.context-metric {
		min-width: 140px;
	}

	.compact .context-metric {
		min-width: 100px;
	}

	.context-content {
		display: flex;
		flex-direction: column;
		gap: 2px;
		flex: 1;
	}

	.context-progress {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		width: 100%;
	}

	.context-label {
		font-size: 10px;
		color: var(--color-text-tertiary);
	}

	/* Progress bar */
	.progress-bar {
		flex: 1;
		height: 8px;
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-full);
		overflow: hidden;
		box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.1);
	}

	.compact .progress-bar {
		height: 6px;
	}

	.progress-fill {
		height: 100%;
		background: linear-gradient(90deg, var(--color-success), color-mix(in srgb, var(--color-success) 80%, var(--color-accent)));
		border-radius: var(--border-radius-full);
		transition: width var(--transition-base), background var(--transition-base);
		box-shadow: 0 0 4px color-mix(in srgb, var(--color-success) 50%, transparent);
	}

	.progress-fill.warning {
		background: linear-gradient(90deg, var(--color-warning), color-mix(in srgb, var(--color-warning) 80%, var(--color-secondary)));
		box-shadow: 0 0 4px color-mix(in srgb, var(--color-warning) 50%, transparent);
	}

	.progress-fill.critical {
		background: linear-gradient(90deg, var(--color-error), color-mix(in srgb, var(--color-error) 80%, var(--color-secondary)));
		box-shadow: 0 0 6px color-mix(in srgb, var(--color-error) 60%, transparent);
		animation: pulse-bar 1.5s ease-in-out infinite;
	}

	.percentage-value {
		font-family: var(--font-mono);
		font-weight: var(--font-weight-semibold);
		font-size: var(--font-size-xs);
		color: var(--color-success);
		min-width: 32px;
		text-align: right;
	}

	.percentage-value.warning {
		color: var(--color-warning);
	}

	.percentage-value.critical {
		color: var(--color-error);
		animation: pulse-text 1.5s ease-in-out infinite;
	}

	/* Token Values */
	.tokens-metric {
		gap: var(--spacing-xs);
	}

	.tokens-metric.cumulative {
		opacity: 0.8;
	}

	.token-pair {
		display: flex;
		align-items: center;
		gap: 2px;
		font-family: var(--font-mono);
	}

	.token-value {
		font-weight: var(--font-weight-medium);
	}

	.input-value {
		color: var(--color-accent);
	}

	.output-value {
		color: var(--color-secondary);
	}

	.token-separator {
		color: var(--color-text-tertiary);
		font-weight: var(--font-weight-normal);
	}

	/* Cost */
	.cost-metric {
		gap: var(--spacing-xs);
	}

	.cost-value {
		font-family: var(--font-mono);
		font-weight: var(--font-weight-semibold);
		color: var(--color-secondary);
	}

	.cost-estimate {
		font-size: 10px;
		color: var(--color-text-tertiary);
		font-style: italic;
	}

	.cost-total {
		font-family: var(--font-mono);
		font-size: 10px;
		color: var(--color-text-tertiary);
	}

	/* Speed */
	.speed-metric {
		gap: 2px;
		padding: var(--spacing-xs) var(--spacing-sm);
		background: color-mix(in srgb, var(--color-status-running) 15%, transparent);
		border-radius: var(--border-radius-md);
	}

	.speed-value {
		font-family: var(--font-mono);
		font-weight: var(--font-weight-semibold);
		color: var(--color-status-running);
	}

	.speed-unit {
		font-size: 10px;
		color: var(--color-status-running);
		opacity: 0.8;
	}

	/* Animations */
	@keyframes pulse-icon {
		0%, 100% {
			opacity: 1;
			transform: scale(1);
		}
		50% {
			opacity: 0.7;
			transform: scale(1.1);
		}
	}

	@keyframes pulse-badge {
		0%, 100% {
			transform: scale(1);
			box-shadow: 0 0 0 0 color-mix(in srgb, var(--color-error) 40%, transparent);
		}
		50% {
			transform: scale(1.1);
			box-shadow: 0 0 0 4px color-mix(in srgb, var(--color-error) 0%, transparent);
		}
	}

	@keyframes pulse-bar {
		0%, 100% {
			box-shadow: 0 0 6px color-mix(in srgb, var(--color-error) 60%, transparent);
		}
		50% {
			box-shadow: 0 0 12px color-mix(in srgb, var(--color-error) 80%, transparent);
		}
	}

	@keyframes pulse-text {
		0%, 100% {
			opacity: 1;
		}
		50% {
			opacity: 0.6;
		}
	}

	/* Responsive */
	@media (max-width: 700px) {
		.token-display {
			gap: var(--spacing-sm);
		}

		.separator {
			display: none;
		}

		.context-metric {
			min-width: 100px;
		}

		.metric-label {
			display: none;
		}

		.tokens-metric.cumulative {
			display: none;
		}

		.cost-total {
			display: none;
		}
	}

	@media (max-width: 480px) {
		.context-label {
			display: none;
		}

		.speed-metric {
			padding: var(--spacing-xs);
		}
	}
</style>
