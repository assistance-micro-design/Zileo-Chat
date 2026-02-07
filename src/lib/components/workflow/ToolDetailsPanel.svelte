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
  ToolDetailsPanel Component
  Lazy-loads tool execution details (input/output) via IPC and displays them
  using the JsonViewer component.

  @example
  <ToolDetailsPanel executionId={activity.metadata.executionId} />
-->
<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { getErrorMessage } from '$lib/utils/error';
	import type { ToolExecution } from '$types/tool';
	import JsonViewer from '$lib/components/ui/JsonViewer.svelte';
	import { Loader2, AlertCircle } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { onMount } from 'svelte';

	interface Props {
		/** Tool execution ID for lazy-loading */
		executionId: string;
	}

	let { executionId }: Props = $props();

	let loading = $state(true);
	let error = $state<string | null>(null);
	let execution = $state<ToolExecution | null>(null);

	onMount(async () => {
		try {
			execution = await invoke<ToolExecution>('get_tool_execution', { executionId });
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			loading = false;
		}
	});
</script>

<div class="tool-details" role="region" aria-label="Tool execution details">
	{#if loading}
		<div class="tool-details-loading">
			<Loader2 size={14} class="spinning" />
			<span>{$i18n('workflow_activity_processing')}</span>
		</div>
	{:else if error}
		<div class="tool-details-error">
			<AlertCircle size={14} />
			<span>{error}</span>
		</div>
	{:else if execution}
		<div class="tool-details-section">
			<span class="tool-details-label">Input</span>
			<div class="tool-details-content">
				<JsonViewer data={execution.input_params} maxDepth={3} collapsed={true} />
			</div>
		</div>
		{#if execution.output_result}
			<div class="tool-details-section">
				<span class="tool-details-label">Output</span>
				<div class="tool-details-content">
					<JsonViewer data={execution.output_result} maxDepth={3} collapsed={true} />
				</div>
			</div>
		{/if}
	{/if}
</div>

<style>
	.tool-details {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		margin-top: var(--spacing-sm);
		padding: var(--spacing-sm);
		background: var(--color-bg-tertiary);
		border-radius: var(--radius-sm);
		border-left: 2px solid var(--color-accent);
		animation: slideDown 150ms ease-out;
	}

	@keyframes slideDown {
		from {
			opacity: 0;
			transform: translateY(-4px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.tool-details-loading {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		color: var(--color-text-tertiary);
		font-size: var(--font-size-xs);
	}

	.tool-details-loading :global(.spinning) {
		animation: spin 1s linear infinite;
		color: var(--color-accent);
	}

	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}

	.tool-details-error {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		color: var(--color-error);
		font-size: var(--font-size-xs);
	}

	.tool-details-section {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.tool-details-label {
		font-size: var(--font-size-xs);
		font-weight: 600;
		color: var(--color-text-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.tool-details-content {
		max-height: 250px;
		overflow-y: auto;
	}

	.tool-details-content::-webkit-scrollbar {
		width: 4px;
	}

	.tool-details-content::-webkit-scrollbar-track {
		background: transparent;
	}

	.tool-details-content::-webkit-scrollbar-thumb {
		background: var(--color-border);
		border-radius: var(--radius-full);
	}
</style>
