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
  MessageList Component
  A scrollable list of chat messages with auto-scroll to bottom.
  Uses CSS containment for performance with long message histories.

  @example
  <MessageList messages={messages} />
-->
<script lang="ts">
	import type { Message } from '$types/message';
	import MessageBubble from './MessageBubble.svelte';
	import { tick } from 'svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * MessageList props
	 */
	interface Props {
		/** Array of messages to display */
		messages: Message[];
		/** Whether to auto-scroll to new messages */
		autoScroll?: boolean;
		/** Threshold for enabling performance optimizations (default: 50 messages) */
		performanceThreshold?: number;
	}

	let { messages, autoScroll = true, performanceThreshold = 50 }: Props = $props();

	let containerRef: HTMLDivElement;

	/**
	 * Enable performance mode for long lists
	 */
	const enablePerformanceMode = $derived(messages.length > performanceThreshold);

	/**
	 * Scroll to the bottom of the message list
	 */
	async function scrollToBottom(): Promise<void> {
		if (!autoScroll || !containerRef) return;
		await tick();
		containerRef.scrollTop = containerRef.scrollHeight;
	}

	/**
	 * Watch for new messages and auto-scroll
	 */
	$effect(() => {
		if (messages.length > 0) {
			scrollToBottom();
		}
	});
</script>

<div
	class="message-list"
	class:performance-mode={enablePerformanceMode}
	bind:this={containerRef}
	role="log"
	aria-live="polite"
	aria-label={$i18n('chat_messages_arialabel')}
>
	{#if messages.length === 0}
		<div class="message-list-empty">
			<p>{$i18n('chat_no_messages')}</p>
		</div>
	{:else}
		{#each messages as message (message.id)}
			<div class="message-wrapper" class:optimize={enablePerformanceMode}>
				<MessageBubble {message} />
			</div>
		{/each}
	{/if}
</div>

<style>
	.message-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		flex: 1;
		overflow-y: auto;
		padding: var(--spacing-lg);
		scroll-behavior: smooth;
	}

	/* Performance mode: enable CSS containment for long lists */
	.message-list.performance-mode {
		contain: strict;
		will-change: scroll-position;
	}

	.message-wrapper {
		animation: fadeIn 200ms ease-out;
	}

	/* Performance mode: use content-visibility for off-screen messages */
	.message-wrapper.optimize {
		content-visibility: auto;
		contain-intrinsic-size: 0 100px;
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

	.message-list-empty {
		display: flex;
		align-items: center;
		justify-content: center;
		flex: 1;
		color: var(--color-text-tertiary);
		font-size: var(--font-size-sm);
	}

	/* Respect reduced motion preference */
	@media (prefers-reduced-motion: reduce) {
		.message-list {
			scroll-behavior: auto;
		}

		.message-wrapper {
			animation: none;
		}
	}
</style>
