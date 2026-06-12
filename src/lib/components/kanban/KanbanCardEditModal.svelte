<!--
  Copyright 2025 Assistance Micro Design

  KanbanCardEditModal — edit a kanban card in place.

  Editable: title, description, target_agent, prompt (XOR inline_prompt),
  variables (per prompt schema), folder. Read-only: kanban_agent (the composer
  is bound to the card by design).
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';
	import { tauriInvoke as invoke } from '$lib/tauri';
	import { getErrorMessage } from '$lib/utils/error';
	import { Modal, Button, Select, Input, Textarea } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui';
	import type { AgentSummary } from '$types/agent';
	import type { Prompt, PromptSummary, PromptVariable } from '$types/prompt';
	import type { WorkflowFolder } from '$types/workflow';
	import type { KanbanCard, KanbanCardUpdate } from '$types/kanban';
	import { kanbanStore } from '$lib/stores/kanban';

	interface Props {
		open: boolean;
		card: KanbanCard | null;
		agents: AgentSummary[];
		prompts: PromptSummary[];
		folders: WorkflowFolder[];
		onclose: () => void;
		onsaved?: () => void;
	}

	let { open, card, agents, prompts, folders, onclose, onsaved }: Props = $props();

	let title = $state('');
	let description = $state('');
	let targetAgentId = $state('');
	let promptId = $state('');
	let inlinePrompt = $state('');
	let targetFolderId = $state('');
	let variableValues = $state<Record<string, string>>({});
	let selectedPromptVariables = $state<PromptVariable[]>([]);
	let error = $state<string | null>(null);
	let saving = $state(false);

	const targetAgentOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_select_target_agent') },
		...agents.filter((a) => a.kind !== 'kanban').map((a) => ({ value: a.id, label: a.name }))
	]);

	const promptOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_select_prompt_optional') },
		...prompts.map((p) => ({ value: p.id, label: p.name }))
	]);

	const folderOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_no_folder') },
		...folders.map((f) => ({ value: f.id, label: f.name }))
	]);

	const kanbanAgentLabel = $derived(
		card ? (agents.find((a) => a.id === card.kanban_agent_id)?.name ?? '—') : ''
	);

	// Re-sync local form state whenever the modal opens with a (possibly new) card.
	let lastSyncedCardId = $state<string | null>(null);
	$effect(() => {
		if (!open || !card) {
			lastSyncedCardId = null;
			return;
		}
		if (lastSyncedCardId === card.id) return;
		lastSyncedCardId = card.id;
		title = card.title;
		description = card.description;
		targetAgentId = card.target_agent_id;
		promptId = card.prompt_id ?? '';
		inlinePrompt = card.inline_prompt ?? '';
		targetFolderId = card.target_folder_id ?? '';
		try {
			variableValues = JSON.parse(card.variables || '{}');
		} catch {
			variableValues = {};
		}
		error = null;
	});

	$effect(() => {
		void loadPromptVariables(promptId);
	});

	async function loadPromptVariables(id: string): Promise<void> {
		if (!id) {
			selectedPromptVariables = [];
			return;
		}
		try {
			const full = await invoke<Prompt>('get_prompt', { promptId: id });
			selectedPromptVariables = full.variables ?? [];
			// Preserve user-entered values when the schema overlaps, fill in defaults
			// for newly-relevant variables.
			const next: Record<string, string> = {};
			for (const v of selectedPromptVariables) {
				next[v.name] = variableValues[v.name] ?? v.defaultValue ?? '';
			}
			variableValues = next;
		} catch (e) {
			error = getErrorMessage(e);
		}
	}

	function validate(): string | null {
		if (!title.trim()) return $i18n('kanban_error_title_required');
		if (!targetAgentId) return $i18n('kanban_error_target_agent_required');
		const hasPrompt = !!promptId;
		const hasInline = !!inlinePrompt.trim();
		if (hasPrompt === hasInline) return $i18n('kanban_error_prompt_xor');
		return null;
	}

	async function handleSave(): Promise<void> {
		if (!card) return;
		const v = validate();
		if (v) {
			error = v;
			return;
		}
		saving = true;
		error = null;
		try {
			// Build a minimal patch — tri-state semantics for clearable fields:
			// `null` = clear, `string` = set, absent = keep.
			const patch: KanbanCardUpdate = {
				title: title.trim(),
				description: description.trim(),
				target_agent_id: targetAgentId,
				variables: JSON.stringify(variableValues)
			};
			if (promptId) {
				patch.prompt_id = promptId;
				patch.inline_prompt = null;
			} else {
				patch.prompt_id = null;
				patch.inline_prompt = inlinePrompt.trim();
			}
			patch.target_folder_id = targetFolderId || null;
			await kanbanStore.updateCard(card.id, patch);
			onsaved?.();
			onclose();
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			saving = false;
		}
	}
</script>

<Modal {open} title={$i18n('kanban_edit_modal_title')} {onclose}>
	{#snippet body()}
		<div class="form-section">
			{#if error}
				<p class="modal-error" role="alert">{error}</p>
			{/if}

			<Input
				label={$i18n('kanban_field_title')}
				value={title}
				oninput={(e) => (title = e.currentTarget.value)}
			/>
			<Textarea
				label={$i18n('kanban_field_description')}
				value={description}
				oninput={(e) => (description = e.currentTarget.value)}
				rows={3}
			/>

			<div class="readonly-row">
				<span class="readonly-label">{$i18n('kanban_kanban_agent')}</span>
				<span class="readonly-value">{kanbanAgentLabel}</span>
			</div>

			<Select
				label={$i18n('kanban_field_target_agent')}
				options={targetAgentOptions}
				value={targetAgentId}
				onchange={(e) => (targetAgentId = e.currentTarget.value)}
			/>

			<Select
				label={$i18n('kanban_field_prompt')}
				options={promptOptions}
				value={promptId}
				onchange={(e) => (promptId = e.currentTarget.value)}
			/>

			{#if !promptId}
				<Textarea
					label={$i18n('kanban_field_inline_prompt')}
					value={inlinePrompt}
					oninput={(e) => (inlinePrompt = e.currentTarget.value)}
					rows={5}
				/>
			{/if}

			{#if selectedPromptVariables.length > 0}
				<fieldset class="variables">
					<legend>{$i18n('kanban_field_variables')}</legend>
					{#each selectedPromptVariables as variable (variable.name)}
						<Input
							label={variable.description || variable.name}
							value={variableValues[variable.name] ?? ''}
							oninput={(e) =>
								(variableValues = {
									...variableValues,
									[variable.name]: e.currentTarget.value
								})}
						/>
					{/each}
				</fieldset>
			{/if}

			<Select
				label={$i18n('kanban_field_folder')}
				options={folderOptions}
				value={targetFolderId}
				onchange={(e) => (targetFolderId = e.currentTarget.value)}
			/>
		</div>
	{/snippet}

	{#snippet footer()}
		<Button type="button" variant="ghost" onclick={onclose} disabled={saving}>
			{$i18n('common_cancel')}
		</Button>
		<Button type="button" variant="primary" onclick={handleSave} disabled={saving}>
			{$i18n('common_save')}
		</Button>
	{/snippet}
</Modal>

<style>
	.form-section {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}
	.readonly-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.5rem 0.75rem;
		background: var(--color-surface-alt, var(--color-surface));
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
	}
	.readonly-label {
		font-size: var(--font-size-sm);
		color: var(--color-text-muted);
	}
	.readonly-value {
		font-size: var(--font-size-sm);
		font-weight: 500;
	}
	.variables {
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		padding: 0.75rem;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	.modal-error {
		color: var(--color-error);
		margin: 0;
		font-size: var(--font-size-sm);
	}
</style>
