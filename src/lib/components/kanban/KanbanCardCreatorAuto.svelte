<!--
  Copyright 2025 Assistance Micro Design

  KanbanCardCreatorAuto — sub-pane of KanbanCardCreator (Mode A).
  Asks the Kanban agent to compose a structured KanbanCardCreate from a
  free-form description, then displays a preview before submission.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { i18n } from '$lib/i18n';
	import { tauriInvoke as invoke } from '$lib/tauri';
	import { getErrorMessage } from '$lib/utils/error';
	import { Select, Textarea, Spinner } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui';
	import type { AgentSummary } from '$types/agent';
	import type { PromptSummary } from '$types/prompt';
	import type { KanbanCardCreate } from '$types/kanban';

	interface Props {
		agents: AgentSummary[];
		prompts: PromptSummary[];
		kanbanAgentOptions: SelectOption[];
		defaultKanbanAgentId: string;
		submitting: boolean;
		/** Called when an error happens locally so the parent can render it. */
		onerror: (message: string | null) => void;
		/** Pushed up when the preview becomes available / clears. */
		onpreview: (preview: KanbanCardCreate | null) => void;
		/** Pushed up so parent can drive the submitting flag during compose. */
		onsubmittingchange: (value: boolean) => void;
	}

	let {
		agents,
		prompts,
		kanbanAgentOptions,
		defaultKanbanAgentId,
		submitting,
		onerror,
		onpreview,
		onsubmittingchange
	}: Props = $props();

	let description = $state('');
	let kanbanAgentId = $state(untrack(() => defaultKanbanAgentId));
	let preview = $state<KanbanCardCreate | null>(null);

	export async function compose(): Promise<KanbanCardCreate | null> {
		onerror(null);
		preview = null;
		onpreview(null);
		if (!kanbanAgentId) {
			onerror($i18n('kanban_error_kanban_agent_required'));
			return null;
		}
		if (!description.trim()) {
			onerror($i18n('kanban_error_description_required'));
			return null;
		}
		onsubmittingchange(true);
		try {
			const result = await invoke<KanbanCardCreate>('compose_card_from_description', {
				kanbanAgentId,
				description
			});
			preview = result;
			onpreview(result);
			return result;
		} catch (e) {
			onerror(getErrorMessage(e));
			return null;
		} finally {
			onsubmittingchange(false);
		}
	}

	export function getPreview(): KanbanCardCreate | null {
		return preview;
	}
</script>

<div class="form-section">
	<Select
		label={$i18n('kanban_kanban_agent')}
		options={kanbanAgentOptions}
		value={kanbanAgentId}
		onchange={(e) => (kanbanAgentId = e.currentTarget.value)}
	/>
	<Textarea
		label={$i18n('kanban_describe_card')}
		value={description}
		oninput={(e) => (description = e.currentTarget.value)}
		rows={6}
		disabled={submitting}
	/>
	{#if submitting && !preview}
		<div class="composing" role="status" aria-live="polite">
			<Spinner size="sm" />
			<span>{$i18n('kanban_composing_preview')}</span>
		</div>
	{/if}
	{#if preview}
		<div class="auto-preview">
			<h4>{$i18n('kanban_preview_title')}</h4>
			<dl>
				<dt>{$i18n('kanban_field_title')}</dt>
				<dd>{preview.title}</dd>
				<dt>{$i18n('kanban_field_target_agent')}</dt>
				<dd>{agents.find((a) => a.id === preview?.target_agent_id)?.name ?? '—'}</dd>
				{#if preview.prompt_id}
					<dt>{$i18n('kanban_field_prompt')}</dt>
					<dd>{prompts.find((p) => p.id === preview?.prompt_id)?.name ?? '—'}</dd>
				{/if}
				{#if preview.inline_prompt}
					<dt>{$i18n('kanban_field_inline_prompt')}</dt>
					<dd class="multiline">{preview.inline_prompt}</dd>
				{/if}
			</dl>
		</div>
	{/if}
</div>

<style>
	.form-section {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
	}
	.composing {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.6rem 0.75rem;
		background: var(--color-surface-alt, var(--color-surface));
		border: 1px dashed var(--color-border);
		border-radius: 6px;
		color: var(--color-text-muted);
		font-size: 0.85rem;
		font-style: italic;
	}
	.auto-preview {
		border: 1px solid var(--color-border);
		border-radius: 6px;
		padding: 0.6rem 0.75rem;
		background: var(--color-bg-secondary);
	}
	.auto-preview h4 {
		margin: 0 0 0.4rem;
	}
	.auto-preview dl {
		margin: 0;
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 0.3rem 0.6rem;
		font-size: 0.85rem;
	}
	.auto-preview dt {
		font-weight: 600;
		color: var(--color-text-muted);
	}
	.auto-preview dd {
		margin: 0;
	}
	.auto-preview .multiline {
		white-space: pre-wrap;
	}
</style>
