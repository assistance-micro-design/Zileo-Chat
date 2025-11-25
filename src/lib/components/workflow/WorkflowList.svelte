<!--
  WorkflowList Component
  A list of workflow items with selection support.

  @example
  <WorkflowList workflows={workflows} selectedId={currentWorkflowId} onselect={handleSelect} ondelete={handleDelete} />
-->
<script lang="ts">
	import type { Workflow } from '$types/workflow';
	import WorkflowItem from './WorkflowItem.svelte';

	/**
	 * WorkflowList props
	 */
	interface Props {
		/** Array of workflows to display */
		workflows: Workflow[];
		/** ID of the currently selected workflow */
		selectedId?: string;
		/** Selection handler */
		onselect?: (workflow: Workflow) => void;
		/** Delete handler */
		ondelete?: (workflow: Workflow) => void;
		/** Rename handler */
		onrename?: (workflow: Workflow, newName: string) => void;
	}

	let { workflows, selectedId, onselect, ondelete, onrename }: Props = $props();
</script>

<div class="workflow-list" role="listbox" aria-label="Workflow list">
	{#if workflows.length === 0}
		<div class="workflow-list-empty">
			<p>No workflows yet</p>
			<p class="hint">Create a new workflow to get started</p>
		</div>
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

	.workflow-list-empty {
		text-align: center;
		padding: var(--spacing-xl);
		color: var(--color-text-tertiary);
	}

	.workflow-list-empty p {
		font-size: var(--font-size-sm);
		margin: 0;
	}

	.workflow-list-empty .hint {
		font-size: var(--font-size-xs);
		margin-top: var(--spacing-sm);
	}
</style>
