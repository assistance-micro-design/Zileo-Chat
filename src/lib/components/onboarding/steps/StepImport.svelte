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
	 * Step 6: Import configuration
	 * Link to external configuration resources
	 */
	import { i18n } from '$lib/i18n';
	import { Button } from '$lib/components/ui';

	interface Props {
		onNext: () => void;
	}

	let { onNext }: Props = $props();

	const EXTERNAL_URL = 'https://assistancemicrodesign.net/';

	function openExternalLink(): void {
		window.open(EXTERNAL_URL, '_blank', 'noopener,noreferrer');
	}

	function handleSkip(): void {
		onNext();
	}
</script>

<div class="step-import" data-step="import">
	<h1 class="step-title">{$i18n('onboarding_import_title')}</h1>
	<p class="step-description">{$i18n('onboarding_import_description')}</p>

	<div class="import-options">
		<Button variant="primary" onclick={openExternalLink}>
			<svg
				class="external-icon"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
			>
				<path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
				<polyline points="15 3 21 3 21 9"></polyline>
				<line x1="10" y1="14" x2="21" y2="3"></line>
			</svg>
			{$i18n('onboarding_import_browse')}
		</Button>

		<Button variant="ghost" onclick={handleSkip}>
			{$i18n('onboarding_import_skip')}
		</Button>
	</div>
</div>

<style>
	.step-import {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-lg);
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

	.import-options {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-md);
		margin-top: var(--spacing-md);
	}

	.external-icon {
		width: 16px;
		height: 16px;
		margin-right: var(--spacing-xs);
	}
</style>
