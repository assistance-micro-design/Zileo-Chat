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
  WorkflowItem Component
  A single workflow item in the sidebar list.
  Supports inline rename, status indicator, and delete on hover.

  @example
  <WorkflowItem workflow={wf} active={selectedId === wf.id} onselect={handleSelect} ondelete={handleDelete} />
-->
<script lang="ts">
	import type { Workflow } from '$types/workflow';
	import StatusIndicator from '$lib/components/ui/StatusIndicator.svelte';
	import { X } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * WorkflowItem props
	 */
	interface Props {
		/** Workflow data */
		workflow: Workflow;
		/** Whether this workflow is currently selected */
		active?: boolean;
		/** Selection handler */
		onselect?: (workflow: Workflow) => void;
		/** Delete handler */
		ondelete?: (workflow: Workflow) => void;
		/** Rename handler */
		onrename?: (workflow: Workflow, newName: string) => void;
	}

	let { workflow, active = false, onselect, ondelete, onrename }: Props = $props();

	let editing = $state(false);
	let editName = $state(workflow.name);
	let nameInputRef = $state<HTMLInputElement | null>(null);

	/**
	 * Handle workflow selection
	 */
	function handleSelect(): void {
		if (!editing) {
			onselect?.(workflow);
		}
	}

	/**
	 * Start inline editing
	 */
	function startEdit(event: MouseEvent): void {
		event.stopPropagation();
		editing = true;
		editName = workflow.name;
		setTimeout(() => nameInputRef?.focus(), 0);
	}

	/**
	 * Finish editing and save
	 */
	function finishEdit(): void {
		editing = false;
		const trimmedName = editName.trim();
		if (trimmedName && trimmedName !== workflow.name) {
			onrename?.(workflow, trimmedName);
		}
	}

	/**
	 * Handle keyboard events during editing
	 */
	function handleEditKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter') {
			finishEdit();
		} else if (event.key === 'Escape') {
			editing = false;
			editName = workflow.name;
		}
	}

	/**
	 * Handle delete button click
	 */
	function handleDelete(event: MouseEvent): void {
		event.stopPropagation();
		ondelete?.(workflow);
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
</script>

<div
	class="workflow-item"
	class:active
	role="button"
	tabindex="0"
	onclick={handleSelect}
	onkeydown={handleKeydown}
	ondblclick={startEdit}
	aria-pressed={active}
	aria-label={`Workflow: ${workflow.name}`}
>
	<StatusIndicator status={workflow.status} size="sm" />
	{#if editing}
		<input
			bind:this={nameInputRef}
			bind:value={editName}
			type="text"
			class="workflow-name-input"
			onblur={finishEdit}
			onkeydown={handleEditKeydown}
			onclick={(e) => e.stopPropagation()}
			aria-label={$i18n('workflow_name_arialabel')}
		/>
	{:else}
		<span class="workflow-name">{workflow.name}</span>
	{/if}
	<button
		type="button"
		class="workflow-delete"
		onclick={handleDelete}
		aria-label={$i18n('workflow_delete_arialabel').replace('{name}', workflow.name)}
	>
		<X size={14} />
	</button>
</div>

<style>
	.workflow-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: all var(--transition-fast);
		border: 1px solid transparent;
		position: relative;
	}

	.workflow-item:hover {
		background: var(--color-bg-hover);
	}

	.workflow-item:focus-visible {
		outline: none;
		box-shadow: 0 0 0 3px var(--color-accent-light);
	}

	.workflow-item.active {
		background: var(--color-accent-light);
		border-color: var(--color-accent);
	}

	.workflow-name {
		flex: 1;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.workflow-item.active .workflow-name {
		color: var(--color-accent);
	}

	.workflow-name-input {
		flex: 1;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-accent);
		border-radius: var(--border-radius-sm);
		padding: var(--spacing-xs) var(--spacing-sm);
		outline: none;
	}

	.workflow-delete {
		opacity: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-xs);
		background: transparent;
		border: none;
		border-radius: var(--border-radius-sm);
		color: var(--color-text-tertiary);
		cursor: pointer;
		transition: all var(--transition-fast);
	}

	.workflow-item:hover .workflow-delete {
		opacity: 1;
	}

	.workflow-delete:hover {
		background: var(--color-error-light);
		color: var(--color-error);
	}
</style>
