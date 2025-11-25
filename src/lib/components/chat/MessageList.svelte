<!--
  MessageList Component
  A scrollable list of chat messages with auto-scroll to bottom.

  @example
  <MessageList messages={messages} />
-->
<script lang="ts">
	import type { Message } from '$types/message';
	import MessageBubble from './MessageBubble.svelte';
	import { tick } from 'svelte';

	/**
	 * MessageList props
	 */
	interface Props {
		/** Array of messages to display */
		messages: Message[];
		/** Whether to auto-scroll to new messages */
		autoScroll?: boolean;
	}

	let { messages, autoScroll = true }: Props = $props();

	let containerRef: HTMLDivElement;

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

<div class="message-list" bind:this={containerRef} role="log" aria-live="polite" aria-label="Chat messages">
	{#if messages.length === 0}
		<div class="message-list-empty">
			<p>No messages yet. Start a conversation!</p>
		</div>
	{:else}
		{#each messages as message (message.id)}
			<MessageBubble {message} />
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
	}

	.message-list-empty {
		display: flex;
		align-items: center;
		justify-content: center;
		flex: 1;
		color: var(--color-text-tertiary);
		font-size: var(--font-size-sm);
	}
</style>
