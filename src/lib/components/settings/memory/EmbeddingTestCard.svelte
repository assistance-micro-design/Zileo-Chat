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
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

EmbeddingTestCard - Test embedding generation with sample text.
Extracted from MemorySettings.svelte.
-->

<script lang="ts">
	import { tauriInvoke } from '$lib/tauri';
	import { Card, Button, Textarea } from '$lib/components/ui';
	import type { EmbeddingTestResult } from '$types/embedding';
	import { i18n, t } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import { toastStore } from '$lib/stores/toast';
	import type { ToastType } from '$types/background-workflow';

	interface Props {
		/** Whether a config exists (required to test) */
		configExists: boolean;
	}

	let { configExists }: Props = $props();

	/** Test embedding state */
	let testText = $state('');
	let testingEmbedding = $state(false);
	let testResult = $state<EmbeddingTestResult | null>(null);

	function notify(type: ToastType, text: string): void {
		toastStore.add({ type, title: text, message: '', persistent: false, duration: 5000 });
	}

	/**
	 * Tests embedding generation with sample text
	 */
	async function handleTestEmbedding(): Promise<void> {
		if (!testText.trim()) {
			notify('error', t('memory_enter_test_text'));
			return;
		}

		testingEmbedding = true;
		testResult = null;

		try {
			testResult = await tauriInvoke<EmbeddingTestResult>('test_embedding', { text: testText });
			if (testResult.success) {
				notify(
					'success',
					t('memory_embedding_generated').replace('{duration}', String(testResult.duration_ms))
				);
			} else {
				notify('error', testResult.error || t('common_error'));
			}
		} catch (err) {
			notify('error', t('memory_test_failed').replace('{error}', getErrorMessage(err)));
		} finally {
			testingEmbedding = false;
		}
	}
</script>

<Card>
	{#snippet header()}
		<h3 class="card-title">{$i18n('memory_test_title')}</h3>
	{/snippet}
	{#snippet body()}
		<div class="test-section">
			<Textarea
				label={$i18n('memory_test_text_label')}
				value={testText}
				placeholder={$i18n('memory_test_text_placeholder')}
				rows={3}
				oninput={(e) => (testText = e.currentTarget.value)}
			/>
			<div class="test-actions">
				<Button
					variant="secondary"
					onclick={handleTestEmbedding}
					disabled={!testText.trim() || testingEmbedding || !configExists}
				>
					{testingEmbedding ? $i18n('memory_testing') : $i18n('memory_test_button')}
				</Button>
				{#if !configExists}
					<span class="test-hint">{$i18n('memory_configure_first')}</span>
				{/if}
			</div>

			{#if testResult}
				<div
					class="test-result"
					class:success={testResult.success}
					class:error={!testResult.success}
				>
					{#if testResult.success}
						<div class="result-row">
							<span class="result-label">{$i18n('memory_dimension')}</span>
							<span class="result-value">{testResult.dimension}</span>
						</div>
						<div class="result-row">
							<span class="result-label">{$i18n('memory_duration')}</span>
							<span class="result-value">{testResult.duration_ms}ms</span>
						</div>
						<div class="result-row">
							<span class="result-label">{$i18n('memory_provider')}</span>
							<span class="result-value">{testResult.provider}</span>
						</div>
						<div class="result-row">
							<span class="result-label">{$i18n('memory_model')}</span>
							<span class="result-value">{testResult.model}</span>
						</div>
						<div class="result-row">
							<span class="result-label">{$i18n('memory_preview')}</span>
							<span class="result-value preview"
								>[{testResult.preview
									.slice(0, 3)
									.map((v) => v.toFixed(4))
									.join(', ')}...]</span
							>
						</div>
					{:else}
						<p class="error-text">{testResult.error}</p>
					{/if}
				</div>
			{/if}
		</div>
	{/snippet}
</Card>

<style>
	.card-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.test-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.test-actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
	}

	.test-hint {
		font-size: var(--font-size-sm);
		color: var(--color-text-tertiary);
		font-style: italic;
	}

	.test-result {
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
	}

	.test-result.success {
		background: var(--color-success-light);
		border: 1px solid var(--color-success);
	}

	.test-result.error {
		background: var(--color-error-light);
		border: 1px solid var(--color-error);
	}

	.result-row {
		display: flex;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-xs);
	}

	.result-label {
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
		min-width: 80px;
	}

	.result-value {
		color: var(--color-text-primary);
	}

	.result-value.preview {
		font-family: var(--font-mono);
		font-size: var(--font-size-xs);
	}

	.error-text {
		color: var(--color-error);
		margin: 0;
	}
</style>
