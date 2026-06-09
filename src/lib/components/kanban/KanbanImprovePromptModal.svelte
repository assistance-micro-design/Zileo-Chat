<!--
  Copyright 2025 Assistance Micro Design

  KanbanImprovePromptModal — lets the operator (or, in auto mode, the Kanban
  agent acting on behalf of the user) edit a prompt's content with a mandatory
  edit_summary. The update is tagged with `edited_by = agent:<kanban_agent_id>`
  so the version history shows the provenance.
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';
	import { tauriInvoke as invoke } from '$lib/tauri';
	import { getErrorMessage } from '$lib/utils/error';
	import { Modal, Button, Input, Textarea } from '$lib/components/ui';
	import type { Prompt } from '$types/prompt';

	interface Props {
		open: boolean;
		promptId: string | null;
		/** Kanban agent ID — tagged in the edit history. */
		kanbanAgentId: string | null;
		/** Optional suggestion to pre-fill the content textarea with (used when
		 *  opened from an auto-analyze `needs_improvement` verdict). */
		suggestedContent?: string | null;
		onclose: () => void;
		onupdated?: () => void;
		/** Fired after a successful update via the "save & re-run" action so the
		 *  parent can re-queue the card for a fresh run with the improved prompt
		 *  (K4 — closes the needs_improvement loop). When absent, the re-run
		 *  button is hidden. */
		onsavedrequeue?: () => void;
	}

	let {
		open,
		promptId,
		kanbanAgentId,
		suggestedContent = null,
		onclose,
		onupdated,
		onsavedrequeue
	}: Props = $props();

	let prompt = $state<Prompt | null>(null);
	let content = $state('');
	let summary = $state('');
	let submitting = $state(false);
	let error = $state<string | null>(null);

	$effect(() => {
		if (open && promptId) {
			void load();
		}
		if (!open) {
			reset();
		}
	});

	async function load(): Promise<void> {
		if (!promptId) return;
		error = null;
		try {
			const full = await invoke<Prompt>('get_prompt', { promptId });
			prompt = full;
			// When opened from an auto-analyze `needs_improvement` verdict, pre-fill
			// the textarea with the analyzer's suggestion and seed the edit summary
			// so the operator only has to review and confirm.
			if (suggestedContent && suggestedContent.trim() !== full.content.trim()) {
				content = suggestedContent;
				summary = $i18n('kanban_improve_auto_summary');
			} else {
				content = full.content;
			}
		} catch (e) {
			error = getErrorMessage(e);
		}
	}

	function reset(): void {
		prompt = null;
		content = '';
		summary = '';
		submitting = false;
		error = null;
	}

	/** Persists the prompt update. Returns true on success (shared by the
	 *  plain "save" and the "save & re-run" actions). */
	async function persistUpdate(): Promise<boolean> {
		if (!promptId || !prompt) return false;
		if (!summary.trim()) {
			error = $i18n('kanban_improve_summary_required');
			return false;
		}
		if (content.trim() === prompt.content.trim()) {
			error = $i18n('kanban_improve_no_change');
			return false;
		}
		submitting = true;
		try {
			await invoke('update_prompt', {
				promptId,
				config: { content },
				editedBy: kanbanAgentId ? `agent:${kanbanAgentId}` : 'user',
				editSummary: summary.trim()
			});
			return true;
		} catch (e) {
			error = getErrorMessage(e);
			return false;
		} finally {
			submitting = false;
		}
	}

	async function submit(): Promise<void> {
		if (await persistUpdate()) {
			onupdated?.();
			onclose();
		}
	}

	/** K4: save the improved prompt AND re-queue the card for a fresh run, so
	 *  the corrected prompt is actually exercised (closes the needs_improvement
	 *  loop) instead of only benefiting future occurrences. */
	async function submitAndRequeue(): Promise<void> {
		if (await persistUpdate()) {
			onupdated?.();
			onsavedrequeue?.();
			onclose();
		}
	}
</script>

<Modal {open} title={$i18n('kanban_improve_title')} {onclose}>
	{#snippet body()}
		{#if error}
			<p class="error" role="alert">{error}</p>
		{/if}

		{#if prompt}
			<div class="improve-form">
				<p class="hint">{$i18n('kanban_improve_hint')} <strong>{prompt.name}</strong></p>

				<Textarea
					label={$i18n('kanban_improve_content')}
					value={content}
					oninput={(e) => (content = e.currentTarget.value)}
					rows={14}
				/>

				<Input
					label={$i18n('kanban_improve_summary')}
					value={summary}
					oninput={(e) => (summary = e.currentTarget.value)}
					placeholder={$i18n('kanban_improve_summary_placeholder')}
				/>
			</div>
		{:else}
			<p class="hint">{$i18n('versions_loading')}</p>
		{/if}
	{/snippet}
	{#snippet footer()}
		<Button variant="ghost" onclick={onclose} disabled={submitting}>
			{$i18n('common_cancel')}
		</Button>
		<Button variant="secondary" onclick={submit} disabled={submitting || !prompt}>
			{$i18n('common_save')}
		</Button>
		{#if onsavedrequeue}
			<Button variant="primary" onclick={submitAndRequeue} disabled={submitting || !prompt}>
				{$i18n('kanban_improve_save_and_rerun')}
			</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.improve-form {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
	}
	.hint {
		margin: 0 0 0.4rem;
		color: var(--color-text-muted);
		font-size: var(--font-size-sm);
	}
	.error {
		color: var(--color-error);
		margin: 0 0 0.5rem;
	}
</style>
