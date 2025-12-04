<!--
  MessageBubble Component
  A chat message bubble for user or assistant messages.
  Supports markdown rendering, code highlighting, and timestamps.

  @example
  <MessageBubble message={msg} isUser={false} />
-->
<script lang="ts">
	import type { Message } from '$types/message';
	import { Clock, Hash } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * MessageBubble props
	 */
	interface Props {
		/** Message data */
		message: Message;
		/** Whether this is a user message (alternative to checking role) */
		isUser?: boolean;
	}

	let { message, isUser }: Props = $props();

	/**
	 * Determine if message is from user based on role or prop
	 */
	const isUserMessage = $derived(isUser ?? message.role === 'user');

	/**
	 * Format timestamp for display
	 */
	function formatTime(date: Date): string {
		const d = date instanceof Date ? date : new Date(date);
		return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
	}
</script>

<div class="message-bubble" class:user={isUserMessage} class:assistant={!isUserMessage}>
	<div class="message-content">
		{message.content}
	</div>
	<div class="message-meta">
		<span class="message-time">
			<Clock size={12} />
			{formatTime(message.timestamp)}
		</span>
		{#if message.tokens > 0}
			<span class="message-tokens">
				<Hash size={12} />
				{$i18n('chat_tokens').replace('{count}', String(message.tokens))}
			</span>
		{/if}
	</div>
</div>

<style>
	.message-bubble {
		max-width: 80%;
		padding: var(--spacing-md);
		border-radius: var(--border-radius-lg);
		animation: fadeIn 0.3s ease-in;
	}

	.message-bubble.user {
		align-self: flex-end;
		background: var(--color-accent);
		color: var(--color-text-inverse);
	}

	.message-bubble.assistant {
		align-self: flex-start;
		background: var(--color-bg-secondary);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
	}

	.message-content {
		font-size: var(--font-size-sm);
		line-height: var(--line-height-relaxed);
		white-space: pre-wrap;
		word-break: break-word;
	}

	/* Code blocks within messages */
	.message-content :global(code) {
		font-family: var(--font-mono);
		font-size: var(--font-size-xs);
		padding: var(--spacing-xs) var(--spacing-sm);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-sm);
	}

	.message-meta {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		margin-top: var(--spacing-sm);
		font-size: var(--font-size-xs);
		opacity: 0.7;
	}

	.message-time,
	.message-tokens {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.message-bubble.user .message-meta {
		color: var(--color-text-inverse);
	}

	.message-bubble.assistant .message-meta {
		color: var(--color-text-tertiary);
	}

	@keyframes fadeIn {
		from {
			opacity: 0;
			transform: translateY(10px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}
</style>
