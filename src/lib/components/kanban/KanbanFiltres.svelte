<!--
  Copyright 2025 Assistance Micro Design

  KanbanFiltres — filters by Kanban agent and target folder.
  Status is intentionally not exposed: the columns ARE the statuses,
  and column transitions are driven by the backend (workflow lifecycle).
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';
	import { Select, type SelectOption } from '$lib/components/ui';
	import type { AgentSummary } from '$types/agent';
	import type { WorkflowFolder } from '$types/workflow';

	interface Props {
		agents: AgentSummary[];
		folders: WorkflowFolder[];
		selectedAgentId: string;
		selectedFolderId: string;
		onchange: (filters: { agentId: string; folderId: string }) => void;
	}

	let { agents, folders, selectedAgentId, selectedFolderId, onchange }: Props = $props();

	const agentOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_filter_all_agents') },
		...agents.filter((a) => a.kind === 'kanban').map((a) => ({ value: a.id, label: a.name }))
	]);

	const folderOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_filter_all_folders') },
		...folders.map((f) => ({ value: f.id, label: f.name }))
	]);

	function emit(part: Partial<Parameters<typeof onchange>[0]>): void {
		onchange({
			agentId: selectedAgentId,
			folderId: selectedFolderId,
			...part
		});
	}
</script>

<div class="kanban-filtres">
	<div class="kanban-filtre">
		<Select
			ariaLabel={$i18n('kanban_filter_agent')}
			options={agentOptions}
			value={selectedAgentId}
			onchange={(e) => emit({ agentId: e.currentTarget.value })}
		/>
	</div>
	<div class="kanban-filtre">
		<Select
			ariaLabel={$i18n('kanban_filter_folder')}
			options={folderOptions}
			value={selectedFolderId}
			onchange={(e) => emit({ folderId: e.currentTarget.value })}
		/>
	</div>
</div>

<style>
	/* Compact inline selects, designed to sit on the page-title row. */
	.kanban-filtres {
		display: flex;
		gap: 0.5rem;
		flex-wrap: wrap;
		align-items: center;
	}
	.kanban-filtre {
		min-width: 180px;
	}
	.kanban-filtre :global(.form-group) {
		margin-bottom: 0;
	}
</style>
