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
	 * Step 4: Application values
	 * Presents the core values of Zileo Chat
	 */
	import { i18n } from '$lib/i18n';

	interface Props {
		onNext?: () => void;
	}

	let { onNext: _onNext }: Props = $props();

	const values = [
		{ key: 'opensource', icon: 'code' },
		{ key: 'local', icon: 'hard-drive' },
		{ key: 'api', icon: 'shield' },
		{ key: 'nonsaas', icon: 'server' }
	] as const;
</script>

<div class="step-values" data-step="values">
	<h1 class="step-title">{$i18n('onboarding_values_title')}</h1>

	<div class="values-grid">
		{#each values as value}
			<div class="value-card">
				<div class="value-icon" data-icon={value.icon}>
					{#if value.icon === 'code'}
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<polyline points="16 18 22 12 16 6"></polyline>
							<polyline points="8 6 2 12 8 18"></polyline>
						</svg>
					{:else if value.icon === 'hard-drive'}
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<line x1="22" y1="12" x2="2" y2="12"></line>
							<path d="M5.45 5.11L2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"></path>
							<line x1="6" y1="16" x2="6.01" y2="16"></line>
							<line x1="10" y1="16" x2="10.01" y2="16"></line>
						</svg>
					{:else if value.icon === 'shield'}
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"></path>
							<polyline points="9 12 11 14 15 10"></polyline>
						</svg>
					{:else if value.icon === 'server'}
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<rect x="2" y="2" width="20" height="8" rx="2" ry="2"></rect>
							<rect x="2" y="14" width="20" height="8" rx="2" ry="2"></rect>
							<line x1="6" y1="6" x2="6.01" y2="6"></line>
							<line x1="6" y1="18" x2="6.01" y2="18"></line>
						</svg>
					{/if}
				</div>
				<h3 class="value-title">{$i18n(`onboarding_values_${value.key}_title`)}</h3>
				<p class="value-description">{$i18n(`onboarding_values_${value.key}_description`)}</p>
			</div>
		{/each}
	</div>
</div>

<style>
	.step-values {
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

	.values-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-md);
		width: 100%;
		max-width: 500px;
	}

	.value-card {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-xs);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-lg);
		text-align: center;
	}

	.value-icon {
		width: 40px;
		height: 40px;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--color-primary);
	}

	.value-icon svg {
		width: 24px;
		height: 24px;
	}

	.value-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.value-description {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		margin: 0;
		line-height: 1.4;
	}

	@media (max-width: 480px) {
		.values-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
