<!--
  ChatInput Component
  A message input area with send button and keyboard shortcuts.
  Supports Ctrl+Enter to send and auto-resize.

  @example
  <ChatInput value={inputValue} disabled={sending} onsend={handleSend} />
-->
<script lang="ts">
	import { Send, BookOpen } from 'lucide-svelte';
	import Spinner from '$lib/components/ui/Spinner.svelte';
	import PromptSelectorModal from './PromptSelectorModal.svelte';

	/**
	 * ChatInput props
	 */
	interface Props {
		/** Current input value */
		value?: string;
		/** Placeholder text */
		placeholder?: string;
		/** Whether input is disabled */
		disabled?: boolean;
		/** Whether currently sending */
		loading?: boolean;
		/** Send handler */
		onsend?: (message: string) => void;
	}

	let {
		value = $bindable(''),
		placeholder = 'Type your message...',
		disabled = false,
		loading = false,
		onsend
	}: Props = $props();

	let textareaRef: HTMLTextAreaElement;
	let showPromptSelector = $state(false);

	/**
	 * Handle send action
	 */
	function handleSend(): void {
		const trimmed = value.trim();
		if (trimmed && !disabled && !loading) {
			onsend?.(trimmed);
			value = '';
			adjustHeight();
		}
	}

	/**
	 * Handle keyboard events
	 */
	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			handleSend();
		}
		// Open prompt selector with Ctrl+P
		if (event.key === 'p' && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			showPromptSelector = true;
		}
	}

	/**
	 * Auto-adjust textarea height based on content
	 */
	function adjustHeight(): void {
		if (textareaRef) {
			textareaRef.style.height = 'auto';
			textareaRef.style.height = `${Math.min(textareaRef.scrollHeight, 200)}px`;
		}
	}

	/**
	 * Handle input changes
	 */
	function handleInput(): void {
		adjustHeight();
	}

	/**
	 * Handle prompt selection from modal
	 */
	function handlePromptSelect(content: string): void {
		value = content;
		showPromptSelector = false;
		adjustHeight();
	}
</script>

<div class="chat-input-container">
	<textarea
		bind:this={textareaRef}
		bind:value
		{placeholder}
		disabled={disabled || loading}
		class="chat-input"
		rows="1"
		oninput={handleInput}
		onkeydown={handleKeydown}
		aria-label="Message input"
	></textarea>
	<button
		type="button"
		class="prompt-button"
		title="Select from prompt library (Ctrl+P)"
		disabled={loading || disabled}
		onclick={() => (showPromptSelector = true)}
		aria-label="Open prompt library"
	>
		<BookOpen size={18} />
	</button>
	<button
		type="button"
		class="send-button"
		onclick={handleSend}
		disabled={disabled || loading || !value.trim()}
		aria-label="Send message"
	>
		{#if loading}
			<Spinner size="sm" />
		{:else}
			<Send size={20} />
		{/if}
	</button>
	<span class="keyboard-hint">Ctrl+Enter to send | Ctrl+P for prompts</span>
</div>

<PromptSelectorModal
	open={showPromptSelector}
	onclose={() => (showPromptSelector = false)}
	onselect={handlePromptSelect}
/>

<style>
	.chat-input-container {
		display: flex;
		align-items: flex-end;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border-top: 1px solid var(--color-border);
		position: relative;
	}

	.chat-input {
		flex: 1;
		min-height: 40px;
		max-height: 200px;
		padding: var(--spacing-sm) var(--spacing-md);
		font-size: var(--font-size-sm);
		font-family: inherit;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		resize: none;
		overflow-y: auto;
		transition: border-color var(--transition-fast);
	}

	.chat-input:focus {
		outline: none;
		border-color: var(--color-accent);
		box-shadow: 0 0 0 3px var(--color-accent-light);
	}

	.chat-input:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.prompt-button {
		width: 40px;
		height: 40px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--color-bg-primary);
		color: var(--color-accent);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: all var(--transition-fast);
		flex-shrink: 0;
	}

	.prompt-button:hover:not(:disabled) {
		background: var(--color-bg-secondary);
		border-color: var(--color-accent);
	}

	.prompt-button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.send-button {
		width: 40px;
		height: 40px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--color-accent);
		color: var(--color-text-inverse);
		border: none;
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: all var(--transition-fast);
		flex-shrink: 0;
	}

	.send-button:hover:not(:disabled) {
		background: var(--color-accent-hover);
	}

	.send-button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.keyboard-hint {
		position: absolute;
		bottom: 4px;
		right: var(--spacing-lg);
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		pointer-events: none;
	}
</style>
