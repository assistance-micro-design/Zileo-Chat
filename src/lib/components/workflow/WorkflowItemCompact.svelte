<!--
  WorkflowItemCompact Component
  A compact workflow item for collapsed sidebar display.
  Shows status indicator with tooltip containing workflow name.

  @example
  <WorkflowItemCompact workflow={wf} active={selectedId === wf.id} onselect={handleSelect} />
-->
<script lang="ts">
	import type { Workflow } from '$types/workflow';
	import StatusIndicator from '$lib/components/ui/StatusIndicator.svelte';

	/**
	 * WorkflowItemCompact props
	 */
	interface Props {
		/** Workflow data */
		workflow: Workflow;
		/** Whether this workflow is currently selected */
		active?: boolean;
		/** Selection handler */
		onselect?: (workflow: Workflow) => void;
	}

	let { workflow, active = false, onselect }: Props = $props();

	/**
	 * Handle workflow selection
	 */
	function handleSelect(): void {
		onselect?.(workflow);
	}

	/**
	 * Handle keyboard activation
	 */
	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			handleSelect();
		}
	}

	/**
	 * Get first letter of workflow name for avatar
	 */
	const initial = $derived(workflow.name.charAt(0).toUpperCase());
</script>

<button
	type="button"
	class="workflow-compact"
	class:active
	onclick={handleSelect}
	onkeydown={handleKeydown}
	title={workflow.name}
	aria-label={`Workflow: ${workflow.name}`}
	aria-pressed={active}
>
	<span class="workflow-avatar">
		{initial}
	</span>
	<span class="workflow-status">
		<StatusIndicator status={workflow.status} size="sm" />
	</span>
</button>

<style>
	.workflow-compact {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 36px;
		height: 36px;
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: all var(--transition-fast);
		border: 2px solid transparent;
		background: var(--color-bg-tertiary);
		padding: 0;
	}

	.workflow-compact:hover {
		background: var(--color-bg-hover);
		transform: scale(1.05);
	}

	.workflow-compact:focus-visible {
		outline: none;
		box-shadow: 0 0 0 3px var(--color-accent-light);
	}

	.workflow-compact.active {
		background: var(--color-accent-light);
		border-color: var(--color-accent);
	}

	.workflow-avatar {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-secondary);
		text-transform: uppercase;
	}

	.workflow-compact.active .workflow-avatar {
		color: var(--color-accent);
	}

	.workflow-status {
		position: absolute;
		bottom: -2px;
		right: -2px;
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-full);
		padding: 2px;
		line-height: 0;
	}
</style>
