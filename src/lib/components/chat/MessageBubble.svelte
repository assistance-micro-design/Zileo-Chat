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
  MessageBubble Component
  A chat message bubble for user or assistant messages.
  Supports markdown rendering for assistant messages, timestamps, and copy functionality.

  @example
  <MessageBubble message={msg} />
-->
<script lang="ts">
	import type { Message, MessageAttachment } from '$types/message';
	import { Clock, Copy, Check, CircleAlert, X } from '@lucide/svelte';
	import MarkdownRenderer from '$lib/components/ui/MarkdownRenderer.svelte';
	import { i18n } from '$lib/i18n';
	import { copyToClipboard } from '$lib/utils/clipboard';

	/**
	 * MessageBubble props
	 */
	interface Props {
		/** Message data */
		message: Message;
	}

	let { message }: Props = $props();

	/**
	 * Determine if message is from user based on role
	 */
	const isUserMessage = $derived(message.role === 'user');

	let copied = $state(false);
	let copyError = $state(false);
	let copyTimer: ReturnType<typeof setTimeout> | null = null;

	/**
	 * Attachment currently shown full-size, or `null` when no zoom is open.
	 * `<a target="_blank">` is a no-op under Tauri (no native window.open),
	 * so the zoom is implemented as an in-app overlay instead.
	 */
	let zoomedAttachment = $state<MessageAttachment | null>(null);
	let zoomTrigger = $state<HTMLElement | null>(null);

	function openZoom(att: MessageAttachment, trigger: HTMLElement): void {
		zoomTrigger = trigger;
		zoomedAttachment = att;
	}

	function closeZoom(): void {
		zoomedAttachment = null;
		zoomTrigger?.focus();
		zoomTrigger = null;
	}

	function handleZoomKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape') {
			event.preventDefault();
			closeZoom();
		}
	}

	$effect(() => {
		return () => {
			if (copyTimer !== null) {
				clearTimeout(copyTimer);
				copyTimer = null;
			}
		};
	});

	/**
	 * Copy message content as markdown to clipboard.
	 * Handles clipboard API errors gracefully with visual feedback.
	 */
	async function copyContent(): Promise<void> {
		copyError = false;
		if (copyTimer !== null) {
			clearTimeout(copyTimer);
			copyTimer = null;
		}
		try {
			await copyToClipboard(message.content);
			copied = true;
			copyTimer = setTimeout(() => {
				copied = false;
				copyTimer = null;
			}, 2000);
		} catch {
			copyError = true;
			copyTimer = setTimeout(() => {
				copyError = false;
				copyTimer = null;
			}, 2000);
		}
	}

	/**
	 * Format timestamp for display
	 */
	function formatTime(date: Date): string {
		const d = date instanceof Date ? date : new Date(date);
		return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
	}
</script>

<div class="message-row" class:user={isUserMessage} class:assistant={!isUserMessage}>
	{#if message.attachments && message.attachments.length > 0}
		<ul class="message-attachments">
			{#each message.attachments as att, i (i)}
				<li>
					<button
						type="button"
						class="attachment-thumb"
						onclick={(e) => openZoom(att, e.currentTarget)}
						aria-label={att.name ?? $i18n('chat_attachment')}
					>
						<img
							src={`data:${att.mime_type};base64,${att.data_base64}`}
							alt={att.name ?? $i18n('chat_attachment')}
							loading="lazy"
						/>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
	{#if message.content}
		<div class="message-bubble" class:user={isUserMessage} class:assistant={!isUserMessage}>
			<div class="message-content">
				{#if isUserMessage}
					{message.content}
				{:else}
					<MarkdownRenderer content={message.content} />
				{/if}
			</div>
		</div>
	{/if}
	<div class="message-footer">
		<span class="message-time">
			<Clock size={12} />
			{formatTime(message.timestamp)}
		</span>
		{#if !isUserMessage}
			<button
				class="copy-button"
				class:copy-error={copyError}
				onclick={copyContent}
				aria-label={$i18n('chat_copy_arialabel')}
			>
				{#if copied}
					<Check size={14} />
				{:else if copyError}
					<CircleAlert size={14} />
				{:else}
					<Copy size={14} />
				{/if}
			</button>
		{/if}
	</div>
</div>

{#if zoomedAttachment}
	<div
		class="image-zoom-overlay"
		role="dialog"
		aria-modal="true"
		aria-label={zoomedAttachment.name ?? $i18n('chat_attachment')}
		tabindex="-1"
		onkeydown={handleZoomKeydown}
		{@attach (el) => {
			// Steal focus on open so the dialog catches Escape immediately,
			// without requiring the user to click into the overlay first.
			(el as HTMLElement).focus();
		}}
	>
		<button
			type="button"
			class="image-zoom-backdrop"
			aria-label={$i18n('chat_remove_attachment')}
			onclick={closeZoom}
		></button>
		<img
			class="image-zoom-image"
			src={`data:${zoomedAttachment.mime_type};base64,${zoomedAttachment.data_base64}`}
			alt={zoomedAttachment.name ?? $i18n('chat_attachment')}
		/>
		<button
			type="button"
			class="image-zoom-close"
			aria-label={$i18n('chat_remove_attachment')}
			onclick={closeZoom}
		>
			<X size={20} />
		</button>
	</div>
{/if}

<style>
	/* Row: bubble + footer stacked; timestamp and actions live OUTSIDE the
	   bubble so the bubble only carries the message ink. */
	.message-row {
		display: flex;
		flex-direction: column;
		gap: 4px;
		animation: fadeIn var(--transition-base);
	}

	.message-row.user {
		align-items: flex-end;
	}

	.message-row.assistant {
		align-items: stretch;
	}

	.message-bubble {
		padding: 0.7rem 1rem;
		border-radius: var(--border-radius-lg);
		position: relative;
	}

	/* User bubble sits on the brand gradient with dark ink (site button
	   convention): white text on the pale turquoise failed AA contrast. */
	.message-bubble.user {
		max-width: 72ch;
		background: var(--gradient-brand);
		color: var(--color-accent-text);
		border-bottom-right-radius: var(--border-radius-sm);
		box-shadow: var(--shadow-xs);
	}

	.message-bubble.assistant {
		background: var(--surface-1);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-bottom-left-radius: var(--border-radius-sm);
		box-shadow: var(--shadow-xs);
	}

	.message-content {
		font-size: var(--font-size-sm);
		line-height: var(--line-height-relaxed);
		word-break: break-word;
	}

	.message-attachments {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-sm);
		margin: 0 0 var(--spacing-sm) 0;
		padding: 0;
		list-style: none;
	}

	.message-attachments li {
		list-style: none;
	}

	.message-attachments .attachment-thumb {
		display: block;
		max-width: 220px;
		max-height: 220px;
		padding: 0;
		overflow: hidden;
		border-radius: var(--border-radius-sm);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		cursor: zoom-in;
	}

	.message-attachments .attachment-thumb:focus-visible {
		outline: 2px solid var(--color-accent-deep);
		outline-offset: 2px;
	}

	.message-attachments .attachment-thumb img {
		display: block;
		max-width: 100%;
		max-height: 220px;
		object-fit: contain;
	}

	.image-zoom-overlay {
		position: fixed;
		inset: 0;
		/* Full-screen overlay: stack at the modal layer, not the dropdown layer,
		   so popovers and sticky chrome can never paint above the zoomed image. */
		z-index: var(--z-index-modal);
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-lg);
	}

	.image-zoom-backdrop {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		background: rgba(0, 0, 0, 0.8);
		border: none;
		padding: 0;
		cursor: zoom-out;
	}

	.image-zoom-image {
		position: relative;
		max-width: min(95vw, 1600px);
		max-height: 90vh;
		object-fit: contain;
		border-radius: var(--border-radius-sm);
		box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
		background: var(--color-bg-primary);
	}

	.image-zoom-close {
		position: absolute;
		top: var(--spacing-md);
		right: var(--spacing-md);
		width: 36px;
		height: 36px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(0, 0, 0, 0.6);
		color: white;
		border: none;
		border-radius: 50%;
		cursor: pointer;
		padding: 0;
	}

	.image-zoom-close:hover {
		background: rgba(0, 0, 0, 0.85);
	}

	/* User messages: plain text with pre-wrap */
	.message-bubble.user .message-content {
		white-space: pre-wrap;
	}

	.message-footer {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		font-size: var(--font-size-2xs);
		color: var(--color-text-tertiary);
	}

	.message-row.user .message-footer {
		justify-content: flex-end;
	}

	.message-time {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.copy-button {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-xs);
		border: none;
		background: transparent;
		color: var(--color-text-tertiary);
		cursor: pointer;
		border-radius: var(--border-radius-sm);
		opacity: 0;
		transition:
			opacity 0.2s ease,
			color 0.2s ease,
			background-color 0.2s ease;
	}

	.message-row.assistant:hover .copy-button,
	.copy-button:focus-visible {
		opacity: 1;
	}

	.copy-button:hover {
		color: var(--color-text-primary);
		background: var(--color-bg-tertiary);
	}

	.copy-button.copy-error {
		opacity: 1;
		color: var(--color-danger);
	}
</style>
