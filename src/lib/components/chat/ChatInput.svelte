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
  ChatInput Component
  A message input area with send button and keyboard shortcuts.
  Supports Ctrl+Enter to send and auto-resize.

  The textarea is only disabled when the parent passes `disabled` (e.g. no
  agent selected). During execution (`loading`), the textarea stays editable
  so the user can pre-type the next turn; a hint surfaces when text is
  present to clarify that nothing is queued yet.

  @example
  <ChatInput value={inputValue} disabled={sending} onsend={handleSend} />
-->
<script lang="ts">
	import { Send, BookOpen, StopCircle } from '@lucide/svelte';
	import Spinner from '$lib/components/ui/Spinner.svelte';
	import PromptSelectorModal from './PromptSelectorModal.svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * ChatInput props
	 */
	interface Props {
		/** Current input value */
		value?: string;
		/** Placeholder text */
		placeholder?: string;
		/**
		 * Hard-disable the textarea (e.g. when no agent is selected). When
		 * `loading` is true the textarea still accepts input — only the send
		 * action is gated — so the user can pre-type while the agent runs.
		 */
		disabled?: boolean;
		/** Whether a workflow is currently executing */
		loading?: boolean;
		/** Send handler */
		onsend?: (message: string) => void;
		/** Cancel handler (shows stop button when provided) */
		oncancel?: () => void;
	}

	let {
		value = $bindable(''),
		placeholder = '',
		disabled = false,
		loading = false,
		onsend,
		oncancel
	}: Props = $props();

	/**
	 * Get effective placeholder (prop or i18n)
	 */
	const effectivePlaceholder = $derived(placeholder || $i18n('chat_input_placeholder'));

	/**
	 * True when the user has pre-typed content while a workflow is still
	 * executing. Drives the "Message en attente" hint so the user knows the
	 * text is not auto-queued.
	 */
	const showPendingHint = $derived(loading && value.trim().length > 0);

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
	 * Handle keyboard events.
	 * Ctrl/Cmd+K opens the prompt library without colliding with the browser's
	 * Ctrl+P (print) shortcut that the previous binding shadowed.
	 */
	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			handleSend();
		}
		if (event.key === 'k' && (event.ctrlKey || event.metaKey)) {
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
	<button
		type="button"
		class="prompt-button"
		title={$i18n('chat_prompt_library_title')}
		disabled={loading || disabled}
		onclick={() => (showPromptSelector = true)}
		aria-label={$i18n('chat_prompt_library_arialabel')}
	>
		<BookOpen size={18} />
	</button>
	<div class="textarea-wrapper">
		<textarea
			bind:this={textareaRef}
			bind:value
			placeholder={effectivePlaceholder}
			disabled={disabled}
			class="chat-input"
			rows="1"
			oninput={handleInput}
			onkeydown={handleKeydown}
			aria-label={$i18n('chat_input_arialabel')}
			aria-describedby={showPendingHint ? 'chat-input-pending-hint' : undefined}
		></textarea>
		{#if showPendingHint}
			<span
				id="chat-input-pending-hint"
				class="pending-hint"
				role="status"
				aria-live="polite"
			>
				{$i18n('chat_input_workflow_in_progress_hint')}
			</span>
		{/if}
	</div>
	{#if oncancel}
		<button
			type="button"
			class="stop-button"
			onclick={oncancel}
			aria-label={$i18n('chat_cancel_arialabel')}
		>
			<StopCircle size={20} />
		</button>
	{:else}
		<button
			type="button"
			class="send-button"
			onclick={handleSend}
			disabled={disabled || loading || !value.trim()}
			aria-disabled={disabled || loading || !value.trim()}
			title={loading ? $i18n('chat_input_send_disabled_tooltip') : undefined}
			aria-label={$i18n('chat_send_arialabel')}
		>
			{#if loading}
				<Spinner size="sm" />
			{:else}
				<Send size={20} />
			{/if}
		</button>
	{/if}
	{#if value.trim() && !loading}
		<span class="keyboard-hint">{$i18n('chat_keyboard_hint')}</span>
	{/if}
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

	.textarea-wrapper {
		position: relative;
		flex: 1;
		display: flex;
		flex-direction: column;
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

	.pending-hint {
		margin-top: 2px;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		font-style: italic;
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

	.stop-button {
		width: 40px;
		height: 40px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--color-danger);
		color: var(--color-text-inverse);
		border: none;
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: all var(--transition-fast);
		flex-shrink: 0;
	}

	.stop-button:hover {
		opacity: 0.85;
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
