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
	 * Onboarding step: getting started checklist.
	 *
	 * Each item is actionable: a "Open Settings" button closes onboarding and
	 * deep-links to the matching Settings route, so the user lands exactly where
	 * the action is performed. A "Finish" button is also offered for users who
	 * prefer to explore on their own.
	 */
	import { i18n } from '$lib/i18n';
	import { goto } from '$app/navigation';
	import { fly } from 'svelte/transition';
	import { KeyRound, Cpu, Bot, ShieldCheck, ArrowRight } from '@lucide/svelte';
	import { onboardingStore } from '$lib/stores/onboarding';
	import { motionDuration, prefersReducedMotion } from '$lib/utils/motion';
	import type { Component } from 'svelte';

	interface Props {
		onComplete?: () => void;
	}

	let { onComplete }: Props = $props();

	interface ChecklistItem {
		key: string;
		icon: Component;
		/** Settings route to deep-link to when the item is opened. */
		route: string;
	}

	/**
	 * Models are configured inside the Providers page (there is no standalone
	 * Models route), so the model item deep-links to /settings/providers.
	 */
	const items: ChecklistItem[] = [
		{ key: 'provider', icon: KeyRound, route: '/settings/providers' },
		{ key: 'model', icon: Cpu, route: '/settings/providers' },
		{ key: 'agent', icon: Bot, route: '/settings/agents' },
		{ key: 'security', icon: ShieldCheck, route: '/settings/validation' }
	];

	/** Per-card entrance delay (ms), disabled under reduced-motion. */
	function cardDelay(index: number): number {
		return prefersReducedMotion() ? 0 : index * 70;
	}

	/**
	 * Closes onboarding then navigates to the matching Settings route so the
	 * user can immediately perform the action described by the card.
	 */
	async function openSettings(route: string): Promise<void> {
		onboardingStore.markComplete();
		onComplete?.();
		await goto(route);
	}

	/** Closes onboarding without navigating, for users who prefer to explore. */
	function finish(): void {
		onboardingStore.markComplete();
		onComplete?.();
	}
</script>

<div class="step-getting-started" data-step="getting_started">
	<h1 class="step-title">{$i18n('onboarding_getting_started_title')}</h1>
	<p class="step-description">{$i18n('onboarding_getting_started_description')}</p>

	<ol class="checklist">
		{#each items as item, index (item.key)}
			{@const Icon = item.icon}
			<li
				class="checklist-item"
				in:fly={{ y: 12, duration: motionDuration(280), delay: cardDelay(index) }}
			>
				<div class="checklist-number" aria-hidden="true">{index + 1}</div>
				<div class="checklist-body">
					<div class="checklist-heading">
						<Icon size={18} aria-hidden="true" />
						<h3 class="checklist-title">
							{$i18n(`onboarding_getting_started_${item.key}_title`)}
						</h3>
					</div>
					<p class="checklist-text">
						{$i18n(`onboarding_getting_started_${item.key}_description`)}
					</p>
				</div>
				<button type="button" class="checklist-action" onclick={() => openSettings(item.route)}>
					<span>{$i18n('onboarding_getting_started_open_settings')}</span>
					<ArrowRight size={14} aria-hidden="true" />
				</button>
			</li>
		{/each}
	</ol>

	<p class="step-reassurance">{$i18n('onboarding_getting_started_reassurance')}</p>

	<button type="button" class="finish-link" onclick={finish}>
		{$i18n('onboarding_getting_started_finish')}
	</button>
</div>

<style>
	.step-getting-started {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-lg);
		width: 100%;
	}

	.step-title {
		font-size: var(--font-size-2xl);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.step-description {
		font-size: var(--font-size-base);
		color: var(--color-text-secondary);
		margin: 0;
		max-width: 480px;
	}

	.checklist {
		list-style: none;
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		width: 100%;
		max-width: 560px;
		margin: 0;
		padding: 0;
		text-align: left;
	}

	.checklist-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		background: var(--surface-1);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		transition:
			transform 0.2s ease,
			box-shadow 0.2s ease,
			border-color 0.2s ease;
	}

	.checklist-item:hover,
	.checklist-item:focus-within {
		transform: translateY(-2px);
		border-color: var(--color-accent-hover);
		box-shadow: var(--glow-accent-soft);
	}

	@media (prefers-reduced-motion: reduce) {
		.checklist-item {
			transition: none;
		}

		.checklist-item:hover,
		.checklist-item:focus-within {
			transform: none;
		}
	}

	.checklist-number {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border-radius: 50%;
		background: var(--gradient-brand);
		color: var(--color-accent-text);
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-bold);
	}

	.checklist-body {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		flex: 1;
		min-width: 0;
	}

	.checklist-heading {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		color: var(--color-accent-deep);
	}

	.checklist-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.checklist-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
		line-height: 1.4;
	}

	.checklist-action {
		flex-shrink: 0;
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-xs);
		padding: var(--spacing-xs) var(--spacing-sm);
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-accent-deep);
		background: transparent;
		border: 1px solid var(--color-accent-deep);
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition:
			background-color 0.2s ease,
			color 0.2s ease;
		white-space: nowrap;
	}

	.checklist-action:hover {
		background: var(--color-accent-deep);
		color: var(--color-accent-text);
	}

	.checklist-action:focus-visible {
		outline: 2px solid var(--color-accent-deep);
		outline-offset: 2px;
	}

	.step-reassurance {
		font-size: var(--font-size-sm);
		color: var(--color-text-tertiary);
		margin: 0;
		max-width: 480px;
	}

	.finish-link {
		background: none;
		border: none;
		color: var(--color-text-secondary);
		font-size: var(--font-size-sm);
		cursor: pointer;
		padding: var(--spacing-xs) var(--spacing-sm);
		transition: color 0.2s ease;
	}

	.finish-link:hover {
		color: var(--color-text-primary);
	}

	.finish-link:focus-visible {
		outline: 2px solid var(--color-accent-deep);
		outline-offset: 2px;
		border-radius: var(--border-radius-sm);
	}

	@media (max-width: 540px) {
		.checklist-item {
			flex-wrap: wrap;
		}

		.checklist-action {
			margin-left: calc(28px + var(--spacing-md));
		}
	}
</style>
