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
	 * Onboarding step: key features
	 * Presents the main capabilities of Zileo Chat in plain language.
	 */
	import { i18n } from '$lib/i18n';
	import { fly } from 'svelte/transition';
	import { MessageSquare, LayoutDashboard, Brain, Mic } from '@lucide/svelte';
	import { motionDuration, prefersReducedMotion } from '$lib/utils/motion';
	import type { Component } from 'svelte';

	interface Props {
		onNext?: () => void;
	}

	let { onNext: _onNext }: Props = $props();

	interface FeatureCard {
		key: string;
		icon: Component;
		/**
		 * Execution-channel token suffix tinting the card's icon pill.
		 * 'accent' is not a channel but the brand turquoise (voice dictation).
		 */
		channel: 'agent' | 'tool' | 'thinking' | 'accent';
	}

	const features: FeatureCard[] = [
		{ key: 'chat', icon: MessageSquare, channel: 'agent' },
		{ key: 'kanban', icon: LayoutDashboard, channel: 'tool' },
		{ key: 'memory', icon: Brain, channel: 'thinking' },
		{ key: 'voice', icon: Mic, channel: 'accent' }
	];

	/** Staggered per-card entrance delay (ms), disabled under reduced-motion. */
	function cardDelay(index: number): number {
		return prefersReducedMotion() ? 0 : index * 80;
	}

	/**
	 * Inline CSS custom properties tinting a card's icon pill. The brand
	 * 'accent' has no `--channel-*` token, so it maps to the accent pair.
	 */
	function iconTint(channel: FeatureCard['channel']): string {
		if (channel === 'accent') {
			return '--feat-channel: var(--color-accent-deep); --feat-channel-soft: var(--color-accent-light)';
		}
		return `--feat-channel: var(--channel-${channel}); --feat-channel-soft: var(--channel-${channel}-soft)`;
	}
</script>

<div class="step-features" data-step="features">
	<h1 class="step-title">{$i18n('onboarding_features_title')}</h1>
	<p class="step-description">{$i18n('onboarding_features_description')}</p>

	<div class="features-grid">
		{#each features as feature, index (feature.key)}
			{@const Icon = feature.icon}
			<div
				class="feature-card"
				style={iconTint(feature.channel)}
				in:fly={{ y: 16, duration: motionDuration(300), delay: cardDelay(index) }}
			>
				<div class="feature-icon">
					<Icon size={20} aria-hidden="true" />
				</div>
				<div class="feature-body">
					<h3 class="feature-name">{$i18n(`onboarding_features_${feature.key}_title`)}</h3>
					<p class="feature-text">{$i18n(`onboarding_features_${feature.key}_description`)}</p>
				</div>
			</div>
		{/each}
	</div>
</div>

<style>
	.step-features {
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

	.features-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-md);
		width: 100%;
		max-width: 540px;
	}

	.feature-card {
		display: flex;
		flex-direction: row;
		align-items: flex-start;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		background: var(--surface-1);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		box-shadow: var(--shadow-xs);
		text-align: left;
		transition:
			transform 0.2s ease,
			box-shadow 0.2s ease,
			border-color 0.2s ease;
	}

	.feature-body {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.feature-card:hover {
		transform: translateY(-3px) scale(1.02);
		border-color: var(--color-accent-hover);
		box-shadow: var(--shadow-md);
	}

	@media (prefers-reduced-motion: reduce) {
		.feature-card {
			transition: none;
		}

		.feature-card:hover {
			transform: none;
		}
	}

	/* Icon pill tinted by the feature's execution channel */
	.feature-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		width: 36px;
		height: 36px;
		border-radius: 10px;
		background: var(--feat-channel-soft);
		color: var(--feat-channel);
	}

	.feature-name {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.feature-text {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		margin: 0;
		line-height: 1.4;
	}

	@media (max-width: 480px) {
		.features-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
