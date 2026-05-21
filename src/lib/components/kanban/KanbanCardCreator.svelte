<!--
  Copyright 2025 Assistance Micro Design

  KanbanCardCreator — modal with two creation modes:
   - Mode A (auto): a free-form description fed to the Kanban agent via
     `compose_card_from_description` which returns a structured KanbanCardCreate.
   - Mode B (manual): full form (target agent, prompt or inline prompt, variables,
     folder, optional recurrence).
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
	import type { KanbanCardCreate, KanbanScheduleCreate } from '$types/kanban';

	interface Props {
		open: boolean;
		agents: AgentSummary[];
		prompts: PromptSummary[];
		folders: WorkflowFolder[];
		/** Pre-selected Kanban agent (current filter), or empty. */
		defaultKanbanAgentId?: string;
		onclose: () => void;
		oncreated: (
			payload: KanbanCardCreate,
			schedule?: Omit<KanbanScheduleCreate, 'card_template_id'>
		) => Promise<void>;
	}

	let {
		open,
		agents,
		prompts,
		folders,
		defaultKanbanAgentId = '',
		onclose,
		oncreated
	}: Props = $props();

	type Mode = 'auto' | 'manual';
	let mode = $state<Mode>('auto');
	let submitting = $state(false);
	let error = $state<string | null>(null);

	// ----- Mode A (auto) -----
	let autoDescription = $state('');
	let autoKanbanAgentId = $state('');
	let autoPreview = $state<KanbanCardCreate | null>(null);

	// ----- Mode B (manual) -----
	let title = $state('');
	let description = $state('');
	let kanbanAgentId = $state('');
	let targetAgentId = $state('');
	let promptId = $state('');
	let inlinePrompt = $state('');
	let targetFolderId = $state('');
	let variableValues = $state<Record<string, string>>({});
	let scheduleEnabled = $state(false);
	let scheduleDays = $state<number[]>([]);
	let scheduleHour = $state(9);
	let scheduleMinute = $state(0);

	$effect(() => {
		if (open) {
			autoKanbanAgentId = defaultKanbanAgentId || autoKanbanAgentId;
			kanbanAgentId = defaultKanbanAgentId || kanbanAgentId;
		}
	});

	const kanbanAgentOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_select_kanban_agent') },
		...agents.filter((a) => a.kind === 'kanban').map((a) => ({ value: a.id, label: a.name }))
	]);

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

	let selectedPromptVariables = $state<PromptVariable[]>([]);

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
			error = getErrorMessage(e);
		}
	}

	function reset(): void {
		mode = 'auto';
		error = null;
		submitting = false;
		autoDescription = '';
		autoPreview = null;
		title = '';
		description = '';
		targetAgentId = '';
		promptId = '';
		inlinePrompt = '';
		targetFolderId = '';
		variableValues = {};
		selectedPromptVariables = [];
		scheduleEnabled = false;
		scheduleDays = [];
		scheduleHour = 9;
		scheduleMinute = 0;
	}

	function close(): void {
		reset();
		onclose();
	}

	async function runAutoCompose(): Promise<void> {
		error = null;
		autoPreview = null;
		if (!autoKanbanAgentId) {
			error = $i18n('kanban_error_kanban_agent_required');
			return;
		}
		if (!autoDescription.trim()) {
			error = $i18n('kanban_error_description_required');
			return;
		}
		submitting = true;
		try {
			const result = await invoke<KanbanCardCreate>('compose_card_from_description', {
				kanbanAgentId: autoKanbanAgentId,
				description: autoDescription
			});
			autoPreview = result;
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			submitting = false;
		}
	}

	async function submitAuto(): Promise<void> {
		if (!autoPreview) {
			await runAutoCompose();
			if (!autoPreview) return;
		}
		submitting = true;
		try {
			await oncreated(autoPreview);
			close();
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			submitting = false;
		}
	}

	function validateManual(): string | null {
		if (!title.trim()) return $i18n('kanban_error_title_required');
		if (!kanbanAgentId) return $i18n('kanban_error_kanban_agent_required');
		if (!targetAgentId) return $i18n('kanban_error_target_agent_required');
		if (!!promptId === !!inlinePrompt.trim()) return $i18n('kanban_error_prompt_xor');
		if (scheduleEnabled && scheduleDays.length === 0) {
			return $i18n('kanban_error_schedule_days_required');
		}
		return null;
	}

	async function submitManual(): Promise<void> {
		error = null;
		const validation = validateManual();
		if (validation) {
			error = validation;
			return;
		}
		submitting = true;
		try {
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
			await oncreated(payload, schedule);
			close();
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			submitting = false;
		}
	}

	function toggleDay(idx: number): void {
		scheduleDays = scheduleDays.includes(idx)
			? scheduleDays.filter((d) => d !== idx)
			: [...scheduleDays, idx];
	}

	const dayKeys = ['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'] as const;
</script>

<Modal {open} title={$i18n('kanban_create_title')} onclose={close}>
	{#snippet body()}
		<div class="mode-tabs" role="tablist">
			<button
				type="button"
				role="tab"
				class="tab"
				class:active={mode === 'auto'}
				aria-selected={mode === 'auto'}
				onclick={() => (mode = 'auto')}
			>
				{$i18n('kanban_mode_auto')}
			</button>
			<button
				type="button"
				role="tab"
				class="tab"
				class:active={mode === 'manual'}
				aria-selected={mode === 'manual'}
				onclick={() => (mode = 'manual')}
			>
				{$i18n('kanban_mode_manual')}
			</button>
		</div>

		{#if error}
			<p class="error" role="alert">{error}</p>
		{/if}

		{#if mode === 'auto'}
			<div class="form-section">
				<Select
					label={$i18n('kanban_kanban_agent')}
					options={kanbanAgentOptions}
					value={autoKanbanAgentId}
					onchange={(e) => (autoKanbanAgentId = e.currentTarget.value)}
				/>
				<Textarea
					label={$i18n('kanban_describe_card')}
					value={autoDescription}
					oninput={(e) => (autoDescription = e.currentTarget.value)}
					rows={6}
				/>
				{#if autoPreview}
					<div class="auto-preview">
						<h4>{$i18n('kanban_preview_title')}</h4>
						<dl>
							<dt>{$i18n('kanban_field_title')}</dt>
							<dd>{autoPreview.title}</dd>
							<dt>{$i18n('kanban_field_target_agent')}</dt>
							<dd>{agents.find((a) => a.id === autoPreview?.target_agent_id)?.name ?? '—'}</dd>
							{#if autoPreview.prompt_id}
								<dt>{$i18n('kanban_field_prompt')}</dt>
								<dd>{prompts.find((p) => p.id === autoPreview?.prompt_id)?.name ?? '—'}</dd>
							{/if}
							{#if autoPreview.inline_prompt}
								<dt>{$i18n('kanban_field_inline_prompt')}</dt>
								<dd class="multiline">{autoPreview.inline_prompt}</dd>
							{/if}
						</dl>
					</div>
				{/if}
			</div>
		{:else}
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

				<fieldset class="schedule">
					<legend>
						<label>
							<input
								type="checkbox"
								checked={scheduleEnabled}
								onchange={(e) => (scheduleEnabled = (e.target as HTMLInputElement).checked)}
							/>
							{$i18n('kanban_schedule_enable')}
						</label>
					</legend>
					{#if scheduleEnabled}
						<div class="days">
							{#each dayKeys as key, idx (key)}
								<label class="day-chip" class:active={scheduleDays.includes(idx)}>
									<input
										type="checkbox"
										checked={scheduleDays.includes(idx)}
										onchange={() => toggleDay(idx)}
									/>
									<span>{$i18n(`kanban_day_${key}`)}</span>
								</label>
							{/each}
						</div>
						<label class="time-row">
							{$i18n('kanban_schedule_time')}
							<input
								type="time"
								class="form-input"
								value={`${scheduleHour.toString().padStart(2, '0')}:${scheduleMinute.toString().padStart(2, '0')}`}
								onchange={(e) => {
									const [h, m] = (e.target as HTMLInputElement).value.split(':');
									scheduleHour = Math.max(0, Math.min(23, Number.parseInt(h ?? '0', 10) || 0));
									scheduleMinute = Math.max(0, Math.min(59, Number.parseInt(m ?? '0', 10) || 0));
								}}
							/>
						</label>
					{/if}
				</fieldset>
			</div>
		{/if}
	{/snippet}
	{#snippet footer()}
		<Button variant="ghost" onclick={close} disabled={submitting}>
			{$i18n('common_cancel')}
		</Button>
		{#if mode === 'auto'}
			<Button variant="secondary" onclick={runAutoCompose} disabled={submitting}>
				{$i18n('kanban_compose_preview')}
			</Button>
			<Button variant="primary" onclick={submitAuto} disabled={submitting || !autoPreview}>
				{$i18n('common_create')}
			</Button>
		{:else}
			<Button variant="primary" onclick={submitManual} disabled={submitting}>
				{$i18n('common_create')}
			</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.mode-tabs {
		display: flex;
		gap: 0.5rem;
		margin-bottom: 0.75rem;
	}
	.tab {
		background: none;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		padding: 0.35rem 0.75rem;
		cursor: pointer;
		color: var(--color-text);
	}
	.tab.active {
		background: var(--color-accent);
		color: var(--color-accent-text);
		border-color: var(--color-accent);
	}
	.error {
		color: var(--color-error);
		margin: 0.25rem 0 0.75rem;
	}
	.form-section {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
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
	.variables,
	.schedule {
		border: 1px solid var(--color-border);
		border-radius: 6px;
		padding: 0.5rem 0.75rem;
		margin: 0;
	}
	.days {
		display: flex;
		gap: 0.3rem;
		flex-wrap: wrap;
		margin: 0.4rem 0;
	}
	.day-chip {
		display: inline-flex;
		align-items: center;
		padding: 0.25rem 0.5rem;
		border-radius: 999px;
		border: 1px solid var(--color-border);
		background: var(--color-surface);
		font-size: 0.8rem;
		cursor: pointer;
	}
	.day-chip.active {
		background: var(--color-accent);
		color: var(--color-accent-text);
		border-color: var(--color-accent);
	}
	.day-chip input {
		display: none;
	}
	.time-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}
</style>
