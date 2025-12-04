<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

WorkflowSidebar Component - Phase C Component Extraction
Left sidebar for workflow management with search and CRUD operations.
-->

<script lang="ts">
	import { Plus, Search } from 'lucide-svelte';
	import { Button } from '$lib/components/ui';
	import Sidebar from '$lib/components/layout/Sidebar.svelte';
	import WorkflowList from '$lib/components/workflow/WorkflowList.svelte';
	import { i18n } from '$lib/i18n';
	import type { Workflow } from '$types/workflow';

	interface Props {
		collapsed?: boolean;
		workflows: Workflow[];
		selectedWorkflowId: string | null;
		searchFilter?: string;
		onsearchchange?: (value: string) => void;
		onselect: (workflow: Workflow) => void;
		oncreate: () => void;
		ondelete: (workflow: Workflow) => void;
		onrename?: (workflow: Workflow, newName: string) => void;
	}

	let {
		collapsed = $bindable(false),
		workflows,
		selectedWorkflowId,
		searchFilter = $bindable(''),
		onsearchchange,
		onselect,
		oncreate,
		ondelete,
		onrename
	}: Props = $props();

	function handleSearchInput(e: Event) {
		const target = e.target as HTMLInputElement;
		searchFilter = target.value;
		onsearchchange?.(target.value);
	}

	// Filter workflows locally for display
	const filteredWorkflows = $derived.by(() => {
		if (!searchFilter.trim()) return workflows;
		const filter = searchFilter.toLowerCase();
		return workflows.filter((w) => w.name.toLowerCase().includes(filter));
	});
</script>

<Sidebar bind:collapsed={collapsed}>
	{#snippet header(isCollapsed)}
		<div class="sidebar-header-content" class:collapsed={isCollapsed}>
			{#if isCollapsed}
				<Button
					variant="primary"
					size="icon"
					onclick={oncreate}
					ariaLabel={$i18n('workflow_new')}
					title={$i18n('workflow_new')}
				>
					<Plus size={16} />
				</Button>
			{:else}
				<div class="flex justify-between items-center">
					<h2 class="sidebar-title">{$i18n('workflow_title')}</h2>
					<Button variant="primary" size="icon" onclick={oncreate} ariaLabel={$i18n('workflow_new')}>
						<Plus size={14} />
					</Button>
				</div>
				<div class="search-input-wrapper">
					<span class="search-icon-container">
						<Search size={16} />
					</span>
					<input
						type="search"
						class="search-input"
						placeholder={$i18n('workflow_filter_placeholder')}
						value={searchFilter}
						oninput={handleSearchInput}
					/>
				</div>
			{/if}
		</div>
	{/snippet}

	{#snippet nav(isCollapsed)}
		<WorkflowList
			workflows={filteredWorkflows}
			selectedId={selectedWorkflowId ?? undefined}
			collapsed={isCollapsed}
			{onselect}
			{ondelete}
			{onrename}
		/>
	{/snippet}
</Sidebar>

<style>
	.sidebar-header-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		transition: all var(--transition-fast);
	}

	.sidebar-header-content.collapsed {
		align-items: center;
		justify-content: center;
		gap: 0;
	}

	.sidebar-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.search-input-wrapper {
		position: relative;
		display: flex;
		align-items: center;
	}

	.search-icon-container {
		position: absolute;
		left: var(--spacing-sm);
		top: 50%;
		transform: translateY(-50%);
		color: var(--color-text-tertiary);
		pointer-events: none;
		z-index: 1;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.search-input {
		width: 100%;
		padding: var(--spacing-sm) var(--spacing-md);
		padding-left: calc(var(--spacing-sm) + 16px + var(--spacing-sm));
		font-size: var(--font-size-sm);
		font-family: var(--font-family);
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		transition:
			border-color var(--transition-fast),
			box-shadow var(--transition-fast);
	}

	.search-input:focus {
		outline: none;
		border-color: var(--color-accent);
		box-shadow: 0 0 0 3px var(--color-accent-light);
	}

	.search-input::placeholder {
		color: var(--color-text-tertiary);
	}

	/* Remove default search input styling */
	.search-input::-webkit-search-cancel-button {
		-webkit-appearance: none;
		appearance: none;
		height: 14px;
		width: 14px;
		background: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%236c757d' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cline x1='18' y1='6' x2='6' y2='18'%3E%3C/line%3E%3Cline x1='6' y1='6' x2='18' y2='18'%3E%3C/line%3E%3C/svg%3E")
			center/contain no-repeat;
		cursor: pointer;
	}

	/* Utility Classes */
	.flex {
		display: flex;
	}

	.justify-between {
		justify-content: space-between;
	}

	.items-center {
		align-items: center;
	}
</style>
