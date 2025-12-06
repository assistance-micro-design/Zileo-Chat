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
  WorkflowList Component
  A list of workflow items with selection support.
  Supports collapsed mode for compact sidebar display.

  @example
  <WorkflowList workflows={workflows} selectedId={currentWorkflowId} onselect={handleSelect} ondelete={handleDelete} />
  <WorkflowList workflows={workflows} collapsed={true} />
-->
<script lang="ts">
	import type { Workflow } from '$types/workflow';
	import WorkflowItem from './WorkflowItem.svelte';
	import WorkflowItemCompact from './WorkflowItemCompact.svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * WorkflowList props
	 */
	interface Props {
		/** Array of workflows to display */
		workflows: Workflow[];
		/** ID of the currently selected workflow */
		selectedId?: string;
		/** Whether to show compact view (collapsed sidebar) */
		collapsed?: boolean;
		/** Selection handler */
		onselect?: (workflow: Workflow) => void;
		/** Delete handler */
		ondelete?: (workflow: Workflow) => void;
		/** Rename handler */
		onrename?: (workflow: Workflow, newName: string) => void;
	}

	let { workflows, selectedId, collapsed = false, onselect, ondelete, onrename }: Props = $props();
</script>

<div class="workflow-list" class:collapsed role="listbox" aria-label={$i18n('workflow_list_arialabel')}>
	{#if workflows.length === 0}
		<div class="workflow-list-empty">
			{#if collapsed}
				<span class="empty-icon" title={$i18n('workflow_no_workflows_short')}>-</span>
			{:else}
				<p>{$i18n('workflow_no_workflows')}</p>
				<p class="hint">{$i18n('workflow_no_workflows_hint')}</p>
			{/if}
		</div>
	{:else if collapsed}
		{#each workflows as workflow (workflow.id)}
			<WorkflowItemCompact
				{workflow}
				active={workflow.id === selectedId}
				{onselect}
			/>
		{/each}
	{:else}
		{#each workflows as workflow (workflow.id)}
			<WorkflowItem
				{workflow}
				active={workflow.id === selectedId}
				{onselect}
				{ondelete}
				{onrename}
			/>
		{/each}
	{/if}
</div>

<style>
	.workflow-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.workflow-list.collapsed {
		gap: var(--spacing-sm);
		align-items: center;
	}

	.workflow-list-empty {
		text-align: center;
		padding: var(--spacing-xl);
		color: var(--color-text-tertiary);
	}

	.workflow-list.collapsed .workflow-list-empty {
		padding: var(--spacing-md);
	}

	.workflow-list-empty p {
		font-size: var(--font-size-sm);
		margin: 0;
	}

	.workflow-list-empty .hint {
		font-size: var(--font-size-xs);
		margin-top: var(--spacing-sm);
	}

	.empty-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		border-radius: var(--border-radius-md);
		background: var(--color-bg-tertiary);
		color: var(--color-text-tertiary);
		font-size: var(--font-size-sm);
	}
</style>
