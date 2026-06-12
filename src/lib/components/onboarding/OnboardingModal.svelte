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

<script lang="ts">
	/**
	 * Full-screen onboarding modal container
	 * Manages step navigation and renders dynamic step components
	 */
	import { fly } from 'svelte/transition';
	import { i18n } from '$lib/i18n';
	import { onboardingStore, currentStep, isLastStep, canGoBack } from '$lib/stores/onboarding';
	import { Button } from '$lib/components/ui';
	import { focusTrap } from '$lib/actions/focusTrap';
	import { motionDuration } from '$lib/utils/motion';
	import OnboardingProgress from './OnboardingProgress.svelte';
	import StepLanguage from './steps/StepLanguage.svelte';
	import StepTheme from './steps/StepTheme.svelte';
	import StepWelcome from './steps/StepWelcome.svelte';
	import StepFeatures from './steps/StepFeatures.svelte';
	import StepApiKey from './steps/StepApiKey.svelte';
	import StepImport from './steps/StepImport.svelte';
	import StepGettingStarted from './steps/StepGettingStarted.svelte';
	import StepComplete from './steps/StepComplete.svelte';

	interface Props {
		onComplete: () => void;
	}

	let { onComplete }: Props = $props();

	const steps = [
		StepLanguage,
		StepTheme,
		StepWelcome,
		StepFeatures,
		StepApiKey,
		StepImport,
		StepGettingStarted,
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

	/**
	 * Escape closes the onboarding modal and marks it complete so the user
	 * is not blocked from the rest of the app. Without this, the dialog
	 * trapped keyboard users with no exit path (WCAG 2.1.2 No Keyboard Trap).
	 */
	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape') {
			event.preventDefault();
			handleComplete();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div
	class="onboarding-modal"
	role="dialog"
	aria-modal="true"
	aria-label={$i18n('onboarding_dialog_arialabel')}
	tabindex="-1"
	{@attach focusTrap}
>
	<div class="onboarding-window">
		<!-- Brand panel: cream surface with turquoise veil + vertical stepper -->
		<aside class="onboarding-side">
			<div class="side-brand">
				<span class="brand-dot" aria-hidden="true"></span>
				<strong>Zileo Chat</strong>
			</div>
			<OnboardingProgress currentStep={$currentStep} />
		</aside>

		<div class="onboarding-main">
			<div class="onboarding-content">
				{#key $currentStep}
					<div class="step-wrapper" in:fly={{ x: 24, duration: motionDuration(280) }}>
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
					<!-- Skip is hidden on the language step: the choice applies
					     immediately, so skipping is redundant with Next. -->
					{#if !$isLastStep && $currentStep !== 0}
						<button class="skip-step" onclick={handleSkip}>
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
		padding: var(--spacing-lg);
	}

	/* Two-panel wizard window: brand panel with vertical stepper on the
	   left, step content + constant footer on the right. */
	.onboarding-window {
		display: grid;
		grid-template-columns: 280px 1fr;
		width: 100%;
		max-width: 980px;
		height: 100%;
		max-height: min(640px, 100%);
		background: var(--surface-1);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-xl);
		box-shadow: var(--shadow-xl);
		overflow: hidden;
	}

	.onboarding-side {
		display: flex;
		flex-direction: column;
		padding: var(--spacing-xl) var(--spacing-lg);
		background:
			linear-gradient(180deg, rgba(148, 239, 238, 0.16), transparent 55%), var(--surface-cream);
		border-right: 1px solid var(--color-border-light);
		overflow-y: auto;
	}

	:global([data-theme='dark']) .onboarding-side {
		background:
			linear-gradient(180deg, rgba(148, 239, 238, 0.08), transparent 55%), var(--surface-2);
	}

	.side-brand {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-xl);
		font-size: var(--font-size-lg);
	}

	.side-brand .brand-dot {
		width: 30px;
		height: 30px;
		border-radius: 9px;
		background: var(--gradient-brand);
		box-shadow: var(--glow-accent-soft);
		flex-shrink: 0;
	}

	.onboarding-main {
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
	}

	.onboarding-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-xl);
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
		padding: var(--spacing-md) var(--spacing-xl);
		border-top: 1px solid var(--color-border-light);
		background: var(--surface-2);
	}

	@media (max-width: 760px) {
		.onboarding-window {
			grid-template-columns: 1fr;
			max-height: 100%;
		}

		.onboarding-side {
			display: none;
		}
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

	/* Named .skip-step (not .skip-link) so the global accessibility
	   skip-to-content rules (absolute, off-screen until focused) can never
	   leak onto this in-flow footer button. */
	.skip-step {
		background: none;
		border: none;
		color: var(--color-text-secondary);
		font-size: var(--font-size-sm);
		cursor: pointer;
		padding: var(--spacing-xs) var(--spacing-sm);
		transition: color 0.2s ease;
	}

	.skip-step:hover {
		color: var(--color-text-primary);
	}

	.skip-step:focus-visible {
		outline: 2px solid var(--color-accent-deep);
		outline-offset: 2px;
		border-radius: var(--border-radius-sm);
	}
</style>
