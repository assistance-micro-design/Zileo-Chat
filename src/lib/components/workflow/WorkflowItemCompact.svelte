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
		/** Whether this workflow is currently running in the background */
		running?: boolean;
		/** Whether this workflow has a pending user question */
		hasQuestion?: boolean;
		/** Selection handler */
		onselect?: (workflow: Workflow) => void;
	}

	let { workflow, active = false, running = false, hasQuestion = false, onselect }: Props = $props();

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
	class:running
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
	{#if hasQuestion}
		<span class="question-badge"></span>
	{/if}
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

	.workflow-compact.running {
		animation: pulse-ring 2s ease-in-out infinite;
	}

	@keyframes pulse-ring {
		0%, 100% {
			box-shadow: 0 0 0 0 var(--color-success);
		}
		50% {
			box-shadow: 0 0 0 3px var(--color-success);
		}
	}

	.question-badge {
		position: absolute;
		top: -2px;
		right: -2px;
		width: 6px;
		height: 6px;
		border-radius: var(--border-radius-full);
		background: var(--color-warning);
	}
</style>
