<!--
  ProgressBar Component
  A progress indicator showing completion percentage.

  @example
  <ProgressBar value={50} />
  <ProgressBar value={75} max={100} />
  <ProgressBar value={3} max={10} showLabel />
-->
<script lang="ts">
	/**
	 * ProgressBar props
	 */
	interface Props {
		/** Current progress value */
		value: number;
		/** Maximum value (default 100) */
		max?: number;
		/** Show percentage label */
		showLabel?: boolean;
		/** Accessible label */
		label?: string;
	}

	let { value, max = 100, showLabel = false, label = 'Progress' }: Props = $props();

	/**
	 * Calculate percentage
	 */
	const percentage = $derived(Math.min(100, Math.max(0, (value / max) * 100)));
</script>

<div class="progress-wrapper">
	<div
		class="progress-bar"
		role="progressbar"
		aria-valuenow={value}
		aria-valuemin={0}
		aria-valuemax={max}
		aria-label={label}
	>
		<div class="progress-fill" style="width: {percentage}%;"></div>
	</div>
	{#if showLabel}
		<span class="progress-label">{Math.round(percentage)}%</span>
	{/if}
</div>

<style>
	.progress-wrapper {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.progress-bar {
		flex: 1;
	}

	.progress-label {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		min-width: 40px;
		text-align: right;
	}
</style>
