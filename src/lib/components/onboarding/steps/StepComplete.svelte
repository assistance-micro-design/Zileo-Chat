<script lang="ts">
	/**
	 * Step 7: Completion
	 * Final step with launch button
	 */
	import { i18n } from '$lib/i18n';
	import { Button } from '$lib/components/ui';

	interface Props {
		onComplete: () => void;
	}

	let { onComplete }: Props = $props();
</script>

<div class="step-complete" data-step="complete">
	<div class="success-icon">
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
			<path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
			<polyline points="22 4 12 14.01 9 11.01"></polyline>
		</svg>
	</div>

	<h1 class="step-title">{$i18n('onboarding_complete_title')}</h1>
	<p class="step-description">{$i18n('onboarding_complete_description')}</p>

	<div class="complete-action">
		<Button variant="primary" size="lg" onclick={onComplete}>
			{$i18n('onboarding_complete_button')}
		</Button>
	</div>
</div>

<style>
	.step-complete {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-lg);
	}

	.success-icon {
		width: 80px;
		height: 80px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--color-success-bg, #d1fae5);
		border-radius: 50%;
		color: var(--color-success, #059669);
	}

	.success-icon svg {
		width: 40px;
		height: 40px;
	}

	.step-title {
		font-size: var(--font-size-2xl);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.step-description {
		font-size: var(--font-size-md);
		color: var(--color-text-secondary);
		margin: 0;
		max-width: 400px;
	}

	.complete-action {
		margin-top: var(--spacing-md);
	}
</style>
