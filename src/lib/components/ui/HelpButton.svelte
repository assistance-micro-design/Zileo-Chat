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
  HelpButton Component
  A circular help button (?) that opens a modal with explanation and tutorial.

  @example
  <HelpButton
    titleKey="help_providers_title"
    descriptionKey="help_providers_description"
    tutorialKey="help_providers_tutorial"
  />
-->
<script lang="ts">
	import { HelpCircle } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';
	import Modal from './Modal.svelte';
	import Button from './Button.svelte';

	/**
	 * HelpButton props
	 */
	interface Props {
		/** i18n key for the modal title */
		titleKey: string;
		/** i18n key for the description text */
		descriptionKey: string;
		/** i18n key for the tutorial steps (newline separated) */
		tutorialKey?: string;
		/** Size of the help icon */
		size?: 'sm' | 'md';
	}

	let { titleKey, descriptionKey, tutorialKey, size = 'sm' }: Props = $props();

	let showModal = $state(false);

	/**
	 * Parse tutorial text into steps (split by newlines)
	 */
	const tutorialSteps = $derived(
		tutorialKey ? $i18n(tutorialKey).split('\n').filter(Boolean) : []
	);

	/**
	 * Icon size based on button size
	 */
	const iconSize = $derived(size === 'sm' ? 14 : 18);
</script>

<button
	type="button"
	class="help-button"
	class:help-button-sm={size === 'sm'}
	class:help-button-md={size === 'md'}
	onclick={() => showModal = true}
	title={$i18n('help_button_tooltip')}
	aria-label={$i18n('help_button_label')}
>
	<HelpCircle size={iconSize} />
</button>

<Modal open={showModal} title={$i18n(titleKey)} onclose={() => showModal = false}>
	{#snippet body()}
		<div class="help-content">
			<p class="help-description">{$i18n(descriptionKey)}</p>

			{#if tutorialSteps.length > 0}
				<div class="help-tutorial">
					<h4 class="tutorial-title">{$i18n('help_how_to_use')}</h4>
					<ol class="tutorial-steps">
						{#each tutorialSteps as step, i}
							<li class="tutorial-step">
								<span class="step-number">{i + 1}</span>
								<span class="step-text">{step}</span>
							</li>
						{/each}
					</ol>
				</div>
			{/if}
		</div>
	{/snippet}
	{#snippet footer()}
		<Button variant="primary" onclick={() => showModal = false}>
			{$i18n('common_understood')}
		</Button>
	{/snippet}
</Modal>

<style>
	.help-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border: none;
		background: var(--color-bg-tertiary);
		color: var(--color-text-secondary);
		border-radius: 50%;
		cursor: pointer;
		transition: all var(--transition-fast);
		flex-shrink: 0;
	}

	.help-button:hover {
		background: var(--color-accent-light);
		color: var(--color-accent);
		transform: scale(1.1);
	}

	.help-button:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 2px;
	}

	.help-button-sm {
		width: 22px;
		height: 22px;
	}

	.help-button-md {
		width: 28px;
		height: 28px;
	}

	/* Modal content styles */
	.help-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.help-description {
		font-size: var(--font-size-base);
		color: var(--color-text-primary);
		line-height: var(--line-height-relaxed);
		margin: 0;
	}

	.help-tutorial {
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
		padding: var(--spacing-lg);
	}

	.tutorial-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		margin: 0 0 var(--spacing-md) 0;
	}

	.tutorial-steps {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.tutorial-step {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
	}

	.step-number {
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: 24px;
		height: 24px;
		background: var(--color-accent);
		color: var(--color-text-on-accent);
		border-radius: 50%;
		font-size: var(--font-size-xs);
		font-weight: var(--font-weight-semibold);
		flex-shrink: 0;
	}

	.step-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
		line-height: var(--line-height-normal);
		padding-top: 2px;
	}
</style>
