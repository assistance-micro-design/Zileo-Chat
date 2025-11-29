<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

Agent Page - Refactored with Design System Components
Uses: Sidebar, WorkflowList, ChatInput, MessageList, MetricsBar, AgentSelector
Agents are loaded from the centralized agentStore (Phase 4 integration).
Messages are now persisted to SurrealDB (Phase 6 - Message Persistence).
Streaming integration for real-time response display (Phase 2).
-->

<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onDestroy, onMount } from 'svelte';
	import type { Workflow, WorkflowResult, WorkflowFullState } from '$types/workflow';
	import type { Message } from '$types/message';
	import type { AgentSummary, AgentConfig } from '$types/agent';
	import type { ToolExecution, WorkflowToolExecution } from '$types/tool';
	import { Sidebar } from '$lib/components/layout';
	import { Button, Input, Spinner } from '$lib/components/ui';
	import { WorkflowList, MetricsBar, AgentSelector, ToolExecutionPanel, SubAgentActivity } from '$lib/components/workflow';
	import { MessageList, ChatInput, StreamingMessage, MessageListSkeleton } from '$lib/components/chat';
	import { Plus, Bot, Search, Settings, RefreshCw, StopCircle } from 'lucide-svelte';
	import { agentStore, agents as agentsStore, isLoading as agentsLoading } from '$lib/stores/agents';
	import {
		streamingStore,
		isStreaming as isStreamingStore,
		streamContent,
		activeTools,
		reasoningSteps,
		activeSubAgents
	} from '$lib/stores/streaming';
	import {
		validationStore,
		hasPendingValidation,
		pendingValidation
	} from '$lib/stores/validation';
	import type { ValidationRequest } from '$types/validation';
	import { ValidationModal } from '$lib/components/workflow';

	/** LocalStorage key for persisting selected workflow */
	const LAST_WORKFLOW_KEY = 'zileo_last_workflow_id';

	/** Workflow state */
	let workflows = $state<Workflow[]>([]);
	let selectedWorkflowId = $state<string | null>(null);

	/**
	 * Agent state - sourced from centralized agentStore.
	 * The store is loaded on mount and provides reactive updates.
	 */
	let selectedAgentId = $state<string | null>(null);

	/** Current agent's max tool iterations limit (1-200, default: 50) */
	let currentMaxIterations = $state<number>(50);

	/** Messages state - persisted to backend */
	let messages = $state<Message[]>([]);
	let messagesLoading = $state(false);

	/** Tool executions state - persisted to backend */
	let toolExecutions = $state<ToolExecution[]>([]);
	let currentToolExecutions = $state<WorkflowToolExecution[]>([]);

	/** Last completed streaming session - kept for display after streaming ends */
	let lastReasoningSteps = $state<import('$lib/stores/streaming').ActiveReasoningStep[]>([]);
	let lastActiveTools = $state<import('$lib/stores/streaming').ActiveTool[]>([]);

	/** Input/Output state */
	let result = $state<WorkflowResult | null>(null);
	let loading = $state(false);

	/** UI state */
	let searchFilter = $state('');
	let sidebarCollapsed = $state(false);

	/** State recovery indicator (Phase 5: Complete State Recovery) */
	let restoringState = $state(false);
	let restorationError = $state<string | null>(null);

	/**
	 * Reactive agent list from store
	 */
	const agentList = $derived<AgentSummary[]>($agentsStore);

	/**
	 * Reactive loading state from store
	 */
	const agentLoadingState = $derived<boolean>($agentsLoading);

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
	 * Loads all agents from backend using the centralized store.
	 * The store handles caching and state management.
	 */
	async function loadAgents(): Promise<void> {
		await agentStore.loadAgents();

		// Auto-select first agent if none selected and agents are available
		if (!selectedAgentId && agentList.length > 0) {
			selectedAgentId = agentList[0].id;
		}
	}

	/**
	 * Loads messages for the current workflow from backend.
	 * Messages are sorted by timestamp (chronological order).
	 */
	async function loadMessages(workflowId: string): Promise<void> {
		messagesLoading = true;
		try {
			const loadedMessages = await invoke<Message[]>('load_workflow_messages', {
				workflowId
			});
			// Convert timestamp strings to Date objects
			messages = loadedMessages.map(msg => ({
				...msg,
				timestamp: new Date(msg.timestamp)
			}));
		} catch (err) {
			console.error('Failed to load messages:', err);
			messages = [];
		} finally {
			messagesLoading = false;
		}
	}

	/**
	 * Loads tool executions for the current workflow from backend.
	 * Tool executions are sorted by creation time (chronological order).
	 */
	async function loadToolExecutions(workflowId: string): Promise<void> {
		try {
			const loadedExecutions = await invoke<ToolExecution[]>('load_workflow_tool_executions', {
				workflowId
			});
			toolExecutions = loadedExecutions;
		} catch (err) {
			console.error('Failed to load tool executions:', err);
			toolExecutions = [];
		}
	}

	/**
	 * Saves a user message to the backend.
	 *
	 * @param workflowId - The workflow ID
	 * @param content - Message content
	 * @returns The saved message ID
	 */
	async function saveUserMessage(workflowId: string, content: string): Promise<string> {
		return await invoke<string>('save_message', {
			workflowId,
			role: 'user',
			content
		});
	}

	/**
	 * Saves an assistant message with metrics to the backend.
	 *
	 * @param workflowId - The workflow ID
	 * @param content - Message content
	 * @param metrics - Optional metrics from WorkflowResult
	 * @returns The saved message ID
	 */
	async function saveAssistantMessage(
		workflowId: string,
		content: string,
		metrics?: {
			tokens_input: number;
			tokens_output: number;
			model: string;
			provider: string;
			duration_ms: number;
		}
	): Promise<string> {
		return await invoke<string>('save_message', {
			workflowId,
			role: 'assistant',
			content,
			tokensInput: metrics?.tokens_input,
			tokensOutput: metrics?.tokens_output,
			model: metrics?.model,
			provider: metrics?.provider,
			durationMs: metrics?.duration_ms
		});
	}

	/**
	 * Saves a system message (errors, notifications) to the backend.
	 *
	 * @param workflowId - The workflow ID
	 * @param content - Message content
	 * @returns The saved message ID
	 */
	async function saveSystemMessage(workflowId: string, content: string): Promise<string> {
		return await invoke<string>('save_message', {
			workflowId,
			role: 'system',
			content
		});
	}

	/**
	 * Creates a new workflow with user-provided name
	 */
	async function createWorkflow(): Promise<void> {
		if (agentList.length === 0) {
			alert('No agents configured. Please create an agent in Settings first.');
			return;
		}

		if (!selectedAgentId) {
			alert('Please select an agent first.');
			return;
		}

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
	 * Handles workflow selection - loads persisted messages and tool executions
	 */
	async function handleWorkflowSelect(workflow: Workflow): Promise<void> {
		selectedWorkflowId = workflow.id;
		result = null;
		currentToolExecutions = [];
		// Load persisted data from backend in parallel
		await Promise.all([
			loadMessages(workflow.id),
			loadToolExecutions(workflow.id)
		]);
	}

	/**
	 * Handles workflow deletion
	 */
	async function handleWorkflowDelete(workflow: Workflow): Promise<void> {
		if (!confirm(`Delete workflow "${workflow.name}"?`)) return;

		try {
			// Clear associated data first (optional, cascade delete would be better)
			await Promise.all([
				invoke('clear_workflow_messages', { workflowId: workflow.id }),
				invoke('clear_workflow_tool_executions', { workflowId: workflow.id })
			]);
			await invoke('delete_workflow', { id: workflow.id });
			await loadWorkflows();
			if (selectedWorkflowId === workflow.id) {
				selectedWorkflowId = null;
				messages = [];
				toolExecutions = [];
				currentToolExecutions = [];
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
	 * Handles agent selection - loads agent config to get max_tool_iterations
	 */
	async function handleAgentSelect(agentId: string): Promise<void> {
		selectedAgentId = agentId;
		try {
			const config = await invoke<AgentConfig>('get_agent_config', { agentId });
			currentMaxIterations = config.max_tool_iterations;
		} catch (err) {
			console.error('Failed to load agent config:', err);
			currentMaxIterations = 50; // Default fallback
		}
	}

	/**
	 * Handles max tool iterations change
	 */
	async function handleMaxIterationsChange(event: Event): Promise<void> {
		const target = event.target as HTMLInputElement;
		const value = Math.max(1, Math.min(200, parseInt(target.value) || 50));
		currentMaxIterations = value;

		if (!selectedAgentId) return;

		try {
			await invoke('update_agent', {
				agentId: selectedAgentId,
				config: { max_tool_iterations: value }
			});
		} catch (err) {
			console.error('Failed to update max iterations:', err);
		}
	}

	/**
	 * Handles sending a message with streaming - persists to backend
	 */
	async function handleSend(message: string): Promise<void> {
		if (!selectedWorkflowId || !selectedAgentId || !message.trim()) return;

		loading = true;

		try {
			// 1. Save user message to backend
			const userMsgId = await saveUserMessage(selectedWorkflowId, message);

			// 2. Add user message to local state immediately for responsive UI
			const userMessage: Message = {
				id: userMsgId,
				workflow_id: selectedWorkflowId,
				role: 'user',
				content: message,
				tokens: 0,
				timestamp: new Date()
			};
			messages = [...messages, userMessage];

			// 3. Clear previous streaming session data
			lastReasoningSteps = [];
			lastActiveTools = [];

			// 4. Start streaming and setup event listeners
			await streamingStore.start(selectedWorkflowId);

			// 5. Execute workflow with streaming
			result = await invoke<WorkflowResult>('execute_workflow_streaming', {
				workflowId: selectedWorkflowId,
				message: message,
				agentId: selectedAgentId
			});

			// 6. Streaming complete - save assistant message with metrics to backend
			const assistantMsgId = await saveAssistantMessage(
				selectedWorkflowId,
				result.report,
				{
					tokens_input: result.metrics.tokens_input,
					tokens_output: result.metrics.tokens_output,
					model: result.metrics.model,
					provider: result.metrics.provider,
					duration_ms: result.metrics.duration_ms
				}
			);

			// 7. Add assistant message to local state
			const assistantMessage: Message = {
				id: assistantMsgId,
				workflow_id: selectedWorkflowId,
				role: 'assistant',
				content: result.report,
				tokens: result.metrics.tokens_output,
				tokens_input: result.metrics.tokens_input,
				tokens_output: result.metrics.tokens_output,
				model: result.metrics.model,
				provider: result.metrics.provider,
				duration_ms: result.metrics.duration_ms,
				timestamp: new Date()
			};
			messages = [...messages, assistantMessage];

			// 8. Capture tool executions from result for display
			currentToolExecutions = result.tool_executions || [];

			// 9. Capture streaming session data before reset (for persistent display)
			const streamState = streamingStore.getState();
			lastReasoningSteps = [...streamState.reasoning];
			lastActiveTools = [...streamState.tools];

			// 10. Cleanup streaming state
			await streamingStore.reset();
		} catch (err) {
			// Cleanup streaming on error
			await streamingStore.reset();

			// Save error as system message
			const errorContent = `Error: ${err}`;
			try {
				const errorMsgId = await saveSystemMessage(selectedWorkflowId, errorContent);
				const errorMessage: Message = {
					id: errorMsgId,
					workflow_id: selectedWorkflowId,
					role: 'system',
					content: errorContent,
					tokens: 0,
					timestamp: new Date()
				};
				messages = [...messages, errorMessage];
			} catch (saveErr) {
				// Fallback: show error locally if save fails
				console.error('Failed to save error message:', saveErr);
				const errorMessage: Message = {
					id: crypto.randomUUID(),
					workflow_id: selectedWorkflowId,
					role: 'system',
					content: errorContent,
					tokens: 0,
					timestamp: new Date()
				};
				messages = [...messages, errorMessage];
			}
		} finally {
			loading = false;
		}
	}

	/**
	 * Cancels the current streaming workflow.
	 */
	async function handleCancel(): Promise<void> {
		if (!selectedWorkflowId) return;

		try {
			await invoke('cancel_workflow_streaming', { workflowId: selectedWorkflowId });
			streamingStore.cancel();
		} catch (err) {
			console.error('Failed to cancel workflow:', err);
		}
	}

	/**
	 * Cleanup streaming and validation on component destroy
	 */
	onDestroy(async () => {
		await streamingStore.cleanup();
		await validationStore.cleanup();
	});

	/**
	 * Handle validation approval
	 */
	async function handleValidationApprove(_request: ValidationRequest): Promise<void> {
		await validationStore.approve();
	}

	/**
	 * Handle validation rejection
	 */
	async function handleValidationReject(_request: ValidationRequest, reason?: string): Promise<void> {
		await validationStore.reject(reason);
	}

	/**
	 * Handle validation modal close
	 */
	function handleValidationClose(): void {
		// Dismiss without action (will timeout)
		validationStore.dismiss();
	}

	/**
	 * Restores complete workflow state from backend using parallel queries.
	 * Phase 5: Complete State Recovery
	 *
	 * @param workflowId - The workflow ID to restore
	 * @returns true if successful, false otherwise
	 */
	async function restoreWorkflowState(workflowId: string): Promise<boolean> {
		restoringState = true;
		restorationError = null;

		try {
			const fullState = await invoke<WorkflowFullState>('load_workflow_full_state', {
				workflowId
			});

			// Restore all state from the full state object
			selectedWorkflowId = fullState.workflow.id;

			// Convert timestamp strings to Date objects for messages
			messages = fullState.messages.map(msg => ({
				...msg,
				timestamp: new Date(msg.timestamp)
			}));

			// Restore tool executions
			toolExecutions = fullState.tool_executions;

			// Auto-select the agent associated with this workflow
			if (fullState.workflow.agent_id && agentList.length > 0) {
				const agentExists = agentList.some(a => a.id === fullState.workflow.agent_id);
				if (agentExists) {
					selectedAgentId = fullState.workflow.agent_id;
				}
			}

			// Log restoration success for debugging

			return true;
		} catch (err) {
			console.warn('Failed to restore workflow state:', err);
			restorationError = err instanceof Error ? err.message : String(err);

			// Clear invalid localStorage entry
			localStorage.removeItem(LAST_WORKFLOW_KEY);
			return false;
		} finally {
			restoringState = false;
		}
	}

	/**
	 * Initialize component on mount.
	 * Loads workflows, agents, and attempts to restore last selected workflow.
	 * Phase 5: Complete State Recovery
	 * Phase D: Initialize validation store for human-in-the-loop
	 */
	onMount(async () => {
		// Initialize validation store listener
		await validationStore.init();

		// Load workflows and agents in parallel
		await Promise.all([loadWorkflows(), loadAgents()]);

		// Attempt to restore last selected workflow from localStorage
		const lastWorkflowId = localStorage.getItem(LAST_WORKFLOW_KEY);
		if (lastWorkflowId) {
			// Verify the workflow still exists
			const workflowExists = workflows.some(w => w.id === lastWorkflowId);
			if (workflowExists) {
				await restoreWorkflowState(lastWorkflowId);
			} else {
				// Workflow no longer exists, clear localStorage
				localStorage.removeItem(LAST_WORKFLOW_KEY);
			}
		}
	});

	/**
	 * Persist selected workflow ID to localStorage.
	 * Phase 5: Complete State Recovery
	 */
	$effect(() => {
		if (selectedWorkflowId) {
			localStorage.setItem(LAST_WORKFLOW_KEY, selectedWorkflowId);
		}
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
						{#if agentLoadingState}
							<span class="agents-loading">Loading agents...</span>
						{:else if agentList.length === 0}
							<span class="no-agents">No agents configured - <a href="/settings" class="settings-link">Create one in Settings</a></span>
						{:else}
							<AgentSelector
								agents={agentList}
								selected={selectedAgentId ?? agentList[0]?.id ?? ''}
								onselect={handleAgentSelect}
								label=""
							/>
						{/if}
					</div>
				</div>
				<div class="header-right">
					{#if messagesLoading}
						<RefreshCw size={16} class="loading-icon" />
						<span class="loading-text">Loading messages...</span>
					{:else if selectedAgentId && agentList.length > 0}
						<div class="iterations-control">
							<label for="max-iterations" class="iterations-label">Max iterations:</label>
							<input
								type="number"
								id="max-iterations"
								class="iterations-input"
								min="1"
								max="200"
								value={currentMaxIterations}
								onchange={handleMaxIterationsChange}
							/>
						</div>
					{/if}
				</div>
			</div>

			<!-- Messages Area -->
			<div class="messages-container">
				{#if messagesLoading}
					<MessageListSkeleton count={3} />
				{:else}
					<MessageList {messages} />
				{/if}
			</div>

			<!-- Streaming Message (shown during generation and after completion) -->
			{#if $isStreamingStore}
				<div class="streaming-container">
					<StreamingMessage
						content={$streamContent}
						tools={$activeTools}
						reasoning={$reasoningSteps}
						isStreaming={true}
					/>
				</div>
			{:else if lastReasoningSteps.length > 0 || lastActiveTools.length > 0}
				<!-- Collapsed view of last completed streaming session -->
				<div class="streaming-container completed">
					<StreamingMessage
						content=""
						tools={lastActiveTools}
						reasoning={lastReasoningSteps}
						isStreaming={false}
						showTools={true}
						showReasoning={true}
					/>
				</div>
			{/if}

			<!-- Sub-Agent Activity Panel (Phase E: Streaming Events) -->
			{#if $activeSubAgents.length > 0 || $isStreamingStore}
				<div class="sub-agent-container">
					<SubAgentActivity
						subAgents={$activeSubAgents}
						isStreaming={$isStreamingStore}
						collapsed={false}
					/>
				</div>
			{/if}

			<!-- Tool Execution Panel -->
			{#if toolExecutions.length > 0 || currentToolExecutions.length > 0 || $isStreamingStore}
				<div class="tool-execution-container">
					<ToolExecutionPanel
						executions={toolExecutions}
						workflowExecutions={currentToolExecutions}
						activeTools={$activeTools}
						isStreaming={$isStreamingStore}
						collapsed={true}
					/>
				</div>
			{/if}

			<!-- Chat Input with Cancel Button -->
			{#if $isStreamingStore}
				<div class="chat-input-wrapper">
					<ChatInput
						disabled={true}
						loading={true}
						onsend={handleSend}
					/>
					<Button
						variant="danger"
						size="sm"
						onclick={handleCancel}
						ariaLabel="Cancel generation"
					>
						<StopCircle size={16} />
						Cancel
					</Button>
				</div>
			{:else}
				<ChatInput
					disabled={loading}
					{loading}
					onsend={handleSend}
				/>
			{/if}

			<!-- Metrics Bar -->
			{#if result}
				<MetricsBar metrics={result.metrics} />
			{/if}
		{:else}
			<!-- Empty State -->
			<div class="empty-state">
				{#if restoringState}
					<!-- Restoring workflow state (Phase 5) -->
					<Spinner size="lg" />
					<h3>Restoring workflow...</h3>
					<p class="empty-description">
						Loading messages, tool history, and reasoning steps...
					</p>
				{:else if restorationError}
					<!-- Restoration failed -->
					<Bot size={64} class="empty-icon error-icon" />
					<h3>Failed to restore workflow</h3>
					<p class="empty-description error-text">{restorationError}</p>
					<Button variant="secondary" onclick={() => { restorationError = null; }}>
						Dismiss
					</Button>
				{:else if agentLoadingState}
					<!-- Loading agents -->
					<Bot size={64} class="empty-icon" />
					<h3>Loading agents...</h3>
					<p class="empty-description">Please wait while we load your configured agents.</p>
				{:else if agentList.length === 0}
					<!-- No agents configured -->
					<Settings size={64} class="empty-icon" />
					<h3>No agents configured</h3>
					<p class="empty-description">
						You need to create at least one agent before starting a workflow.
						Configure your first agent in the Settings page.
					</p>
					<a href="/settings" class="button-link">
						<Button variant="primary">
							<Settings size={16} />
							Go to Settings
						</Button>
					</a>
				{:else}
					<!-- Ready to create workflow -->
					<Bot size={64} class="empty-icon" />
					<h3>Select or create a workflow</h3>
					<p class="empty-description">Choose an existing workflow from the sidebar or create a new one to get started.</p>
					<Button variant="primary" onclick={createWorkflow}>
						<Plus size={16} />
						New Workflow
					</Button>
				{/if}
			</div>
		{/if}
	</main>

	<!-- Validation Modal for Sub-Agent Operations (Phase D) -->
	<ValidationModal
		request={$pendingValidation}
		open={$hasPendingValidation}
		onapprove={handleValidationApprove}
		onreject={handleValidationReject}
		onclose={handleValidationClose}
	/>
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
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
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

	.header-right {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		color: var(--color-text-tertiary);
		font-size: var(--font-size-sm);
	}

	.header-right :global(.loading-icon) {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	/* Iterations Control */
	.iterations-control {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.iterations-label {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.iterations-input {
		width: 60px;
		padding: var(--spacing-xs) var(--spacing-sm);
		font-size: var(--font-size-sm);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		text-align: center;
	}

	.iterations-input:focus {
		outline: none;
		border-color: var(--color-accent);
	}

	.iterations-input::-webkit-inner-spin-button,
	.iterations-input::-webkit-outer-spin-button {
		opacity: 1;
	}

	.agent-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin-bottom: var(--spacing-sm);
	}

	/* Messages Container */
	.messages-container {
		flex: 1;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
	}

	/* Streaming Container */
	.streaming-container {
		padding: 0 var(--spacing-lg) var(--spacing-md);
		animation: fadeIn 0.3s ease-in;
	}

	/* Completed streaming session - more compact */
	.streaming-container.completed {
		opacity: 0.85;
	}

	.streaming-container.completed :global(.streaming-content) {
		display: none;
	}

	.streaming-container.completed :global(.streaming-header) {
		padding: var(--spacing-xs) var(--spacing-md);
	}

	.streaming-container.completed :global(.streaming-indicator) {
		display: none;
	}

	/* Sub-Agent Activity Panel Container (Phase E) */
	.sub-agent-container {
		padding: 0 var(--spacing-lg) var(--spacing-md);
	}

	/* Tool Execution Panel Container */
	.tool-execution-container {
		padding: 0 var(--spacing-lg) var(--spacing-md);
	}

	/* Chat Input Wrapper (with cancel button) */
	.chat-input-wrapper {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: 0 var(--spacing-md) var(--spacing-md);
	}

	.chat-input-wrapper :global(.chat-input-container) {
		flex: 1;
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

	/* Agent Loading States */
	.agents-loading,
	.no-agents {
		font-size: var(--font-size-sm);
		color: var(--color-text-tertiary);
	}

	/* State Recovery Error Styles (Phase 5) */
	.empty-state :global(.error-icon) {
		color: var(--color-error);
	}

	.error-text {
		color: var(--color-error);
	}

	.settings-link {
		color: var(--color-accent);
		text-decoration: underline;
	}

	.settings-link:hover {
		color: var(--color-accent-hover);
	}

	.button-link {
		text-decoration: none;
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
