<!--
  Copyright 2025 Assistance Micro Design

  KanbanFiltres — filters by Kanban agent and (optional) target folder / status.
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';
	import { Select, type SelectOption } from '$lib/components/ui';
	import type { AgentSummary } from '$types/agent';
	import type { WorkflowFolder } from '$types/workflow';
	import type { KanbanCardStatus } from '$types/kanban';

	interface Props {
		agents: AgentSummary[];
		folders: WorkflowFolder[];
		selectedAgentId: string;
		selectedFolderId: string;
		selectedStatus: KanbanCardStatus | '';
		onchange: (filters: {
			agentId: string;
			folderId: string;
			status: KanbanCardStatus | '';
		}) => void;
	}

	let { agents, folders, selectedAgentId, selectedFolderId, selectedStatus, onchange }: Props =
		$props();

	const agentOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_filter_all_agents') },
		...agents.filter((a) => a.kind === 'kanban').map((a) => ({ value: a.id, label: a.name }))
	]);

	const folderOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_filter_all_folders') },
		...folders.map((f) => ({ value: f.id, label: f.name }))
	]);

	const statusOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_filter_all_statuses') },
		{ value: 'todo', label: $i18n('kanban_status_todo') },
		{ value: 'ready', label: $i18n('kanban_status_ready') },
		{ value: 'doing', label: $i18n('kanban_status_doing') },
		{ value: 'review', label: $i18n('kanban_status_review') },
		{ value: 'done', label: $i18n('kanban_status_done') },
		{ value: 'failed', label: $i18n('kanban_status_failed') }
	]);

	function emit(part: Partial<Parameters<typeof onchange>[0]>): void {
		onchange({
			agentId: selectedAgentId,
			folderId: selectedFolderId,
			status: selectedStatus,
			...part
		});
	}
</script>

<div class="kanban-filtres">
	<div class="kanban-filtre">
		<Select
			label={$i18n('kanban_filter_agent')}
			options={agentOptions}
			value={selectedAgentId}
			onchange={(e) => emit({ agentId: e.currentTarget.value })}
		/>
	</div>
	<div class="kanban-filtre">
		<Select
			label={$i18n('kanban_filter_folder')}
			options={folderOptions}
			value={selectedFolderId}
			onchange={(e) => emit({ folderId: e.currentTarget.value })}
		/>
	</div>
	<div class="kanban-filtre">
		<Select
			label={$i18n('kanban_filter_status')}
			options={statusOptions}
			value={selectedStatus}
			onchange={(e) => emit({ status: e.currentTarget.value as KanbanCardStatus | '' })}
		/>
	</div>
</div>

<style>
	.kanban-filtres {
		display: flex;
		gap: 0.75rem;
		flex-wrap: wrap;
		padding: 0.5rem 0;
	}
	.kanban-filtre {
		min-width: 180px;
	}
</style>
