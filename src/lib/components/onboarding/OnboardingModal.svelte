<script lang="ts">
	/**
	 * Full-screen onboarding modal container
	 * Manages step navigation and renders dynamic step components
	 */
	import { fade } from 'svelte/transition';
	import { i18n } from '$lib/i18n';
	import { onboardingStore, currentStep, isLastStep, canGoBack } from '$lib/stores/onboarding';
	import { Button } from '$lib/components/ui';
	import OnboardingProgress from './OnboardingProgress.svelte';
	import StepLanguage from './steps/StepLanguage.svelte';
	import StepTheme from './steps/StepTheme.svelte';
	import StepWelcome from './steps/StepWelcome.svelte';
	import StepValues from './steps/StepValues.svelte';
	import StepApiKey from './steps/StepApiKey.svelte';
	import StepImport from './steps/StepImport.svelte';
	import StepComplete from './steps/StepComplete.svelte';

	interface Props {
		onComplete: () => void;
	}

	let { onComplete }: Props = $props();

	const steps = [
		StepLanguage,
		StepTheme,
		StepWelcome,
		StepValues,
		StepApiKey,
		StepImport,
		StepComplete
	];

	const CurrentStep = $derived(steps[$currentStep]);

	function handleNext(): void {
		if ($isLastStep) {
			onboardingStore.markComplete();
			onComplete();
		} else {
			onboardingStore.nextStep();
		}
	}

	function handlePrev(): void {
		onboardingStore.prevStep();
	}

	function handleSkip(): void {
		onboardingStore.skipToEnd();
	}

	function handleComplete(): void {
		onboardingStore.markComplete();
		onComplete();
	}
</script>

<div class="onboarding-modal" role="dialog" aria-modal="true" aria-labelledby="onboarding-title">
	<div class="onboarding-container">
		<OnboardingProgress currentStep={$currentStep} />

		<div class="onboarding-content">
			{#key $currentStep}
				<div class="step-wrapper" in:fade={{ duration: 200 }}>
					<CurrentStep onNext={handleNext} onComplete={handleComplete} />
				</div>
			{/key}
		</div>

		<footer class="onboarding-footer">
			<div class="footer-left">
				{#if $canGoBack}
					<Button variant="ghost" onclick={handlePrev}>
						{$i18n('onboarding_previous')}
					</Button>
				{/if}
			</div>

			<div class="footer-center">
				{#if !$isLastStep}
					<button class="skip-link" onclick={handleSkip}>
						{$i18n('onboarding_skip')}
					</button>
				{/if}
			</div>

			<div class="footer-right">
				{#if !$isLastStep}
					<Button variant="primary" onclick={handleNext}>
						{$i18n('onboarding_next')}
					</Button>
				{/if}
			</div>
		</footer>
	</div>
</div>


<style>
	.onboarding-modal {
		position: fixed;
		inset: 0;
		z-index: var(--z-index-modal, 1000);
		background: var(--color-bg-primary);
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.onboarding-container {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		max-width: 800px;
		max-height: 100vh;
		padding: var(--spacing-xl);
	}

	.onboarding-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-xl) 0;
		overflow-y: auto;
	}

	.step-wrapper {
		width: 100%;
		max-width: 600px;
		text-align: center;
	}

	.onboarding-footer {
		display: grid;
		grid-template-columns: 1fr auto 1fr;
		align-items: center;
		gap: var(--spacing-md);
		padding-top: var(--spacing-lg);
		border-top: 1px solid var(--color-border);
	}

	.footer-left {
		justify-self: start;
	}

	.footer-center {
		justify-self: center;
	}

	.footer-right {
		justify-self: end;
	}

	.skip-link {
		background: none;
		border: none;
		color: var(--color-text-secondary);
		font-size: var(--font-size-sm);
		cursor: pointer;
		padding: var(--spacing-xs) var(--spacing-sm);
		transition: color 0.2s ease;
	}

	.skip-link:hover {
		color: var(--color-text-primary);
	}

	.skip-link:focus-visible {
		outline: 2px solid var(--color-primary);
		outline-offset: 2px;
		border-radius: var(--radius-sm);
	}
</style>
