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

<div class="workflow-list" class:collapsed role="listbox" aria-label="Workflow list">
	{#if workflows.length === 0}
		<div class="workflow-list-empty">
			{#if collapsed}
				<span class="empty-icon" title="No workflows">-</span>
			{:else}
				<p>No workflows yet</p>
				<p class="hint">Create a new workflow to get started</p>
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
