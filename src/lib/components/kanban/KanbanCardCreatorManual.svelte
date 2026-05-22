<!--
  Copyright 2025 Assistance Micro Design

  KanbanCardCreatorManual — sub-pane of KanbanCardCreator (Mode B).
  Full form (target agent, prompt or inline prompt, variables, folder,
  optional recurrence). Defers recurrence UI to KanbanScheduleForm.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { i18n } from '$lib/i18n';
	import { tauriInvoke as invoke } from '$lib/tauri';
	import { getErrorMessage } from '$lib/utils/error';
	import { Select, Input, Textarea } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui';
	import KanbanScheduleForm from './KanbanScheduleForm.svelte';
	import type { AgentSummary } from '$types/agent';
	import type { Prompt, PromptSummary, PromptVariable } from '$types/prompt';
	import type { WorkflowFolder } from '$types/workflow';
	import type { KanbanCardCreate, KanbanScheduleCreate } from '$types/kanban';

	interface Props {
		agents: AgentSummary[];
		prompts: PromptSummary[];
		folders: WorkflowFolder[];
		kanbanAgentOptions: SelectOption[];
		targetAgentOptions: SelectOption[];
		promptOptions: SelectOption[];
		folderOptions: SelectOption[];
		defaultKanbanAgentId: string;
		onerror: (message: string | null) => void;
	}

	let {
		agents: _agents,
		prompts: _prompts,
		folders: _folders,
		kanbanAgentOptions,
		targetAgentOptions,
		promptOptions,
		folderOptions,
		defaultKanbanAgentId,
		onerror
	}: Props = $props();

	let title = $state('');
	let description = $state('');
	let kanbanAgentId = $state(untrack(() => defaultKanbanAgentId));
	let targetAgentId = $state('');
	let promptId = $state('');
	let inlinePrompt = $state('');
	let targetFolderId = $state('');
	let variableValues = $state<Record<string, string>>({});
	let selectedPromptVariables = $state<PromptVariable[]>([]);
	let scheduleEnabled = $state(false);
	let scheduleDays = $state<number[]>([]);
	let scheduleHour = $state(9);
	let scheduleMinute = $state(0);

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
			const next: Record<string, string> = {};
			for (const v of selectedPromptVariables) {
				next[v.name] = variableValues[v.name] ?? v.defaultValue ?? '';
			}
			variableValues = next;
		} catch (e) {
			onerror(getErrorMessage(e));
		}
	}

	function validate(): string | null {
		if (!title.trim()) return $i18n('kanban_error_title_required');
		if (!kanbanAgentId) return $i18n('kanban_error_kanban_agent_required');
		if (!targetAgentId) return $i18n('kanban_error_target_agent_required');
		if (!!promptId === !!inlinePrompt.trim()) return $i18n('kanban_error_prompt_xor');
		if (scheduleEnabled && scheduleDays.length === 0) {
			return $i18n('kanban_error_schedule_days_required');
		}
		return null;
	}

	/** Returns the payload + optional schedule, or `null` if validation fails. */
	export function buildPayload(): {
		payload: KanbanCardCreate;
		schedule?: Omit<KanbanScheduleCreate, 'card_template_id'>;
	} | null {
		const err = validate();
		if (err) {
			onerror(err);
			return null;
		}
		onerror(null);
		const payload: KanbanCardCreate = {
			title: title.trim(),
			description: description.trim() || undefined,
			kanban_agent_id: kanbanAgentId,
			target_agent_id: targetAgentId,
			prompt_id: promptId || undefined,
			inline_prompt: inlinePrompt.trim() || undefined,
			variables: JSON.stringify(variableValues),
			target_folder_id: targetFolderId || undefined
		};
		const schedule = scheduleEnabled
			? {
					days_of_week: scheduleDays.slice().sort((a, b) => a - b),
					hour: scheduleHour,
					minute: scheduleMinute
				}
			: undefined;
		return { payload, schedule };
	}
</script>

<div class="form-section">
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
	<Select
		label={$i18n('kanban_kanban_agent')}
		options={kanbanAgentOptions}
		value={kanbanAgentId}
		onchange={(e) => (kanbanAgentId = e.currentTarget.value)}
	/>
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

	<KanbanScheduleForm
		enabled={scheduleEnabled}
		daysOfWeek={scheduleDays}
		hour={scheduleHour}
		minute={scheduleMinute}
		onchange={(v) => {
			scheduleEnabled = v.enabled;
			scheduleDays = v.daysOfWeek;
			scheduleHour = v.hour;
			scheduleMinute = v.minute;
		}}
	/>
</div>

<style>
	.form-section {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
	}
	.variables {
		border: 1px solid var(--color-border);
		border-radius: 6px;
		padding: 0.5rem 0.75rem;
		margin: 0;
	}
</style>
