<script lang="ts">
	/**
	 * Progress indicator for onboarding wizard
	 * Shows current step and visual progress bar
	 */
	import { i18n } from '$lib/i18n';
	import { TOTAL_STEPS } from '$types/onboarding';

	interface Props {
		currentStep: number;
	}

	let { currentStep }: Props = $props();

	const progressPercent = $derived(((currentStep + 1) / TOTAL_STEPS) * 100);
</script>

<div class="onboarding-progress">
	<div class="progress-text">
		{$i18n('onboarding_progress').replace('{current}', String(currentStep + 1)).replace('{total}', String(TOTAL_STEPS))}
	</div>
	<div class="progress-bar">
		<div class="progress-fill" style="width: {progressPercent}%"></div>
	</div>
	<div class="progress-dots">
		{#each Array(TOTAL_STEPS) as _, i}
			<div
				class="dot"
				class:active={i <= currentStep}
				class:current={i === currentStep}
			></div>
		{/each}
	</div>
</div>

<style>
	.onboarding-progress {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-md) 0;
	}

	.progress-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.progress-bar {
		width: 100%;
		max-width: 300px;
		height: 4px;
		background: var(--color-border);
		border-radius: var(--radius-full);
		overflow: hidden;
	}

	.progress-fill {
		height: 100%;
		background: var(--color-primary);
		border-radius: var(--radius-full);
		transition: width 0.3s ease;
	}

	.progress-dots {
		display: flex;
		gap: var(--spacing-sm);
		margin-top: var(--spacing-xs);
	}

	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-border);
		transition: all 0.3s ease;
	}

	.dot.active {
		background: var(--color-primary);
	}

	.dot.current {
		transform: scale(1.25);
		box-shadow: 0 0 0 2px var(--color-bg-primary), 0 0 0 4px var(--color-primary);
	}
</style>
