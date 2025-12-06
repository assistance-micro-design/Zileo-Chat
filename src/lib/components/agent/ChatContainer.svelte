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

ChatContainer Component - Phase C Component Extraction
Main chat area with message display, streaming content, and input controls.
-->

<script lang="ts">
	import { StopCircle, Bot } from 'lucide-svelte';
	import { Button, Spinner, HelpButton } from '$lib/components/ui';
	import MessageList from '$lib/components/chat/MessageList.svelte';
	import MessageListSkeleton from '$lib/components/chat/MessageListSkeleton.svelte';
	import ChatInput from '$lib/components/chat/ChatInput.svelte';
	import { i18n } from '$lib/i18n';
	import type { Message } from '$types/message';

	interface Props {
		messages: Message[];
		messagesLoading: boolean;
		streamContent: string;
		isStreaming: boolean;
		disabled: boolean;
		onsend: (message: string) => void;
		oncancel?: () => void;
	}

	let {
		messages,
		messagesLoading,
		streamContent,
		isStreaming,
		disabled,
		onsend,
		oncancel
	}: Props = $props();

	let messagesContainer: HTMLDivElement | null = $state(null);

	// Auto-scroll to bottom when new messages or streaming content arrives
	$effect(() => {
		if (messagesContainer && (messages.length > 0 || streamContent)) {
			messagesContainer.scrollTop = messagesContainer.scrollHeight;
		}
	});
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
	<div class="messages-area" bind:this={messagesContainer}>
		{#if messagesLoading}
			<MessageListSkeleton count={3} />
		{:else}
			<MessageList {messages} />
		{/if}

		<!-- Streaming Text (shown during generation) -->
		{#if isStreaming && streamContent}
			<div class="streaming-text-container">
				<div class="streaming-text-bubble">
					<div class="streaming-header">
						<Bot size={16} class="bot-icon" />
						<span>{$i18n('chat_assistant')}</span>
						<Spinner size="sm" />
					</div>
					<div class="streaming-content">
						{streamContent}
						<span class="cursor"></span>
					</div>
				</div>
			</div>
		{/if}
	</div>

	<!-- Chat Input with Cancel Button -->
	<div class="input-area">
		{#if isStreaming}
			<div class="chat-input-wrapper">
				<ChatInput disabled={true} loading={true} onsend={() => {}} />
				<Button variant="danger" size="sm" onclick={oncancel} ariaLabel={$i18n('chat_cancel_arialabel')}>
					<StopCircle size={16} />
					{$i18n('chat_cancel')}
				</Button>
			</div>
		{:else}
			<ChatInput {disabled} loading={isStreaming} {onsend} />
		{/if}
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

	/* Streaming Text Display */
	.streaming-text-container {
		padding: var(--spacing-md) var(--spacing-lg);
	}

	.streaming-text-bubble {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-lg);
		padding: var(--spacing-md);
		max-width: 80%;
		animation: fadeIn 0.3s ease-in;
	}

	.streaming-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-sm);
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
	}

	.streaming-header :global(.bot-icon) {
		color: var(--color-accent);
	}

	.streaming-content {
		font-size: var(--font-size-md);
		line-height: 1.6;
		color: var(--color-text-primary);
		white-space: pre-wrap;
		word-break: break-word;
	}

	.streaming-content .cursor {
		display: inline-block;
		width: 2px;
		height: 1.2em;
		background: var(--color-accent);
		margin-left: 2px;
		vertical-align: text-bottom;
		animation: blink 1s step-end infinite;
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

	@keyframes blink {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0;
		}
	}

	.input-area {
		padding: 0 var(--spacing-md) var(--spacing-md);
	}

	/* Chat Input Wrapper (with cancel button) */
	.chat-input-wrapper {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
	}

	.chat-input-wrapper :global(.chat-input-container) {
		flex: 1;
	}
</style>
