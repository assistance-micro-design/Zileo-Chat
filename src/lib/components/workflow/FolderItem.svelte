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
  FolderItem Component
  Collapsible folder accordion for organizing workflows in the sidebar.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { WorkflowFolder } from '$types/workflow';
	import { ChevronDown, ChevronRight, EllipsisVertical, Pencil, Trash2 } from '@lucide/svelte';
	import type { ContextMenuItem } from '$types/sidebar';
	import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
	import { i18n } from '$lib/i18n';
	import { getWorkflowIdsFromDrag, hasWorkflowDragData } from '$lib/utils/dragDrop';
	import { tick } from 'svelte';

	interface Props {
		/** Folder data */
		folder: WorkflowFolder;
		/** Whether the folder accordion is expanded */
		expanded: boolean;
		/** Number of workflows in this folder */
		workflowCount: number;
		/** Toggle expanded state */
		ontoggle: () => void;
		/** Rename handler */
		onrename?: (folder: WorkflowFolder, name: string) => void;
		/** Delete handler */
		ondelete?: (folder: WorkflowFolder) => void;
		/** Drop handler for workflows dragged into this folder */
		onworkflowdrop?: (workflowIds: string[], folderId: string) => void;
		/** Children snippet (workflow items inside the folder) */
		children: Snippet;
	}

	let {
		folder,
		expanded,
		workflowCount,
		ontoggle,
		onrename,
		ondelete,
		onworkflowdrop,
		children
	}: Props = $props();

	let editing = $state(false);
	let editName = $state('');
	let nameInputRef = $state<HTMLInputElement | null>(null);
	let contextMenuOpen = $state(false);
	let contextMenuX = $state(0);
	let contextMenuY = $state(0);

	/** Whether a valid drag is hovering over this folder */
	let dragOver = $state(false);

	function handleDragOver(event: DragEvent): void {
		if (!onworkflowdrop || !event.dataTransfer || !hasWorkflowDragData(event)) return;
		event.preventDefault();
		event.dataTransfer.dropEffect = 'move';
		dragOver = true;
	}

	function handleDragEnter(event: DragEvent): void {
		if (!onworkflowdrop || !hasWorkflowDragData(event)) return;
		event.preventDefault();
		dragOver = true;
	}

	function handleDragLeave(event: DragEvent): void {
		const related = event.relatedTarget as Node | null;
		const container = event.currentTarget as HTMLElement;
		if (related && container.contains(related)) return;
		dragOver = false;
	}

	function handleDrop(event: DragEvent): void {
		event.preventDefault();
		dragOver = false;
		const ids = getWorkflowIdsFromDrag(event);
		if (ids && ids.length > 0) {
			onworkflowdrop?.(ids, folder.id);
		}
	}

	const contextMenuItems: ContextMenuItem[] = [
		{ id: 'rename', labelKey: 'sidebar_folder_rename', icon: Pencil },
		{
			id: 'delete',
			labelKey: 'sidebar_folder_delete',
			icon: Trash2,
			variant: 'danger',
			separator: true
		}
	];

	function startEdit(event?: MouseEvent): void {
		event?.stopPropagation();
		editing = true;
		editName = folder.name;
		tick().then(() => nameInputRef?.focus());
	}

	function finishEdit(): void {
		editing = false;
		const trimmedName = editName.trim();
		if (trimmedName && trimmedName !== folder.name) {
			onrename?.(folder, trimmedName);
		}
	}

	function handleEditKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter') {
			finishEdit();
		} else if (event.key === 'Escape') {
			editing = false;
			editName = folder.name;
		}
		event.stopPropagation();
	}

	function handleContextMenu(event: MouseEvent): void {
		event.preventDefault();
		event.stopPropagation();
		contextMenuX = event.clientX;
		contextMenuY = event.clientY;
		contextMenuOpen = true;
	}

	function handleMoreClick(event: MouseEvent): void {
		event.stopPropagation();
		const button = event.currentTarget as HTMLElement;
		const rect = button.getBoundingClientRect();
		contextMenuX = rect.right;
		contextMenuY = rect.bottom;
		contextMenuOpen = true;
	}

	function handleContextAction(actionId: string): void {
		contextMenuOpen = false;
		switch (actionId) {
			case 'rename':
				startEdit();
				break;
			case 'delete':
				ondelete?.(folder);
				break;
		}
	}

	function handleToggleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			ontoggle();
		}
	}
</script>

<div
	class="folder-item"
	class:drag-over={dragOver}
	role="group"
	aria-label={folder.name}
	oncontextmenu={handleContextMenu}
	ondragover={handleDragOver}
	ondragenter={handleDragEnter}
	ondragleave={handleDragLeave}
	ondrop={handleDrop}
>
	<div
		class="folder-header"
		role="button"
		tabindex="0"
		onclick={ontoggle}
		onkeydown={handleToggleKeydown}
		aria-expanded={expanded}
	>
		<span class="folder-chevron">
			{#if expanded}
				<ChevronDown size={14} />
			{:else}
				<ChevronRight size={14} />
			{/if}
		</span>
		<span class="folder-color-dot" style="background: {folder.color}"></span>
		{#if editing}
			<input
				bind:this={nameInputRef}
				bind:value={editName}
				type="text"
				class="folder-name-input"
				onblur={finishEdit}
				onkeydown={handleEditKeydown}
				onclick={(e) => e.stopPropagation()}
			/>
		{:else}
			<span class="folder-name">{folder.name}</span>
		{/if}
		<span class="folder-count">({workflowCount})</span>
		<div class="folder-actions">
			<button
				type="button"
				class="action-btn"
				onclick={handleMoreClick}
				aria-label={$i18n('sidebar_context_actions')}
				aria-haspopup="menu"
				aria-expanded={contextMenuOpen}
			>
				<EllipsisVertical size={14} />
			</button>
		</div>
	</div>
	{#if expanded}
		<div class="folder-children" role="group" aria-label={folder.name}>
			{@render children()}
		</div>
	{/if}
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
	.folder-item {
		display: flex;
		flex-direction: column;
	}

	.folder-item.drag-over {
		outline: 2px dashed var(--color-accent);
		outline-offset: -2px;
		border-radius: var(--border-radius-md);
		background: var(--color-accent-light);
	}

	.folder-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		padding: var(--spacing-xs) var(--spacing-md);
		cursor: pointer;
		border-radius: var(--border-radius-md);
		transition: background var(--transition-fast);
		user-select: none;
	}

	.folder-header:hover {
		background: var(--color-bg-hover);
	}

	.folder-header:focus-visible {
		outline: none;
		box-shadow: 0 0 0 3px var(--color-accent-light);
	}

	.folder-chevron {
		display: flex;
		align-items: center;
		color: var(--color-text-tertiary);
		min-width: 14px;
	}

	.folder-color-dot {
		width: 10px;
		height: 10px;
		min-width: 10px;
		border-radius: var(--border-radius-full);
	}

	.folder-name {
		flex: 1;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.folder-name-input {
		flex: 1;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-accent);
		border-radius: var(--border-radius-sm);
		padding: var(--spacing-xs) var(--spacing-sm);
		outline: none;
	}

	.folder-count {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.folder-actions {
		display: flex;
		align-items: center;
		opacity: 0;
		transition: opacity var(--transition-fast);
	}

	.folder-header:hover .folder-actions {
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

	.action-btn:hover {
		background: var(--color-bg-tertiary);
		color: var(--color-text-primary);
	}

	.folder-children {
		padding-left: var(--spacing-lg);
	}
</style>
