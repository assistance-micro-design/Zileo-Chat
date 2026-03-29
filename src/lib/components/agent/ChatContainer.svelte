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

ChatContainer Component
Main chat area with message display, execution blocks inline, and input controls.
-->

<script lang="ts">
	import { tick, untrack } from 'svelte';
	import { Bot, ArrowDown } from '@lucide/svelte';
	import { HelpButton } from '$lib/components/ui';
	import MarkdownRenderer from '$lib/components/ui/MarkdownRenderer.svelte';
	import MessageList from '$lib/components/chat/MessageList.svelte';
	import MessageListSkeleton from '$lib/components/chat/MessageListSkeleton.svelte';
	import ChatInput from '$lib/components/chat/ChatInput.svelte';
	import ThinkingBlock from '$lib/components/chat/ThinkingBlock.svelte';
	import ToolCallBlock from '$lib/components/chat/ToolCallBlock.svelte';
	import SubAgentBlock from '$lib/components/chat/SubAgentBlock.svelte';
	import ExecutionSpinner from '$lib/components/chat/ExecutionSpinner.svelte';
	import TodoTasksBlock from '$lib/components/chat/TodoTasksBlock.svelte';
	import { i18n } from '$lib/i18n';
	import type { Message } from '$types/message';
	import type { ChatBlock, ThinkingBlockData, ToolCallBlockData, SubAgentBlockData, TodoTaskDisplay } from '$types/chat-block';

	interface Props {
		messages: Message[];
		messagesLoading: boolean;
		/** Persisted blocks per message (message_id -> blocks) */
		messageBlocks?: Map<string, ChatBlock[]>;
		/** Real-time execution blocks (current execution) */
		executionBlocks?: ChatBlock[];
		/** Whether an execution is currently active */
		isExecuting?: boolean;
		/** Contextual spinner text */
		spinnerContext?: string | null;
		/** Active tasks from TodoTool (displayed after spinner) */
		executionTasks?: TodoTaskDisplay[];
		/** Final response from the current execution */
		executionResponse?: {
			content: string;
			tokensInput: number;
			tokensOutput: number;
		} | null;
		disabled: boolean;
		onsend: (message: string) => void;
		oncancel?: () => void;
	}

	let {
		messages,
		messagesLoading,
		messageBlocks,
		executionBlocks = [],
		isExecuting = false,
		spinnerContext = null,
		executionTasks = [],
		executionResponse = null,
		disabled,
		onsend,
		oncancel
	}: Props = $props();

	let messagesContainer: HTMLDivElement | null = $state(null);

	// Smart scroll state
	let userHasScrolledUp = $state(false);
	let wasLoading = $state(false);
	let scrollRafPending = false;
	const SCROLL_BOTTOM_THRESHOLD = 80;

	// Content signal: uses addition to force Svelte 5 to track ALL dependencies
	// (avoids short-circuit with || where only the first truthy stops evaluation)
	let contentSignal = $derived(
		messages.length + executionBlocks.length + executionTasks.length + (executionResponse ? 1 : 0)
	);

	function isNearBottom(container: HTMLElement): boolean {
		const { scrollTop, scrollHeight, clientHeight } = container;
		return scrollHeight - scrollTop - clientHeight <= SCROLL_BOTTOM_THRESHOLD;
	}

	function scrollToBottom(behavior: 'auto' | 'instant' | 'smooth' = 'smooth'): void {
		if (messagesContainer) {
			messagesContainer.scrollTo({ top: messagesContainer.scrollHeight, behavior });
		}
	}

	function handleScroll(): void {
		if (scrollRafPending) return;
		scrollRafPending = true;
		requestAnimationFrame(() => {
			scrollRafPending = false;
			if (!messagesContainer) return;
			userHasScrolledUp = !isNearBottom(messagesContainer);
		});
	}

	function handleScrollToBottomClick(): void {
		scrollToBottom();
		userHasScrolledUp = false;
	}

	// Auto-scroll when content changes (unless user scrolled up)
	$effect(() => {
		const _signal = contentSignal;
		if (messagesContainer && _signal > 0 && !userHasScrolledUp) {
			tick().then(() => {
				if (messagesContainer && !userHasScrolledUp) {
					scrollToBottom();
				}
			});
		}
	});

	// Scroll to bottom after skeleton -> content transition
	$effect(() => {
		const currentlyLoading = messagesLoading;
		const previouslyLoading = untrack(() => wasLoading);
		if (previouslyLoading && !currentlyLoading && messagesContainer) {
			tick().then(() => {
				if (messagesContainer) {
					scrollToBottom('instant');
					userHasScrolledUp = false;
				}
			});
		}
		wasLoading = currentlyLoading;
	});

	/**
	 * Get persisted blocks for a specific message.
	 */
	function getBlocksForMessage(messageId: string): ChatBlock[] {
		return messageBlocks?.get(messageId) ?? [];
	}
</script>

<div class="chat-container">
	<!-- Help Button -->
	<div class="chat-help">
		<HelpButton
			titleKey="help_chat_title"
			descriptionKey="help_chat_description"
			tutorialKey="help_chat_tutorial"
		/>
	</div>

	<!-- Messages Area -->
	<div class="messages-area" bind:this={messagesContainer} onscroll={handleScroll}>
		{#if messagesLoading}
			<MessageListSkeleton count={3} />
		{:else}
			{#snippet renderBlock(block: ChatBlock, _index: number)}
				{#if block.block_type === 'thinking'}
					{@const data = block.data as ThinkingBlockData}
					<ThinkingBlock
						content={data.content}
						source={data.source}
					/>
				{:else if block.block_type === 'tool_call'}
					{@const data = block.data as ToolCallBlockData}
					<ToolCallBlock
						toolName={data.tool_name}
						toolType={data.tool_type}
						serverName={data.server_name}
						inputParams={data.input_params}
						outputResult={data.output_result}
						success={data.success}
						errorMessage={data.error_message}
						durationMs={data.duration_ms}
					/>
				{:else if block.block_type === 'sub_agent'}
					{@const data = block.data as SubAgentBlockData}
					<SubAgentBlock
						agentName={data.agent_name}
						status={data.status}
						durationMs={data.duration_ms}
						tokensInput={data.tokens_input}
						tokensOutput={data.tokens_output}
						reportSummary={data.report_summary}
					/>
				{/if}
			{/snippet}

			<!-- Message List with Persisted Blocks -->
			<div class="message-list-with-blocks">
				{#each messages as message (message.id)}
					<div class="message-wrapper">
						<div class="message-item">
							<MessageList messages={[message]} />
						</div>

						<!-- Persisted blocks for assistant messages (reactive - no {@const}) -->
						{#if message.role === 'assistant' && getBlocksForMessage(message.id).length > 0}
							<div class="persisted-blocks">
								{#each getBlocksForMessage(message.id) as block, i (`${block.block_type}-${i}`)}
									{@render renderBlock(block, i)}
								{/each}
							</div>
						{/if}
					</div>
				{/each}
			</div>

			<!-- Real-time execution blocks (current execution) -->
			{#if isExecuting || executionBlocks.length > 0}
				<div class="execution-blocks">
					{#each executionBlocks as block, i (`${block.block_type}-${i}`)}
						{@render renderBlock(block, i)}
					{/each}

					{#if isExecuting}
						<ExecutionSpinner context={spinnerContext} active={true} />
					{/if}
				</div>
			{/if}

			<!-- Tasks block (independent of execution-blocks, persists after execution) -->
			{#if executionTasks.length > 0}
				<div class="tasks-section">
					<TodoTasksBlock tasks={executionTasks} />
				</div>
			{/if}

			<!-- Final response (pending persistence) -->
			{#if executionResponse}
				<div class="execution-response">
					<div class="response-bubble">
						<div class="response-header">
							<Bot size={16} class="bot-icon" />
							<span>{$i18n('chat_assistant')}</span>
						</div>
						<div class="response-content">
							<MarkdownRenderer content={executionResponse.content} />
						</div>
					</div>
				</div>
			{/if}
		{/if}
	</div>

	<!-- Scroll to bottom button -->
	{#if userHasScrolledUp}
		<button
			class="scroll-to-bottom"
			onclick={handleScrollToBottomClick}
			aria-label={$i18n('chat_scroll_to_bottom')}
			title={$i18n('chat_scroll_to_bottom')}
		>
			<ArrowDown size={18} />
		</button>
	{/if}

	<!-- Chat Input with Cancel Button -->
	<div class="input-area">
		<ChatInput
			disabled={isExecuting || disabled}
			loading={isExecuting}
			onsend={isExecuting ? undefined : onsend}
			oncancel={isExecuting ? oncancel : undefined}
		/>
	</div>
</div>

<style>
	.chat-container {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-height: 0;
		overflow: hidden;
		position: relative;
	}

	.chat-help {
		position: absolute;
		top: var(--spacing-sm);
		right: var(--spacing-sm);
		z-index: 10;
	}

	.messages-area {
		flex: 1;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		min-height: 0;
	}

	.message-list-with-blocks {
		display: flex;
		flex-direction: column;
		padding: var(--spacing-lg);
		gap: var(--spacing-sm);
	}

	.message-wrapper {
		animation: fadeIn 200ms ease-out;
	}

	.message-item :global(.message-list) {
		padding: 0;
		gap: 0;
	}

	.persisted-blocks {
		padding: 0 var(--spacing-md);
	}

	/* Execution Blocks (real-time) */
	.execution-blocks {
		padding: var(--spacing-sm) var(--spacing-lg);
	}

	/* Tasks section (independent of execution blocks, persists after execution) */
	.tasks-section {
		padding: var(--spacing-sm) var(--spacing-lg);
	}

	/* Execution Response (pending persistence) */
	.execution-response {
		padding: var(--spacing-md) var(--spacing-lg);
	}

	.response-bubble {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-lg);
		padding: var(--spacing-md);
		max-width: 80%;
		animation: fadeIn 0.3s ease-in;
	}

	.response-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-sm);
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
	}

	.response-header :global(.bot-icon) {
		color: var(--color-accent);
	}

	.response-content {
		font-size: var(--font-size-md);
		line-height: 1.6;
		color: var(--color-text-primary);
		word-break: break-word;
	}

	@keyframes fadeIn {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.scroll-to-bottom {
		position: absolute;
		bottom: 80px;
		right: var(--spacing-lg);
		width: 36px;
		height: 36px;
		border-radius: 50%;
		border: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
		color: var(--color-text-secondary);
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		z-index: 5;
		animation: fadeIn 200ms ease-out;
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
	}

	.scroll-to-bottom:hover {
		background: var(--color-accent);
		color: var(--color-text-on-accent, #fff);
		border-color: var(--color-accent);
	}

	.input-area {
		padding: 0 var(--spacing-md) var(--spacing-md);
	}

	/* Respect reduced motion preference */
	@media (prefers-reduced-motion: reduce) {
		.message-wrapper,
		.response-bubble,
		.scroll-to-bottom {
			animation: none;
		}
	}
</style>
