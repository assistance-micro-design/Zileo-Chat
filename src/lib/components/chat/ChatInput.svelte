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
	import { Send, BookOpen, CircleStop, Paperclip, X, Clock } from '@lucide/svelte';
	import { openDialog, tauriInvoke as invoke } from '$lib/tauri';
	import Spinner from '$lib/components/ui/Spinner.svelte';
	import PromptSelectorModal from './PromptSelectorModal.svelte';
	import { i18n } from '$lib/i18n';
	import { generateUuid } from '$lib/utils/uuid';
	import { processImageFile } from '$lib/utils/image-processing';
	import type { AttachmentMime, MessageAttachment, PendingAttachment } from '$types/message';

	/** Max images per send. Mirrored backend-side in `save_message_core`. */
	const MAX_ATTACHMENTS = 8;
	/** Max raw size per image (4 MB). Re-validated at every boundary. */
	const MAX_FILE_SIZE_BYTES = 4 * 1024 * 1024;
	/** MIME types accepted by the multimodal pipeline. */
	const ALLOWED_MIME: AttachmentMime[] = ['image/png', 'image/jpeg', 'image/webp', 'image/gif'];
	/**
	 * File extensions allowed by the Tauri picker filter. Mirrored Rust-side
	 * in `tools/file_manager/helpers::ALLOWED_IMAGE_EXTENSIONS`; the lists are
	 * not wired through IPC because both ends are tiny static whitelists and
	 * an end-to-end test would still need to assert byte-for-byte equality.
	 * Kept next to `ALLOWED_MIME` so a future contributor sees both surfaces
	 * at once if they ever extend the whitelist.
	 */
	const ALLOWED_IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'gif'];
	/** Canvas resize threshold. ~1568px keeps Mistral/OpenAI happy. */
	const MAX_DIMENSION = 1568;

	/**
	 * Tauri command response shape for the picker. Defined inline because the
	 * shape is a backend implementation detail not used anywhere else.
	 */
	interface ImageReadResult {
		data_base64: string;
		mime_type: string;
		size_bytes: number;
		name: string;
	}

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
		/** Send handler — passes attachments when present. */
		onsend?: (message: string, attachments?: MessageAttachment[]) => void;
		/** Cancel handler (shows stop button when provided) */
		oncancel?: () => void;
		/**
		 * Whether the selected model is flagged as multimodal. When `false`,
		 * attaching an image surfaces a soft warning (no hard block — the user
		 * may still want to test). `undefined` keeps the UI quiet.
		 */
		modelSupportsVision?: boolean;
	}

	let {
		value = $bindable(''),
		placeholder = '',
		disabled = false,
		loading = false,
		onsend,
		oncancel,
		modelSupportsVision
	}: Props = $props();

	let pendingAttachments = $state<PendingAttachment[]>([]);
	let attachmentError = $state<string | null>(null);
	let isDragOver = $state(false);

	/**
	 * Auto-strip pending image attachments when the parent flips the model
	 * to a non-vision one (or any unknown/undefined value, which fails
	 * closed). Without this, attachments queued for a vision model would
	 * survive the switch and either be rejected at send time or sneak past
	 * the picker gate. The toast keeps the user informed of the silent
	 * clear so they understand why their thumbnails vanished.
	 *
	 * Wrapped behind a guard so the effect is a no-op on first render and
	 * whenever there is nothing to clear — `$state` reads inside `$effect`
	 * remain tracked, so the effect still re-runs on every change.
	 */
	$effect(() => {
		if (modelSupportsVision !== true && pendingAttachments.length > 0) {
			pendingAttachments = [];
			attachmentError = $i18n('chat_image_stripped_on_model_switch');
		}
	});

	/** True when there is anything to clear and warn about. */
	const hasAttachments = $derived(pendingAttachments.length > 0);
	/**
	 * Whether the current model accepts image attachments. Strict `=== true`
	 * is intentional: an unknown / undefined vision flag (still-loading model
	 * list, missing DB column) fails closed so the user cannot attach an
	 * image we have not proven the model can consume. Matches the backend
	 * fallback in `resolve_workflow_supports_vision`.
	 */
	const canAttachImages = $derived(modelSupportsVision === true);

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
	 * Strip the frontend-only fields (`id`, `preview_url`) before sending so
	 * the wire shape matches `MessageAttachment` exactly. Important because
	 * the backend validates these payloads strictly.
	 */
	function toMessageAttachments(): MessageAttachment[] {
		return pendingAttachments.map((att) => ({
			kind: att.kind,
			mime_type: att.mime_type,
			data_base64: att.data_base64,
			name: att.name,
			size_bytes: att.size_bytes
		}));
	}

	/**
	 * Handle send action. Allows sending with attachments and no text (the
	 * model still gets the user turn with the image content blocks).
	 */
	function handleSend(): void {
		const trimmed = value.trim();
		const canSend = (trimmed.length > 0 || hasAttachments) && !disabled && !loading;
		if (!canSend) return;
		// Safety net: should never fire because the picker/paste/drop paths all
		// gate on canAttachImages, but if a stale attachment survives a model
		// switch race, refuse to send rather than have the backend reject it
		// with a less-friendly error message.
		if (hasAttachments && !canAttachImages) {
			showError($i18n('chat_image_blocked_non_multimodal'));
			return;
		}
		const attachments = hasAttachments ? toMessageAttachments() : undefined;
		onsend?.(trimmed, attachments);
		value = '';
		pendingAttachments = [];
		attachmentError = null;
		if (textareaRef) {
			textareaRef.value = '';
			adjustHeight();
		}
	}

	/**
	 * Display an attachment error. The message is intentionally sticky: it
	 * clears only when a subsequent attachment succeeds (`addAttachmentFromFile`
	 * / `handlePickFiles`) or when the user removes the last pending
	 * attachment. No auto-timeout — a transient toast would race with the
	 * upload pipeline and disappear before the user reads what failed.
	 */
	function showError(message: string): void {
		attachmentError = message;
	}

	/** Add a new pending attachment from raw base64 (already validated). */
	function pushAttachment(input: {
		data_base64: string;
		mime_type: string;
		size_bytes: number;
		name?: string;
	}): void {
		const preview_url = `data:${input.mime_type};base64,${input.data_base64}`;
		pendingAttachments = [
			...pendingAttachments,
			{
				id: generateUuid(),
				kind: 'image',
				mime_type: input.mime_type as AttachmentMime,
				data_base64: input.data_base64,
				size_bytes: input.size_bytes,
				name: input.name,
				preview_url
			}
		];
	}

	/** Validate a `File` against the size/MIME/count caps and process it. */
	async function addAttachmentFromFile(file: File): Promise<void> {
		if (!canAttachImages) {
			showError($i18n('chat_image_blocked_non_multimodal'));
			return;
		}
		if (pendingAttachments.length >= MAX_ATTACHMENTS) {
			showError($i18n('chat_max_attachments_reached', { max: MAX_ATTACHMENTS }));
			return;
		}
		if (file.size > MAX_FILE_SIZE_BYTES) {
			showError($i18n('chat_image_too_large', { max: '4 MB' }));
			return;
		}
		if (!ALLOWED_MIME.includes(file.type as AttachmentMime)) {
			showError($i18n('chat_image_unsupported_format'));
			return;
		}
		try {
			const processed = await processImageFile(file, MAX_DIMENSION);
			pushAttachment({
				data_base64: processed.data_base64,
				mime_type: processed.mime_type,
				size_bytes: processed.size_bytes,
				name: file.name
			});
			attachmentError = null;
		} catch (e) {
			showError(e instanceof Error ? e.message : String(e));
		}
	}

	/** Remove a pending attachment by id. */
	function removeAttachment(id: string): void {
		pendingAttachments = pendingAttachments.filter((att) => att.id !== id);
		if (pendingAttachments.length === 0) {
			attachmentError = null;
		}
	}

	/** Open the Tauri file picker and add each chosen image. */
	async function handlePickFiles(): Promise<void> {
		if (!canAttachImages) {
			showError($i18n('chat_image_blocked_non_multimodal'));
			return;
		}
		try {
			const selected = await openDialog({
				multiple: true,
				filters: [
					{
						name: 'Images',
						extensions: ALLOWED_IMAGE_EXTENSIONS
					}
				]
			});
			if (!selected) return;
			const paths = Array.isArray(selected) ? selected : [selected];
			for (const path of paths) {
				if (pendingAttachments.length >= MAX_ATTACHMENTS) {
					showError($i18n('chat_max_attachments_reached', { max: MAX_ATTACHMENTS }));
					break;
				}
				try {
					const result = await invoke<ImageReadResult>('read_image_for_attachment', {
						path
					});
					pushAttachment(result);
					attachmentError = null;
				} catch (e) {
					showError(e instanceof Error ? e.message : String(e));
				}
			}
		} catch (e) {
			showError(e instanceof Error ? e.message : String(e));
		}
	}

	/** Drag&drop handler. */
	async function handleDrop(event: DragEvent): Promise<void> {
		event.preventDefault();
		isDragOver = false;
		const files = Array.from(event.dataTransfer?.files ?? []);
		for (const file of files) {
			if (pendingAttachments.length >= MAX_ATTACHMENTS) {
				showError($i18n('chat_max_attachments_reached', { max: MAX_ATTACHMENTS }));
				break;
			}
			await addAttachmentFromFile(file);
		}
	}

	function handleDragOver(event: DragEvent): void {
		event.preventDefault();
		isDragOver = true;
	}

	function handleDragLeave(event: DragEvent): void {
		event.preventDefault();
		isDragOver = false;
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
	 * Handle paste events.
	 *
	 * If the clipboard carries one or more image files, swallow the default
	 * behaviour (otherwise the textarea would insert a stringified blob name)
	 * and route them through the attachment pipeline.
	 *
	 * Images can arrive through two parallel surfaces depending on the WebView
	 * and the source app: `clipboardData.files` (canonical for binary blobs;
	 * what gnome-screenshot, Wayland snapshots and browser "Copy image" land
	 * on under Tauri/WebKit-Linux) or `clipboardData.items[].kind === 'file'`
	 * (older Chromium path). Trying `.files` first then falling back to
	 * `.items` avoids missing the screenshot case observed on Linux while
	 * still working on builds that only populate `.items`.
	 *
	 * Otherwise, fall through to the auto-resize side effect — some WebView
	 * builds fire `input` before the pasted content is laid out, so
	 * `scrollHeight` reads stale; a second adjust on the next animation frame
	 * is idempotent but guarantees the textarea catches up.
	 */
	async function handlePaste(event: ClipboardEvent): Promise<void> {
		const clipboard = event.clipboardData;
		if (!clipboard) {
			requestAnimationFrame(adjustHeight);
			return;
		}
		const filesFromList = Array.from(clipboard.files ?? []);
		const filesFromItems = Array.from(clipboard.items ?? [])
			.filter((it) => it.kind === 'file')
			.map((it) => it.getAsFile())
			.filter((f): f is File => f !== null);
		const candidates = filesFromList.length > 0 ? filesFromList : filesFromItems;
		const images = candidates.filter((f) => ALLOWED_MIME.includes(f.type as AttachmentMime));
		if (images.length > 0) {
			event.preventDefault();
			if (!canAttachImages) {
				// Hard block: drop the images on the floor with a clear toast so
				// the user knows nothing was attached. Any pasted text alongside
				// the image is preserved by the default paste behaviour because
				// we only call preventDefault when images are present, which
				// also stops the text part — accept that minor edge: pasting
				// "see attached: <image>" loses the text too. The toast tells
				// them so.
				showError($i18n('chat_image_blocked_non_multimodal'));
				return;
			}
			for (const file of images) {
				await addAttachmentFromFile(file);
			}
			return;
		}
		requestAnimationFrame(adjustHeight);
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

<div
	class="chat-input-frame"
	class:drag-over={isDragOver}
	ondragover={handleDragOver}
	ondragleave={handleDragLeave}
	ondrop={handleDrop}
	role="region"
	aria-label={$i18n('chat_input_arialabel')}
>
	{#if hasAttachments}
		<div class="attachments-preview" role="list">
			{#each pendingAttachments as att (att.id)}
				<div class="attachment-thumb" role="listitem">
					<img src={att.preview_url} alt={att.name ?? $i18n('chat_attachment')} loading="lazy" />
					<button
						type="button"
						class="attachment-remove"
						onclick={() => removeAttachment(att.id)}
						aria-label={$i18n('chat_remove_attachment')}
					>
						<X size={12} />
					</button>
				</div>
			{/each}
		</div>
	{/if}
	{#if hasAttachments && !canAttachImages}
		<div class="warning-banner" role="status" aria-live="polite">
			{$i18n('chat_warning_model_no_vision')}
		</div>
	{/if}
	{#if attachmentError}
		<div class="attachment-error" role="alert">
			{attachmentError}
		</div>
	{/if}
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
		<button
			type="button"
			class="attach-button"
			title={canAttachImages
				? $i18n('chat_attach_image')
				: $i18n('chat_image_picker_disabled_tooltip')}
			disabled={loading ||
				disabled ||
				!canAttachImages ||
				pendingAttachments.length >= MAX_ATTACHMENTS}
			aria-disabled={!canAttachImages}
			onclick={handlePickFiles}
			aria-label={$i18n('chat_attach_image')}
		>
			<Paperclip size={18} />
		</button>
		<div class="textarea-wrapper">
			{#if showPendingHint}
				<span id="chat-input-pending-hint" class="pending-hint" role="status" aria-live="polite">
					<Clock size={12} />
					{$i18n('chat_input_workflow_in_progress_hint')}
				</span>
			{/if}
			<textarea
				bind:this={textareaRef}
				bind:value
				placeholder={effectivePlaceholder}
				{disabled}
				class="chat-input"
				rows="1"
				oninput={handleInput}
				onpaste={handlePaste}
				onkeydown={handleKeydown}
				aria-label={$i18n('chat_input_arialabel')}
				aria-describedby={showPendingHint ? 'chat-input-pending-hint' : undefined}
			></textarea>
		</div>
		{#if oncancel}
			<button
				type="button"
				class="cancel-button"
				onclick={oncancel}
				title={$i18n('chat_cancel_arialabel')}
				aria-label={$i18n('chat_cancel_arialabel')}
			>
				<CircleStop size={20} />
			</button>
		{/if}
		<button
			type="button"
			class="send-button"
			onclick={handleSend}
			disabled={disabled || loading || (!value.trim() && !hasAttachments)}
			aria-disabled={disabled || loading || (!value.trim() && !hasAttachments)}
			title={loading ? $i18n('chat_input_send_disabled_tooltip') : undefined}
			aria-label={$i18n('chat_send_arialabel')}
		>
			{#if loading && !oncancel}
				<Spinner size="sm" />
			{:else}
				<Send size={20} />
			{/if}
		</button>
	</div>
</div>
<span class="keyboard-hint">{$i18n('chat_keyboard_hint')}</span>

<PromptSelectorModal
	open={showPromptSelector}
	onclose={() => (showPromptSelector = false)}
	onselect={handlePromptSelect}
/>

<style>
	/* Composer pill: the frame carries all the chrome (border, radius, glow)
	   so the textarea inside can stay borderless. */
	.chat-input-frame {
		background: var(--surface-1);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-xl);
		box-shadow: var(--shadow-sm);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-base),
			box-shadow var(--transition-base);
	}

	.chat-input-frame:focus-within {
		border-color: var(--color-accent-hover);
		box-shadow: var(--shadow-sm), var(--glow-accent-soft);
	}

	.chat-input-frame.drag-over {
		background: var(--color-accent-light);
	}

	.attachments-preview {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-sm);
		padding: var(--spacing-sm) var(--spacing-md) 0;
	}

	.attachment-thumb {
		position: relative;
		width: 64px;
		height: 64px;
		border-radius: var(--border-radius-sm);
		overflow: hidden;
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
	}

	.attachment-thumb img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}

	.attachment-remove {
		position: absolute;
		top: 2px;
		right: 2px;
		width: 18px;
		height: 18px;
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

	.attachment-remove:hover {
		background: rgba(0, 0, 0, 0.85);
	}

	.warning-banner {
		padding: var(--spacing-sm) var(--spacing-md);
		margin: var(--spacing-sm) var(--spacing-md) 0;
		background: var(--color-warning-light, rgba(255, 193, 7, 0.15));
		color: var(--color-warning);
		border: 1px solid var(--color-warning);
		border-radius: var(--border-radius-sm);
		font-size: var(--font-size-xs);
	}

	.attachment-error {
		padding: var(--spacing-sm) var(--spacing-md);
		margin: var(--spacing-sm) var(--spacing-md) 0;
		background: var(--color-danger-light, rgba(220, 53, 69, 0.1));
		color: var(--color-danger);
		border: 1px solid var(--color-danger);
		border-radius: var(--border-radius-sm);
		font-size: var(--font-size-xs);
	}

	.attach-button {
		width: 36px;
		height: 36px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		color: var(--color-text-secondary);
		border: none;
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: all var(--transition-fast);
		flex-shrink: 0;
	}

	.attach-button:hover:not(:disabled) {
		background: var(--color-bg-hover);
		color: var(--color-accent-deep);
	}

	.attach-button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.chat-input-container {
		display: flex;
		align-items: flex-end;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		position: relative;
	}

	.textarea-wrapper {
		position: relative;
		flex: 1;
		display: flex;
		flex-direction: column;
	}

	/* Borderless inside the pill: the frame's focus-within glow is the
	   focus affordance. */
	.chat-input {
		width: 100%;
		min-height: 40px;
		max-height: 200px;
		padding: var(--spacing-sm) var(--spacing-sm);
		font-size: var(--font-size-sm);
		font-family: inherit;
		line-height: 1.55;
		color: var(--color-text-primary);
		background: transparent;
		border: none;
		resize: none;
		overflow-y: auto;
	}

	.chat-input:focus {
		outline: none;
	}

	.chat-input:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	/* Queued-message pill: warning tint, sits above the textarea inside the
	   composer so the user sees the deferred-send state before typing more. */
	.pending-hint {
		display: inline-flex;
		align-items: center;
		align-self: flex-start;
		gap: var(--spacing-xs);
		padding: 0.1rem 0.5rem;
		margin-bottom: var(--spacing-xs);
		font-size: var(--font-size-2xs);
		color: var(--color-warning);
		background: var(--color-warning-light);
		border-radius: var(--border-radius-full);
	}

	.prompt-button {
		width: 36px;
		height: 36px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		color: var(--color-text-secondary);
		border: none;
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: all var(--transition-fast);
		flex-shrink: 0;
	}

	.prompt-button:hover:not(:disabled) {
		background: var(--color-bg-hover);
		color: var(--color-accent-deep);
	}

	.prompt-button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	/* Square gradient send button with dark ink, glowing on hover (site CTA) */
	.send-button {
		width: 36px;
		height: 36px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--gradient-brand);
		color: var(--color-accent-text);
		border: none;
		border-radius: 10px;
		box-shadow: var(--shadow-xs);
		cursor: pointer;
		transition:
			box-shadow var(--transition-fast),
			transform var(--transition-fast),
			filter var(--transition-fast);
		flex-shrink: 0;
	}

	.send-button:hover:not(:disabled) {
		filter: brightness(1.04);
		box-shadow: var(--glow-accent);
	}

	.send-button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	/* Ghost cancel button with red stop icon, next to the always-visible
	   send button (which stays disabled while the workflow runs). */
	.cancel-button {
		width: 36px;
		height: 36px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		color: var(--color-error);
		border: none;
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: background-color var(--transition-fast);
		flex-shrink: 0;
	}

	.cancel-button:hover {
		background: var(--color-bg-hover);
	}

	.keyboard-hint {
		display: block;
		margin-top: var(--spacing-xs);
		font-size: var(--font-size-2xs);
		color: var(--color-text-tertiary);
		text-align: center;
		pointer-events: none;
	}
</style>
