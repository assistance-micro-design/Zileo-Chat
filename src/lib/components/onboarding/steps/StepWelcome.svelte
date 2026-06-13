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
	 * Onboarding step: welcome message
	 * Warm introduction to Zileo Chat.
	 */
	import { i18n } from '$lib/i18n';

	interface Props {
		onNext?: () => void;
	}

	let { onNext: _onNext }: Props = $props();
</script>

<div class="step-welcome" data-step="welcome">
	<span class="hero-dot" aria-hidden="true"></span>

	<h1 class="step-title">{$i18n('onboarding_welcome_title')}</h1>
	<p class="step-description">{$i18n('onboarding_welcome_description')}</p>
	<p class="step-subtext">{$i18n('onboarding_welcome_subtext')}</p>
</div>

<style>
	.step-welcome {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-lg);
	}

	.hero-dot {
		width: 76px;
		height: 76px;
		border-radius: 22px;
		background: var(--gradient-brand);
		box-shadow: var(--glow-accent);
		margin-bottom: var(--spacing-md);
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
		max-width: 460px;
	}

	.step-subtext {
		font-size: var(--font-size-sm);
		color: var(--color-text-tertiary);
		margin: 0;
		max-width: 460px;
	}
</style>
