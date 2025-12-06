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
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

Agent Page - Simplified and Refactored (Phase D)
Uses extracted components, services, and stores for clean architecture.
-->

<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import type { Message } from '$types/message';
	import type { ModalState } from '$types/services';
	import type { WorkflowResult } from '$types/workflow';
	import type { ValidationRequest } from '$types/validation';

	// Component imports
	import {
		AgentHeader,
		WorkflowSidebar,
		ActivitySidebar,
		ChatContainer
	} from '$lib/components/agent';
	import { NewWorkflowModal, ConfirmDeleteModal, ValidationModal, TokenDisplay } from '$lib/components/workflow';
	import { Button } from '$lib/components/ui';
	import { MessageSquare, Settings, Bot } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	// Service imports
	import { WorkflowService, MessageService } from '$lib/services';

	// Store imports
	import {
		workflowStore,
		workflows,
		selectedWorkflow,
		filteredWorkflows,
		workflowSearchFilter
	} from '$lib/stores/workflows';
	import {
		activityStore,
		filteredActivities,
		activityFilter
	} from '$lib/stores/activity';
	import {
		tokenStore,
		tokenDisplayData
	} from '$lib/stores/tokens';
	import { agentStore, agents, isLoading as agentsLoading } from '$lib/stores/agents';
	import { streamingStore, isStreaming, streamContent } from '$lib/stores/streaming';
	import { validationStore, hasPendingValidation, pendingValidation } from '$lib/stores/validation';
	import { fetchModelByApiName } from '$lib/stores/llm';
	import { locale } from '$lib/stores/locale';
	import type { ProviderType } from '$types/llm';

	/** LocalStorage key for persisting selected workflow */
	const LAST_WORKFLOW_KEY = 'zileo_last_workflow_id';
	/** LocalStorage key for persisting right sidebar collapsed state */
	const RIGHT_SIDEBAR_COLLAPSED_KEY = 'zileo_right_sidebar_collapsed';

	// ============================================================================
	// State Variables (reduced from 27 to 8)
	// ============================================================================

	/** Modal state - single union type instead of 3 booleans */
	let modalState = $state<ModalState>({ type: 'none' });

	/** UI state */
	let leftSidebarCollapsed = $state(false);
	let rightSidebarCollapsed = $state(
		typeof window !== 'undefined' ? localStorage.getItem(RIGHT_SIDEBAR_COLLAPSED_KEY) === 'true' : false
	);

	/** Selection state (stores handle the rest) */
	let selectedWorkflowId = $state<string | null>(null);
	let selectedAgentId = $state<string | null>(null);

	/** Agent config (loaded on demand) */
	let currentMaxIterations = $state(50);
	let _currentContextWindow = $state(128000);

	/** Messages (local state, could move to store later) */
	let messages = $state<Message[]>([]);
	let messagesLoading = $state(false);

	// ============================================================================
	// Helper Functions
	// ============================================================================

	/**
	 * Create a local user message for immediate UI feedback.
	 */
	function createLocalUserMessage(content: string): Message {
		return {
			id: crypto.randomUUID(),
			workflow_id: selectedWorkflowId!,
			role: 'user',
			content,
			tokens: 0,
			timestamp: new Date()
		};
	}

	/**
	 * Create a local assistant message from workflow result.
	 */
	function createLocalAssistantMessage(result: WorkflowResult): Message {
		return {
			id: crypto.randomUUID(),
			workflow_id: selectedWorkflowId!,
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
	}

	/**
	 * Create a local system message for errors.
	 */
	function createLocalSystemMessage(error: string): Message {
		return {
			id: crypto.randomUUID(),
			workflow_id: selectedWorkflowId!,
			role: 'system',
			content: `Error: ${error}`,
			tokens: 0,
			timestamp: new Date()
		};
	}

	// ============================================================================
	// Data Loading Functions (simplified using services)
	// ============================================================================

	/**
	 * Load workflow data (messages and historical activities).
	 */
	async function loadWorkflowData(workflowId: string): Promise<void> {
		messagesLoading = true;

		try {
			// Load messages
			messages = await MessageService.load(workflowId);

			// Load historical activities (store handles internally)
			await activityStore.loadHistorical(workflowId);
		} finally {
			messagesLoading = false;
		}
	}

	// ============================================================================
	// Workflow Management Functions
	// ============================================================================

	/**
	 * Create a new workflow.
	 */
	async function handleCreateWorkflow(name: string, agentId: string): Promise<void> {
		try {
			const id = await WorkflowService.create(name, agentId);

			// Update selection
			selectedAgentId = agentId;
			selectedWorkflowId = id;
			messages = [];

			// Reload workflows and select the new one
			await workflowStore.loadWorkflows();
			await selectWorkflow(id);

			// Close modal
			modalState = { type: 'none' };
		} catch (err) {
			console.error('Failed to create workflow:', err);
			throw err; // Let modal handle the error
		}
	}

	/**
	 * Select a workflow and load its data.
	 */
	async function selectWorkflow(workflowId: string): Promise<void> {
		selectedWorkflowId = workflowId;
		workflowStore.select(workflowId);
		if (typeof window !== 'undefined') {
			localStorage.setItem(LAST_WORKFLOW_KEY, workflowId);
		}

		// Load workflow data
		await loadWorkflowData(workflowId);

		// Update token store with workflow cumulative metrics
		const workflow = workflowStore.getSelected();
		if (workflow) {
			tokenStore.updateFromWorkflow(workflow);
		}

		// Auto-select agent if workflow has one
		const agentId = workflow?.agent_id;
		if (agentId && agentId !== selectedAgentId) {
			await handleAgentChange(agentId);
		}
	}

	/**
	 * Delete a workflow.
	 */
	async function handleDeleteWorkflow(workflowId: string): Promise<void> {
		try {
			await WorkflowService.delete(workflowId);
			await workflowStore.loadWorkflows();

			// Clear selection if deleted workflow was selected
			if (selectedWorkflowId === workflowId) {
				selectedWorkflowId = null;
				messages = [];
				activityStore.reset();
			}

			modalState = { type: 'none' };
		} catch (err) {
			console.error('Failed to delete workflow:', err);
		}
	}

	/**
	 * Rename a workflow.
	 */
	async function handleRename(workflowId: string, newName: string): Promise<void> {
		try {
			await WorkflowService.rename(workflowId, newName);
			await workflowStore.loadWorkflows();
		} catch (err) {
			console.error('Failed to rename workflow:', err);
			alert('Failed to rename workflow: ' + err);
		}
	}

	// ============================================================================
	// Agent Management Functions
	// ============================================================================

	/**
	 * Handle agent selection change.
	 */
	function handleAgentChange(agentId: string): void {
		selectedAgentId = agentId;
		loadAgentConfig(agentId);
	}

	/**
	 * Load agent configuration (max iterations and model info).
	 * Also loads the full model data to get context_window and pricing.
	 */
	async function loadAgentConfig(agentId: string): Promise<void> {
		try {
			const config = await agentStore.getAgentConfig(agentId);
			currentMaxIterations = config.max_tool_iterations ?? 50;

			// Load full model data to get context_window and pricing
			if (config.llm?.model && config.llm?.provider) {
				try {
					const model = await fetchModelByApiName(
						config.llm.model,
						config.llm.provider.toLowerCase() as ProviderType
					);
					// Update token store with model context window and pricing
					tokenStore.updateFromModel(model);
					_currentContextWindow = model.context_window;
				} catch (modelErr) {
					console.warn('Failed to load model for token metrics, using defaults:', modelErr);
					_currentContextWindow = 128000;
				}
			} else {
				_currentContextWindow = 128000;
			}
		} catch (e) {
			console.error('Failed to load agent config:', e);
			currentMaxIterations = 50;
			_currentContextWindow = 128000;
		}
	}

	/**
	 * Handle max iterations change.
	 */
	function handleIterationsChange(value: number): void {
		currentMaxIterations = value;
	}

	// ============================================================================
	// Message Handling (simplified from 114 lines to ~40 lines)
	// ============================================================================

	/**
	 * Handle sending a message with streaming.
	 */
	async function handleSend(message: string): Promise<void> {
		if (!selectedWorkflowId || !selectedAgentId || !message.trim()) return;

		try {
			// 1. Save user message
			await MessageService.saveUser(selectedWorkflowId, message);
			messages = [...messages, createLocalUserMessage(message)];

			// 2. Start streaming
			tokenStore.startStreaming();
			await streamingStore.start(selectedWorkflowId);

			// 3. Execute workflow with user's selected locale
			const result = await WorkflowService.executeStreaming(
				selectedWorkflowId,
				message,
				selectedAgentId,
				$locale
			);

			// 4. Update streaming tokens and cost with final result
			tokenStore.setInputTokens(result.metrics.tokens_input);
			tokenStore.updateStreamingTokens(result.metrics.tokens_output);
			tokenStore.setSessionCost(result.metrics.cost_usd);

			// 5. Save and display assistant response
			await MessageService.saveAssistant(selectedWorkflowId, result.report, result.metrics);
			messages = [...messages, createLocalAssistantMessage(result)];

			// 6. Capture streaming activities to historical
			activityStore.captureStreamingActivities();

			// 7. Refresh workflows and update cumulative tokens
			await workflowStore.loadWorkflows();
			const workflow = workflowStore.getSelected();
			if (workflow) {
				tokenStore.updateFromWorkflow(workflow);
			}

		} catch (error) {
			const errorMsg = error instanceof Error ? error.message : String(error);
			await MessageService.saveSystem(selectedWorkflowId, `Error: ${errorMsg}`);
			messages = [...messages, createLocalSystemMessage(errorMsg)];

		} finally {
			await streamingStore.reset();
			tokenStore.stopStreaming();
		}
	}

	/**
	 * Handle canceling streaming workflow.
	 */
	function handleCancel(): void {
		if (selectedWorkflowId) {
			WorkflowService.cancel(selectedWorkflowId);
			streamingStore.reset();
			tokenStore.stopStreaming();
		}
	}

	// ============================================================================
	// Validation Handlers
	// ============================================================================

	/**
	 * Handle validation approval.
	 */
	async function handleApproveValidation(_request: ValidationRequest): Promise<void> {
		await validationStore.approve();
		modalState = { type: 'none' };
	}

	/**
	 * Handle validation rejection.
	 */
	async function handleRejectValidation(_request: ValidationRequest, reason?: string): Promise<void> {
		await validationStore.reject(reason);
		modalState = { type: 'none' };
	}

	// ============================================================================
	// Lifecycle Hooks (simplified onMount)
	// ============================================================================

	/**
	 * Initialize component on mount.
	 */
	onMount(async () => {
		// Load workflows and agents
		await workflowStore.loadWorkflows();
		await agentStore.loadAgents();

		// Restore last selected workflow from localStorage
		const lastWorkflowId = localStorage.getItem(LAST_WORKFLOW_KEY);
		if (lastWorkflowId && $workflows.find(w => w.id === lastWorkflowId)) {
			await selectWorkflow(lastWorkflowId);
		}

		// Initialize validation store
		await validationStore.init();
	});

	/**
	 * Cleanup on component destroy.
	 */
	onDestroy(() => {
		streamingStore.cleanup();
		validationStore.cleanup();
	});

	/**
	 * Persist right sidebar state to localStorage.
	 */
	$effect(() => {
		if (typeof window !== 'undefined') {
			localStorage.setItem(RIGHT_SIDEBAR_COLLAPSED_KEY, String(rightSidebarCollapsed));
		}
	});
</script>

<div class="agent-page">
	<!-- Left Sidebar - Workflows -->
	<WorkflowSidebar
		bind:collapsed={leftSidebarCollapsed}
		workflows={$filteredWorkflows}
		{selectedWorkflowId}
		searchFilter={$workflowSearchFilter}
		onsearchchange={(v) => workflowStore.setSearchFilter(v)}
		onselect={(w) => selectWorkflow(w.id)}
		oncreate={() => modalState = { type: 'new-workflow' }}
		ondelete={(w) => modalState = { type: 'delete-workflow', workflowId: w.id }}
		onrename={(w, name) => handleRename(w.id, name)}
	/>

	<!-- Main Content -->
	<main class="agent-main">
		{#if selectedWorkflowId && $selectedWorkflow}
			<!-- Agent Header -->
			<AgentHeader
				workflow={$selectedWorkflow}
				agents={$agents}
				{selectedAgentId}
				maxIterations={currentMaxIterations}
				agentsLoading={$agentsLoading}
				{messagesLoading}
				onagentchange={handleAgentChange}
				oniterationschange={handleIterationsChange}
			/>

			<!-- Chat Container -->
			<ChatContainer
				{messages}
				{messagesLoading}
				streamContent={$streamContent}
				isStreaming={$isStreaming}
				disabled={!selectedAgentId}
				onsend={handleSend}
				oncancel={handleCancel}
			/>

			<!-- Token Display -->
			<div class="token-display">
				<TokenDisplay data={$tokenDisplayData} compact={false} />
			</div>
		{:else}
			<!-- Empty State -->
			<div class="empty-state">
				{#if $agentsLoading}
					<Bot size={64} class="empty-icon" />
					<h3>{$i18n('agent_loading')}</h3>
					<p class="empty-description">{$i18n('agent_loading_description')}</p>
				{:else if $agents.length === 0}
					<Settings size={64} class="empty-icon" />
					<h3>{$i18n('agent_no_agents')}</h3>
					<p class="empty-description">
						{$i18n('agent_no_agents_description')}
					</p>
					<a href="/settings">
						<Button variant="primary">
							<Settings size={16} />
							{$i18n('agent_go_to_settings')}
						</Button>
					</a>
				{:else}
					<MessageSquare size={64} class="empty-icon" />
					<h3>{$i18n('agent_select_or_create')}</h3>
					<p class="empty-description">
						{$i18n('agent_select_description')}
					</p>
					<Button variant="primary" onclick={() => modalState = { type: 'new-workflow' }}>
						{$i18n('agent_new_workflow')}
					</Button>
				{/if}
			</div>
		{/if}
	</main>

	<!-- Right Sidebar - Activity Feed -->
	<ActivitySidebar
		bind:collapsed={rightSidebarCollapsed}
		activities={$filteredActivities}
		isStreaming={$isStreaming}
		filter={$activityFilter}
		onfilterchange={(f) => activityStore.setFilter(f)}
	/>

	<!-- Modals -->
	{#if modalState.type === 'new-workflow'}
		<NewWorkflowModal
			open={true}
			agents={$agents}
			selectedAgentId={selectedAgentId}
			oncreate={handleCreateWorkflow}
			onclose={() => modalState = { type: 'none' }}
		/>
	{:else if modalState.type === 'delete-workflow'}
		{@const workflowId = modalState.workflowId}
		{@const workflow = $workflows.find(w => w.id === workflowId)}
		<ConfirmDeleteModal
			open={true}
			workflowName={workflow?.name ?? ''}
			onconfirm={() => handleDeleteWorkflow(workflowId)}
			oncancel={() => modalState = { type: 'none' }}
		/>
	{:else if modalState.type === 'validation'}
		<ValidationModal
			request={modalState.request}
			open={true}
			onapprove={handleApproveValidation}
			onreject={handleRejectValidation}
			onclose={() => modalState = { type: 'none' }}
		/>
	{/if}

	<!-- Validation Modal (separate from union modals) -->
	{#if $hasPendingValidation && $pendingValidation}
		<ValidationModal
			request={$pendingValidation}
			open={true}
			onapprove={handleApproveValidation}
			onreject={handleRejectValidation}
			onclose={() => modalState = { type: 'none' }}
		/>
	{/if}
</div>

<style>
	/* Essential layout styles only - components handle their own styling */
	.agent-page {
		display: flex;
		flex: 1;
		min-height: 0;
		overflow: hidden;
	}

	.agent-main {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
		overflow: hidden;
	}

	.token-display {
		flex-shrink: 0;
		padding: var(--spacing-xs) var(--spacing-md);
		border-top: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
	}

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

	.empty-state a {
		text-decoration: none;
	}
</style>
