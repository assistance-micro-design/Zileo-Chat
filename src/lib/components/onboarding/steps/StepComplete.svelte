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
	 * Step 7: Completion
	 * Final step with launch button
	 */
	import { i18n } from '$lib/i18n';
	import { scale } from 'svelte/transition';
	import { backOut } from 'svelte/easing';
	import { Button } from '$lib/components/ui';
	import { motionDuration } from '$lib/utils/motion';

	interface Props {
		onComplete: () => void;
	}

	let { onComplete }: Props = $props();
</script>

<div class="step-complete" data-step="complete">
	<div
		class="success-icon"
		in:scale={{ duration: motionDuration(350), start: 0.5, easing: backOut }}
	>
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
		width: 76px;
		height: 76px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--color-success-light);
		border-radius: 50%;
		color: var(--color-success);
		box-shadow: 0 0 24px rgba(74, 222, 128, 0.4);
	}

	.success-icon svg {
		width: 36px;
		height: 36px;
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
		max-width: 400px;
	}

	.complete-action {
		margin-top: var(--spacing-md);
	}
</style>
