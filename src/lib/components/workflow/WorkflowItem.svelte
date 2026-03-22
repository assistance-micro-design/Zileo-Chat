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
	import type { Workflow, WorkflowFolder } from '$types/workflow';
	import type { ContextMenuItem } from '$types/sidebar';
	import StatusIndicator from '$lib/components/ui/StatusIndicator.svelte';
	import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
	import { EllipsisVertical, Pencil, Pin, PinOff, FolderInput, Trash2 } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { WORKFLOW_DRAG_TYPE } from '$lib/utils/dragDrop';
	import { tick } from 'svelte';

	/**
	 * WorkflowItem props
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
		/** Whether multi-selection mode is active */
		selectionMode?: boolean;
		/** Whether this item is currently selected (multi-select) */
		selected?: boolean;
		/** Selection handler */
		onselect?: (workflow: Workflow) => void;
		/** Delete handler */
		ondelete?: (workflow: Workflow) => void;
		/** Rename handler */
		onrename?: (workflow: Workflow, newName: string) => void;
		/** Multi-selection toggle handler (Ctrl+Click or checkbox) */
		onselectiontoggle?: (workflowId: string, event: MouseEvent | KeyboardEvent) => void;
		/** Pin toggle handler */
		ontogglepin?: (workflow: Workflow) => void;
		/** Move to folder handler */
		onmoveto?: (workflow: Workflow, folderId: string | null) => void;
		/** Available folders for move-to menu */
		folders?: WorkflowFolder[];
		/** Set of selected IDs for multi-drag support */
		selectedIds?: Set<string>;
	}

	let {
		workflow,
		active = false,
		running = false,
		hasQuestion = false,
		selectionMode = false,
		selected = false,
		onselect,
		ondelete,
		onrename,
		onselectiontoggle,
		ontogglepin,
		onmoveto,
		folders = [],
		selectedIds = new Set<string>()
	}: Props = $props();

	let editing = $state(false);
	let editName = $state('');
	let nameInputRef = $state<HTMLInputElement | null>(null);
	let contextMenuOpen = $state(false);
	let contextMenuX = $state(0);
	let contextMenuY = $state(0);

	/** Context menu items for this workflow (dynamic based on state) */
	const contextMenuItems = $derived<ContextMenuItem[]>([
		{ id: 'rename', labelKey: 'sidebar_context_rename', icon: Pencil },
		{
			id: 'pin',
			labelKey: workflow.pinned ? 'sidebar_context_unpin' : 'sidebar_context_pin',
			icon: workflow.pinned ? PinOff : Pin,
			disabled: !ontogglepin,
		},
		...(onmoveto && folders.length > 0
			? folders
				.filter((f) => f.id !== workflow.folder_id)
				.map((f, idx) => ({
					id: `move_to_${f.id}`,
					label: f.name,
					icon: FolderInput,
					separator: idx === 0,
				}))
			: []),
		...(workflow.folder_id && onmoveto ? [{ id: 'remove_from_folder', labelKey: 'sidebar_folder_remove_from', icon: FolderInput }] : []),
		{ id: 'delete', labelKey: 'sidebar_context_delete', icon: Trash2, variant: 'danger' as const, separator: true },
	]);

	// Sync editName with workflow.name when workflow changes (e.g., external rename).
	// Note (M-011): While editing, external renames are intentionally ignored to
	// avoid overwriting user input. The sync resumes when editing ends.
	$effect(() => {
		if (!editing) {
			editName = workflow.name;
		}
	});

	/**
	 * Handle workflow selection or multi-select toggle
	 */
	function handleClick(event: MouseEvent): void {
		if (editing) return;

		if (selectionMode || event.ctrlKey || event.metaKey || event.shiftKey) {
			onselectiontoggle?.(workflow.id, event);
			return;
		}

		onselect?.(workflow);
	}

	/**
	 * Start inline editing
	 */
	function startEdit(event?: MouseEvent): void {
		event?.stopPropagation();
		editing = true;
		editName = workflow.name;
		tick().then(() => nameInputRef?.focus());
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
		// Stop propagation to prevent parent div from intercepting keys (e.g. space)
		event.stopPropagation();
	}

	/**
	 * Handle checkbox click in selection mode.
	 * Uses onclick (not onchange) to preserve MouseEvent with modifier keys
	 * for Shift+Click range selection support.
	 */
	function handleCheckboxClick(event: MouseEvent): void {
		event.stopPropagation();
		onselectiontoggle?.(workflow.id, event);
	}

	/**
	 * Handle keyboard activation
	 */
	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			if (selectionMode) {
				onselectiontoggle?.(workflow.id, event);
			} else {
				onselect?.(workflow);
			}
		} else if (event.key === 'F2') {
			event.preventDefault();
			startEdit();
		}
	}

	/** Whether this item is currently being dragged */
	let dragging = $state(false);

	/**
	 * Handle drag start - set workflow IDs in transfer data.
	 * In multi-select mode with this item selected, drags all selected IDs.
	 */
	function handleDragStart(event: DragEvent): void {
		if (editing || !event.dataTransfer) return;

		dragging = true;

		const ids = selectionMode && selected && selectedIds.size > 0
			? [...selectedIds]
			: [workflow.id];

		event.dataTransfer.effectAllowed = 'move';
		event.dataTransfer.setData(WORKFLOW_DRAG_TYPE, JSON.stringify(ids));
	}

	/**
	 * Handle drag end
	 */
	function handleDragEnd(): void {
		dragging = false;
	}

	/**
	 * Open context menu at position
	 */
	function openContextMenu(x: number, y: number): void {
		contextMenuX = x;
		contextMenuY = y;
		contextMenuOpen = true;
	}

	/**
	 * Handle right-click context menu
	 */
	function handleContextMenu(event: MouseEvent): void {
		event.preventDefault();
		event.stopPropagation();
		openContextMenu(event.clientX, event.clientY);
	}

	/**
	 * Handle ... button click
	 */
	function handleMoreClick(event: MouseEvent): void {
		event.stopPropagation();
		const button = event.currentTarget as HTMLElement;
		const rect = button.getBoundingClientRect();
		openContextMenu(rect.right, rect.bottom);
	}

	/**
	 * Handle context menu action selection
	 */
	function handleContextAction(actionId: string): void {
		contextMenuOpen = false;
		switch (actionId) {
			case 'rename':
				startEdit();
				break;
			case 'pin':
				ontogglepin?.(workflow);
				break;
			case 'remove_from_folder':
				onmoveto?.(workflow, null);
				break;
			case 'delete':
				ondelete?.(workflow);
				break;
			default:
				// Handle folder move actions (prefixed with 'move_to_')
				if (actionId.startsWith('move_to_')) {
					const folderId = actionId.replace('move_to_', '');
					onmoveto?.(workflow, folderId);
				}
				break;
		}
	}
</script>

<div
	class="workflow-item"
	class:active
	class:selected
	class:dragging
	role="button"
	tabindex="0"
	draggable={!editing}
	ondragstart={handleDragStart}
	ondragend={handleDragEnd}
	onclick={handleClick}
	onkeydown={handleKeydown}
	ondblclick={selectionMode ? undefined : startEdit}
	oncontextmenu={handleContextMenu}
	aria-pressed={active}
	aria-haspopup="menu"
	aria-roledescription="draggable item"
	aria-label={`Workflow: ${workflow.name}`}
>
	{#if selectionMode}
		<input
			type="checkbox"
			class="selection-checkbox"
			checked={selected}
			onclick={handleCheckboxClick}
			aria-label={$i18n('sidebar_selection_toggle')}
		/>
	{/if}
	{#if running}
		<span class="running-indicator"></span>
	{/if}
	<StatusIndicator status={workflow.status} size="sm" />
	{#if workflow.pinned}
		<Pin size={12} class="pin-icon" />
	{/if}
	{#if hasQuestion}
		<span class="question-badge"></span>
	{/if}
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
	<div class="item-actions">
		<button
			type="button"
			class="action-btn more-btn"
			onclick={handleMoreClick}
			title={$i18n('sidebar_context_actions')}
			aria-label={$i18n('sidebar_context_actions')}
			aria-haspopup="menu"
			aria-expanded={contextMenuOpen}
		>
			<EllipsisVertical size={14} />
		</button>
	</div>
</div>

{#if contextMenuOpen}
	<ContextMenu
		items={contextMenuItems}
		x={contextMenuX}
		y={contextMenuY}
		onselect={handleContextAction}
		onclose={() => (contextMenuOpen = false)}
	/>
{/if}

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

	.workflow-item.selected {
		background: var(--color-accent-light);
		border-color: var(--color-accent);
		opacity: 0.9;
	}

	.workflow-item.dragging {
		opacity: 0.4;
	}

	.selection-checkbox {
		width: 16px;
		height: 16px;
		min-width: 16px;
		accent-color: var(--color-accent);
		cursor: pointer;
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

	.item-actions {
		display: flex;
		align-items: center;
		gap: 2px;
		opacity: 0;
		transition: opacity var(--transition-fast);
	}

	.workflow-item:hover .item-actions {
		opacity: 1;
	}

	.action-btn {
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

	.more-btn:hover {
		background: var(--color-bg-tertiary);
		color: var(--color-text-primary);
	}

	.running-indicator {
		width: 8px;
		height: 8px;
		min-width: 8px;
		border-radius: var(--border-radius-full);
		background: var(--color-success);
		animation: pulse 2s ease-in-out infinite;
	}

	@keyframes pulse {
		0%, 100% {
			opacity: 1;
		}
		50% {
			opacity: 0.4;
		}
	}

	:global(.pin-icon) {
		color: var(--color-accent);
		min-width: 12px;
	}

	.question-badge {
		position: absolute;
		top: 4px;
		right: 4px;
		width: 6px;
		height: 6px;
		border-radius: var(--border-radius-full);
		background: var(--color-warning);
	}
</style>
