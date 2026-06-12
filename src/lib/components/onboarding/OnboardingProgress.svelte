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
	 * Vertical stepper for the onboarding wizard's brand panel.
	 * Lists every step with its done / active / upcoming state (check mark,
	 * glowing gradient dot, plain number) plus a "Step x of y" label.
	 * Purely presentational: navigation stays in the onboarding store.
	 */
	import { Check } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { ONBOARDING_STEPS, TOTAL_STEPS } from '$types/onboarding';

	interface Props {
		currentStep: number;
	}

	let { currentStep }: Props = $props();
</script>

<nav class="onboarding-steps" aria-label={$i18n('onboarding_progress_arialabel')}>
	<ol class="steps-list">
		{#each ONBOARDING_STEPS as step, i (step)}
			<li
				class="step-item"
				class:done={i < currentStep}
				class:active={i === currentStep}
				aria-current={i === currentStep ? 'step' : undefined}
			>
				<span class="step-dot" aria-hidden="true">
					{#if i < currentStep}
						<Check size={12} />
					{:else}
						{i + 1}
					{/if}
				</span>
				{$i18n(`onboarding_step_${step}`)}
			</li>
		{/each}
	</ol>

	<span class="progress-label">
		{$i18n('onboarding_progress')
			.replace('{current}', String(currentStep + 1))
			.replace('{total}', String(TOTAL_STEPS))}
	</span>
</nav>

<style>
	.onboarding-steps {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-height: 0;
	}

	.steps-list {
		display: flex;
		flex-direction: column;
		gap: 2px;
		flex: 1;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.step-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: 0.45rem 0.6rem;
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
		color: var(--color-text-tertiary);
	}

	.step-dot {
		width: 22px;
		height: 22px;
		border-radius: 50%;
		flex-shrink: 0;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border: 1px solid var(--color-border-dark);
		font-size: var(--font-size-2xs);
		font-weight: var(--font-weight-semibold);
		background: var(--surface-1);
	}

	.step-item.done {
		color: var(--color-text-secondary);
	}

	.step-item.done .step-dot {
		background: var(--color-success-light);
		border-color: var(--color-success-border);
		color: var(--color-success);
	}

	.step-item.active {
		color: var(--color-text-primary);
		font-weight: var(--font-weight-semibold);
		background: var(--color-accent-light);
	}

	.step-item.active .step-dot {
		background: var(--gradient-brand);
		border: none;
		color: var(--color-accent-text);
		box-shadow: var(--glow-accent-soft);
	}

	.progress-label {
		font-size: var(--font-size-2xs);
		color: var(--color-text-tertiary);
		padding: var(--spacing-md) 0.6rem 0;
	}
</style>
