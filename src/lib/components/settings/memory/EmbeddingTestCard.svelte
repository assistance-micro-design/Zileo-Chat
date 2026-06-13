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
Results render in a monospace code panel; the action sits in the footer.
-->

<script lang="ts">
	import { tauriInvoke } from '$lib/tauri';
	import { Card, Button, Textarea } from '$lib/components/ui';
	import type { EmbeddingTestResult } from '$types/embedding';
	import { Zap } from '@lucide/svelte';
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

	/** Preformatted result lines for the code panel */
	const resultLines = $derived.by(() => {
		if (!testResult || !testResult.success) return '';
		const preview = testResult.preview
			.slice(0, 3)
			.map((v) => v.toFixed(4))
			.join(', ');
		return [
			`${t('memory_dimension')} ${testResult.dimension}`,
			`${t('memory_duration')} ${testResult.duration_ms} ms`,
			`${t('memory_preview')} [${preview}, …]`
		].join('\n');
	});

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

<Card title={$i18n('memory_test_title')} description={$i18n('memory_test_subtitle')}>
	{#snippet body()}
		<div class="test-section">
			<Textarea
				label={$i18n('memory_test_text_label')}
				value={testText}
				placeholder={$i18n('memory_test_text_placeholder')}
				rows={2}
				oninput={(e) => (testText = e.currentTarget.value)}
			/>

			{#if testResult}
				{#if testResult.success}
					<pre class="code-panel">{resultLines}</pre>
				{:else}
					<pre class="code-panel error">{testResult.error}</pre>
				{/if}
			{/if}
		</div>
	{/snippet}
	{#snippet footer()}
		<Button
			variant="primary"
			size="sm"
			onclick={handleTestEmbedding}
			disabled={!testText.trim() || testingEmbedding || !configExists}
		>
			<Zap size={14} aria-hidden="true" />
			<span>{testingEmbedding ? $i18n('memory_testing') : $i18n('memory_test_button')}</span>
		</Button>
	{/snippet}
</Card>

<style>
	.test-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.code-panel {
		margin: 0;
		padding: 0.7rem 0.85rem;
		background: var(--surface-2);
		border: 1px solid var(--color-border-light);
		border-radius: var(--border-radius-md);
		font-family: var(--font-mono);
		font-size: var(--font-size-xs);
		line-height: 1.6;
		overflow-x: auto;
		white-space: pre;
	}

	.code-panel.error {
		color: var(--color-error);
		border-color: var(--color-error);
		white-space: pre-wrap;
	}
</style>
