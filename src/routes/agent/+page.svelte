<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

Agent Page - Refactored with Design System Components
Uses: Sidebar, WorkflowList, ChatInput, MessageList, MetricsBar, AgentSelector
-->

<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import type { Workflow, WorkflowResult } from '$types/workflow';
	import type { Message } from '$types/message';
	import type { Agent } from '$types/agent';
	import { Sidebar } from '$lib/components/layout';
	import { Button, Input } from '$lib/components/ui';
	import { WorkflowList, MetricsBar, AgentSelector } from '$lib/components/workflow';
	import { MessageList, ChatInput } from '$lib/components/chat';
	import { Plus, Bot, Search } from 'lucide-svelte';

	/** Workflow state */
	let workflows = $state<Workflow[]>([]);
	let selectedWorkflowId = $state<string | null>(null);

	/** Agent state */
	let agents = $state<Agent[]>([
		{
			id: 'simple_agent',
			name: 'Simple Agent',
			lifecycle: 'permanent',
			status: 'available',
			capabilities: ['chat', 'analysis'],
			tools: [],
			mcp_servers: []
		}
	]);
	let selectedAgentId = $state('simple_agent');

	/** Messages state */
	let messages = $state<Message[]>([]);

	/** Input/Output state */
	let result = $state<WorkflowResult | null>(null);
	let loading = $state(false);

	/** UI state */
	let searchFilter = $state('');
	let sidebarCollapsed = $state(false);

	/**
	 * Get filtered workflows based on search
	 */
	const filteredWorkflows = $derived(() => {
		if (!searchFilter.trim()) return workflows;
		const filter = searchFilter.toLowerCase();
		return workflows.filter((w) => w.name.toLowerCase().includes(filter));
	});

	/**
	 * Get the currently selected workflow object
	 */
	const currentWorkflow = $derived(() => {
		return workflows.find((w) => w.id === selectedWorkflowId);
	});

	/**
	 * Loads all workflows from backend
	 */
	async function loadWorkflows(): Promise<void> {
		try {
			workflows = await invoke<Workflow[]>('load_workflows');
		} catch (err) {
			console.error('Failed to load workflows:', err);
		}
	}

	/**
	 * Creates a new workflow with user-provided name
	 */
	async function createWorkflow(): Promise<void> {
		const name = prompt('Workflow name:');
		if (!name) return;

		try {
			const id = await invoke<string>('create_workflow', {
				name,
				agentId: selectedAgentId
			});

			await loadWorkflows();
			selectedWorkflowId = id;
			messages = [];
			result = null;
		} catch (err) {
			alert('Failed to create workflow: ' + err);
		}
	}

	/**
	 * Handles workflow selection
	 */
	function handleWorkflowSelect(workflow: Workflow): void {
		selectedWorkflowId = workflow.id;
		messages = [];
		result = null;
	}

	/**
	 * Handles workflow deletion
	 */
	async function handleWorkflowDelete(workflow: Workflow): Promise<void> {
		if (!confirm(`Delete workflow "${workflow.name}"?`)) return;

		try {
			await invoke('delete_workflow', { workflowId: workflow.id });
			await loadWorkflows();
			if (selectedWorkflowId === workflow.id) {
				selectedWorkflowId = null;
				messages = [];
				result = null;
			}
		} catch (err) {
			alert('Failed to delete workflow: ' + err);
		}
	}

	/**
	 * Handles workflow rename
	 */
	async function handleWorkflowRename(workflow: Workflow, newName: string): Promise<void> {
		try {
			await invoke('rename_workflow', {
				workflowId: workflow.id,
				name: newName
			});
			await loadWorkflows();
		} catch (err) {
			alert('Failed to rename workflow: ' + err);
		}
	}

	/**
	 * Handles agent selection
	 */
	function handleAgentSelect(agentId: string): void {
		selectedAgentId = agentId;
	}

	/**
	 * Handles sending a message
	 */
	async function handleSend(message: string): Promise<void> {
		if (!selectedWorkflowId || !message.trim()) return;

		// Add user message
		const userMessage: Message = {
			id: crypto.randomUUID(),
			workflow_id: selectedWorkflowId,
			role: 'user',
			content: message,
			tokens: 0,
			timestamp: new Date()
		};
		messages = [...messages, userMessage];

		loading = true;
		try {
			result = await invoke<WorkflowResult>('execute_workflow', {
				workflowId: selectedWorkflowId,
				message: message,
				agentId: selectedAgentId
			});

			// Add assistant message from result
			const assistantMessage: Message = {
				id: crypto.randomUUID(),
				workflow_id: selectedWorkflowId,
				role: 'assistant',
				content: result.report,
				tokens: result.metrics.tokens_output,
				timestamp: new Date()
			};
			messages = [...messages, assistantMessage];
		} catch (err) {
			// Add error message
			const errorMessage: Message = {
				id: crypto.randomUUID(),
				workflow_id: selectedWorkflowId,
				role: 'system',
				content: `Error: ${err}`,
				tokens: 0,
				timestamp: new Date()
			};
			messages = [...messages, errorMessage];
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		loadWorkflows();
	});
</script>

<div class="agent-page">
	<!-- Workflow Sidebar -->
	<Sidebar bind:collapsed={sidebarCollapsed}>
		{#snippet header()}
			<div class="sidebar-header-content">
				<div class="flex justify-between items-center">
					<h2 class="sidebar-title">Workflows</h2>
					<Button variant="primary" size="icon" onclick={createWorkflow} ariaLabel="New workflow">
						<Plus size={14} />
					</Button>
				</div>
				{#if !sidebarCollapsed}
					<div class="search-wrapper">
						<Search size={16} class="search-icon" />
						<Input
							type="search"
							placeholder="Filter workflows..."
							bind:value={searchFilter}
						/>
					</div>
				{/if}
			</div>
		{/snippet}

		{#snippet nav()}
			{#if !sidebarCollapsed}
				<WorkflowList
					workflows={filteredWorkflows()}
					selectedId={selectedWorkflowId ?? undefined}
					onselect={handleWorkflowSelect}
					ondelete={handleWorkflowDelete}
					onrename={handleWorkflowRename}
				/>
			{/if}
		{/snippet}
	</Sidebar>

	<!-- Agent Main Area -->
	<main class="agent-main">
		{#if selectedWorkflowId && currentWorkflow()}
			<!-- Agent Header -->
			<div class="agent-header">
				<div class="header-left">
					<Bot size={24} class="agent-icon" />
					<div>
						<h2 class="agent-title">{currentWorkflow()?.name || 'Agent'}</h2>
						<AgentSelector
							{agents}
							selected={selectedAgentId}
							onselect={handleAgentSelect}
							label=""
						/>
					</div>
				</div>
			</div>

			<!-- Messages Area -->
			<div class="messages-container">
				<MessageList {messages} />
			</div>

			<!-- Chat Input -->
			<ChatInput
				disabled={loading}
				{loading}
				onsend={handleSend}
			/>

			<!-- Metrics Bar -->
			{#if result}
				<MetricsBar metrics={result.metrics} />
			{/if}
		{:else}
			<!-- Empty State -->
			<div class="empty-state">
				<Bot size={64} class="empty-icon" />
				<h3>Select or create a workflow</h3>
				<p class="empty-description">Choose an existing workflow from the sidebar or create a new one to get started.</p>
				<Button variant="primary" onclick={createWorkflow}>
					<Plus size={16} />
					New Workflow
				</Button>
			</div>
		{/if}
	</main>
</div>

<style>
	.agent-page {
		display: flex;
		height: 100%;
	}

	/* Sidebar Header Content */
	.sidebar-header-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.sidebar-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
	}

	.search-wrapper {
		position: relative;
	}

	.search-wrapper :global(.search-icon) {
		position: absolute;
		left: var(--spacing-md);
		top: 50%;
		transform: translateY(-50%);
		color: var(--color-text-tertiary);
		pointer-events: none;
		z-index: 1;
	}

	.search-wrapper :global(input) {
		padding-left: calc(var(--spacing-md) * 2 + 16px);
	}

	/* Agent Main Area */
	.agent-main {
		flex: 1;
		display: flex;
		flex-direction: column;
		background: var(--color-bg-primary);
		overflow: hidden;
		min-width: 0;
	}

	.agent-header {
		padding: var(--spacing-lg);
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
	}

	.header-left {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-md);
	}

	.header-left :global(.agent-icon) {
		color: var(--color-accent);
		flex-shrink: 0;
		margin-top: var(--spacing-xs);
	}

	.agent-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin-bottom: var(--spacing-sm);
	}

	/* Messages Container */
	.messages-container {
		flex: 1;
		overflow: hidden;
		display: flex;
		flex-direction: column;
	}

	/* Empty State */
	.empty-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-2xl);
		text-align: center;
	}

	.empty-state :global(.empty-icon) {
		color: var(--color-text-tertiary);
		margin-bottom: var(--spacing-lg);
	}

	.empty-state h3 {
		font-size: var(--font-size-xl);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin-bottom: var(--spacing-sm);
	}

	.empty-description {
		color: var(--color-text-secondary);
		margin-bottom: var(--spacing-lg);
		max-width: 400px;
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
