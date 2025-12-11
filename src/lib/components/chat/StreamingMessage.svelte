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
  StreamingMessage Component
  Displays an in-progress assistant message with streaming content,
  tool executions, and reasoning steps.

  @example
  <StreamingMessage
    content={$streamContent}
    tools={$activeTools}
    reasoning={$reasoningSteps}
  />
-->
<script lang="ts">
	import type { ActiveTool, ActiveReasoningStep } from '$lib/stores/streaming';
	import ToolExecution from './ToolExecution.svelte';
	import ReasoningStep from './ReasoningStep.svelte';
	import { Spinner } from '$lib/components/ui';
	import { Bot } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * StreamingMessage props
	 */
	interface Props {
		/** Accumulated streaming content */
		content: string;
		/** Active tools being executed */
		tools?: ActiveTool[];
		/** Reasoning steps captured */
		reasoning?: ActiveReasoningStep[];
		/** Whether streaming is active */
		isStreaming?: boolean;
		/** Whether to show tools section */
		showTools?: boolean;
		/** Whether to show reasoning section */
		showReasoning?: boolean;
	}

	let {
		content,
		tools = [],
		reasoning = [],
		isStreaming = true,
		showTools = true,
		showReasoning = true
	}: Props = $props();

	/**
	 * Running tools count
	 */
	const runningToolsCount = $derived(tools.filter((t) => t.status === 'running').length);

	/**
	 * Whether there are any tools to show
	 */
	const hasTools = $derived(tools.length > 0);

	/**
	 * Whether there are any reasoning steps to show
	 */
	const hasReasoning = $derived(reasoning.length > 0);

	/**
	 * Whether to show the content section
	 */
	const hasContent = $derived(content.length > 0);
</script>

<div class="streaming-message" role="status" aria-live="polite" aria-busy={isStreaming}>
	<!-- Header with status -->
	<div class="streaming-header">
		<Bot size={16} class="assistant-icon" />
		<span class="assistant-label">{$i18n('chat_assistant')}</span>
		{#if isStreaming}
			<div class="streaming-indicator">
				<Spinner size="sm" />
				<span class="streaming-text">{$i18n('chat_generating')}</span>
			</div>
		{/if}
	</div>

	<!-- Reasoning Steps (collapsible) -->
	{#if showReasoning && hasReasoning}
		<div class="reasoning-section">
			<details>
				<summary class="reasoning-summary">
					{reasoning.length !== 1
						? $i18n('chat_reasoning_steps_plural').replace('{count}', String(reasoning.length))
						: $i18n('chat_reasoning_steps').replace('{count}', String(reasoning.length))}
				</summary>
				<div class="reasoning-list">
					{#each reasoning as step (step.timestamp)}
						<ReasoningStep
							step={step.content}
							stepNumber={step.stepNumber}
							expanded={false}
						/>
					{/each}
				</div>
			</details>
		</div>
	{/if}

	<!-- Tool Executions -->
	{#if showTools && hasTools}
		<div class="tools-section">
			<div class="tools-header">
				<span class="tools-label">
					{runningToolsCount > 0
						? $i18n('chat_tools_running').replace('{count}', String(runningToolsCount))
						: $i18n('chat_tools_executed').replace('{count}', String(tools.length))}
				</span>
			</div>
			<div class="tools-list">
				{#each tools as tool (tool.name + tool.startedAt)}
					<ToolExecution
						tool={tool.name}
						status={tool.status}
						duration={tool.duration}
						error={tool.error}
					/>
				{/each}
			</div>
		</div>
	{/if}

	<!-- Streaming Content -->
	<div class="streaming-content">
		{#if hasContent}
			<div class="content-text">{content}</div>
		{:else if isStreaming}
			<div class="content-placeholder">
				<span class="typing-indicator">
					<span class="dot"></span>
					<span class="dot"></span>
					<span class="dot"></span>
				</span>
			</div>
		{/if}
		{#if isStreaming && hasContent}
			<span class="cursor" aria-hidden="true"></span>
		{/if}
	</div>
</div>

<style>
	.streaming-message {
		align-self: flex-start;
		max-width: 80%;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		overflow: hidden;
	}

	.streaming-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-sm) var(--spacing-md);
		background: var(--color-bg-tertiary);
		border-bottom: 1px solid var(--color-border);
	}

	.streaming-header :global(.assistant-icon) {
		color: var(--color-accent);
	}

	.assistant-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.streaming-indicator {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		margin-left: auto;
	}

	.streaming-text {
		font-size: var(--font-size-xs);
		color: var(--color-accent);
	}

	/* Reasoning Section */
	.reasoning-section {
		border-bottom: 1px solid var(--color-border);
	}

	.reasoning-summary {
		display: flex;
		align-items: center;
		padding: var(--spacing-sm) var(--spacing-md);
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		cursor: pointer;
		user-select: none;
	}

	.reasoning-summary:hover {
		background: var(--color-bg-hover);
	}

	.reasoning-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		padding: 0 var(--spacing-md) var(--spacing-sm);
	}

	/* Tools Section */
	.tools-section {
		border-bottom: 1px solid var(--color-border);
		padding: var(--spacing-sm) var(--spacing-md);
	}

	.tools-header {
		margin-bottom: var(--spacing-sm);
	}

	.tools-label {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.tools-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	/* Content */
	.streaming-content {
		padding: var(--spacing-md);
		font-size: var(--font-size-sm);
		line-height: var(--line-height-relaxed);
		color: var(--color-text-primary);
		white-space: pre-wrap;
		word-break: break-word;
		position: relative;
	}

	.content-text {
		display: inline;
	}

	.content-placeholder {
		display: flex;
		align-items: center;
		min-height: 24px;
	}

	/* Typing indicator */
	.typing-indicator {
		display: flex;
		gap: 4px;
	}

	.dot {
		width: 6px;
		height: 6px;
		background: var(--color-text-tertiary);
		border-radius: 50%;
		animation: bounce 1.4s infinite ease-in-out both;
	}

	.dot:nth-child(1) {
		animation-delay: -0.32s;
	}

	.dot:nth-child(2) {
		animation-delay: -0.16s;
	}

	@keyframes bounce {
		0%,
		80%,
		100% {
			transform: scale(0);
		}
		40% {
			transform: scale(1);
		}
	}

	/* Blinking cursor */
	.cursor {
		display: inline-block;
		width: 2px;
		height: 1em;
		background: var(--color-accent);
		margin-left: 2px;
		vertical-align: text-bottom;
		animation: blink 1s step-end infinite;
	}

	@keyframes blink {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0;
		}
	}
</style>
