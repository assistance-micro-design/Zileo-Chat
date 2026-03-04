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

Agent Page - Simplified and Refactored
Uses extracted components, services, and stores for clean architecture.
-->

<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import type { Message } from '$types/message';
	import type { ModalState } from '$types/services';
	import type { ValidationRequest } from '$types/validation';

	// Component imports
	import {
		AgentHeader,
		WorkflowSidebar,
		ChatContainer
	} from '$lib/components/agent';
	import { TokenDisplay, UserQuestionModal } from '$lib/components/workflow';
	import { Button } from '$lib/components/ui';
	import { MessageSquare, Settings, Bot } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	// Service imports
	import { invoke } from '@tauri-apps/api/core';
	import { WorkflowService, MessageService, BlockService, LocalStorage, STORAGE_KEYS, WorkflowExecutorService } from '$lib/services';
	import type { ChatBlock, SubAgentBlockData, TodoTaskDisplay } from '$types/chat-block';
	import type { SubAgentExecution } from '$types/sub-agent';

	// Store imports
	import {
		workflowStore,
		workflows,
		selectedWorkflow,
		filteredWorkflows,
		workflowSearchFilter,
		workflowsError,
		workflowsLoading
	} from '$lib/stores/workflows';
	import {
		tokenStore,
		tokenDisplayData
	} from '$lib/stores/tokens';
	import { agentStore, agents, isLoading as agentsLoading } from '$lib/stores/agents';
	import { streamingStore } from '$lib/stores/streaming';
	import {
		executionBlocksStore,
		executionBlocks as executionBlocks$,
		isExecuting as isExecuting$,
		spinnerContext as spinnerContext$,
		executionResponse as executionResponse$,
		executionTasks as executionTasks$,
		executionWorkflowId as executionWorkflowId$
	} from '$lib/stores/execution-blocks';
	import { validationStore, pendingValidation } from '$lib/stores/validation';
	import { validationSettingsStore } from '$lib/stores/validation-settings';
	import { userQuestionStore } from '$lib/stores/user-question';
	import {
		backgroundWorkflowsStore,
		runningWorkflowIds as runningWorkflowIds$,
		recentlyCompletedIds as recentlyCompletedIds$,
		questionPendingIds as questionPendingIds$
	} from '$lib/stores/background-workflows';
	import { toastStore, navigationTarget } from '$lib/stores/toast';
	import { fetchModelByApiName } from '$lib/stores/llm';
	import { locale } from '$lib/stores/locale';
	import { getErrorMessage } from '$lib/utils/error';
	import type { ProviderType } from '$types/llm';

	// ============================================================================
	// PageState Interface
	// ============================================================================

	/**
	 * Aggregated page state interface for cleaner state management.
	 * Groups 8 related UI/data variables into single reactive object.
	 */
	/** Task as returned by Rust list_workflow_tasks command (snake_case fields) */
	interface PersistedTask {
		id: string;
		name: string;
		description: string;
		agent_assigned: string | null;
		priority: number;
		status: 'pending' | 'in_progress' | 'completed' | 'blocked';
		duration_ms: number | null;
	}

	interface PageState {
		leftSidebarCollapsed: boolean;
		selectedWorkflowId: string | null;
		selectedAgentId: string | null;
		currentMaxIterations: number;
		currentContextWindow: number;
		messages: Message[];
		messagesLoading: boolean;
	}

	/** Initial page state with localStorage restoration */
	const initialPageState: PageState = {
		leftSidebarCollapsed: false,
		selectedWorkflowId: null,
		selectedAgentId: null,
		currentMaxIterations: 50,
		currentContextWindow: 128000,
		messages: [],
		messagesLoading: false
	};

	// ============================================================================
	// State Variables
	// ============================================================================

	/** Modal state - single union type instead of 3 booleans */
	let modalState = $state<ModalState>({ type: 'none' });

	/** Whether a workflow delete operation is in progress */
	let deletingWorkflow = $state(false);

	/** Aggregated page state */
	let pageState = $state<PageState>(initialPageState);

	/** Persisted blocks per message (SA-019 P3) */
	let messageBlocks = new SvelteMap<string, ChatBlock[]>();

	/** Persisted tasks for the current workflow (SA-019 P6) */
	let persistedTasks = $state<TodoTaskDisplay[]>([]);

	/** Resolved tasks: real-time store during execution of THIS workflow, persisted otherwise.
	 *  Resolves agent UUIDs to display names via $agents store. */
	let resolvedTasks = $derived(
		($isExecuting$ && $executionWorkflowId$ === pageState.selectedWorkflowId
			? $executionTasks$
			: persistedTasks
		).map((t) => ({
			...t,
			agent_name: t.agent_name
				? ($agents.find((a) => a.id === t.agent_name)?.name ?? t.agent_name)
				: undefined
		}))
	);

	// ============================================================================
	// Data Loading Functions (simplified using services)
	// ============================================================================

	/**
	 * Append sub-agent execution blocks to messageBlocks.
	 *
	 * Sub-agent executions are stored in a separate table (sub_agent_execution) and
	 * not loaded by load_message_blocks (which only covers tool_execution/thinking_step).
	 * This function rebuilds SubAgent ChatBlocks from the enriched message.sub_agents
	 * data and the raw execution records, then appends them to messageBlocks.
	 */
	function appendSubAgentBlocks(messages: Message[], executions: SubAgentExecution[]): void {
		for (const message of messages) {
			if (!message.sub_agents || message.sub_agents.length === 0) continue;

			const existingBlocks = messageBlocks.get(message.id) ?? [];
			const maxSequence = existingBlocks.reduce((max, b) => Math.max(max, b.sequence), 0);

			const subAgentBlocks: ChatBlock[] = message.sub_agents.map((sa, i) => {
				const execution = executions.find((e) => e.id === sa.id);
				const data: SubAgentBlockData = {
					agent_name: sa.name,
					status: sa.status,
					duration_ms: sa.duration_ms,
					tokens_input: sa.tokens_input,
					tokens_output: sa.tokens_output,
					report_summary: execution?.result_summary
				};
				return {
					block_type: 'sub_agent' as ChatBlock['block_type'],
					sequence: maxSequence + 1 + i,
					data
				};
			});

			messageBlocks.set(message.id, [...existingBlocks, ...subAgentBlocks]);
		}
	}

	/**
	 * Load workflow data (messages and persisted blocks).
	 */
	async function loadWorkflowData(workflowId: string): Promise<void> {
		pageState.messagesLoading = true;

		try {
			// Load messages
			const result = await MessageService.loadWithSubAgents(workflowId);
			pageState.messages = result.messages;
			if (result.error) {
				toastStore.add({
					type: 'error',
					title: result.error,
					message: '',
					persistent: false,
					duration: 5000
				});
			}

			// Load persisted execution blocks for all messages (SA-019 P3)
			messageBlocks.clear();
			try {
				const blocks = await BlockService.loadForMessages(result.messages);
				for (const [id, b] of blocks) {
					messageBlocks.set(id, b);
				}
			} catch {
				// Already cleared above
			}

			// Rebuild sub-agent blocks from executions (not in tool_execution/thinking_step tables)
			appendSubAgentBlocks(result.messages, result.executions);

			// Load persisted tasks for this workflow (SA-019 P6)
			persistedTasks = [];
			try {
				const tasks = await invoke<PersistedTask[]>('list_workflow_tasks', { workflowId });
				persistedTasks = tasks.map((t) => ({
					id: t.id,
					name: t.name,
					description: t.description,
					status: t.status,
					priority: t.priority,
					agent_name: t.agent_assigned ?? undefined,
					duration_ms: t.duration_ms ?? undefined
				}));
			} catch {
				// Tasks are optional; silently continue if loading fails
			}
		} finally {
			pageState.messagesLoading = false;
		}
	}

	// ============================================================================
	// Workflow Management Functions
	// ============================================================================

	/**
	 * Create a new workflow.
	 */
	async function handleCreateWorkflow(name: string, agentId: string): Promise<void> {
		const id = await WorkflowService.create(name, agentId);

		pageState.selectedWorkflowId = id;
		pageState.messages = [];

		await workflowStore.loadWorkflows();
		await selectWorkflow(id);

		modalState = { type: 'none' };
	}

	/**
	 * Select a workflow and load its data.
	 * Handles workflow switching for background workflows by restoring streaming state.
	 */
	async function selectWorkflow(workflowId: string): Promise<void> {
		pageState.selectedWorkflowId = workflowId;
		workflowStore.select(workflowId);
		LocalStorage.set(STORAGE_KEYS.SELECTED_WORKFLOW_ID, workflowId);

		// Notify background store which workflow is being viewed
		backgroundWorkflowsStore.setViewed(workflowId);

		// Load workflow data (messages and historical activities)
		await loadWorkflowData(workflowId);

		// Check if this workflow is running in the background
		const bgExecution = backgroundWorkflowsStore.getExecution(workflowId);
		if (bgExecution && bgExecution.status === 'running') {
			// Restore streaming state from background execution
			streamingStore.restoreFrom(bgExecution);
			executionBlocksStore.start(workflowId);
			tokenStore.startStreaming();
			tokenStore.setSessionTokens(0, bgExecution.tokensReceived);

			// Open user question modal if there are pending questions for this workflow
			userQuestionStore.openForWorkflow(workflowId);
		} else {
			// Not running in background - reset streaming and execution state
			streamingStore.reset();
			executionBlocksStore.reset();
		}

		// Update token store with workflow cumulative metrics
		const workflow = workflowStore.getSelected();
		if (workflow) {
			tokenStore.updateFromWorkflow(workflow);
		}

		// Auto-select agent if workflow has one
		const agentId = workflow?.agent_id;
		if (agentId && agentId !== pageState.selectedAgentId) {
			await handleAgentChange(agentId);
		}
	}

	/**
	 * Delete a workflow.
	 */
	async function handleDeleteWorkflow(workflowId: string): Promise<void> {
		deletingWorkflow = true;
		try {
			await WorkflowService.delete(workflowId);
			await workflowStore.loadWorkflows();

			// Clear selection if deleted workflow was selected
			if (pageState.selectedWorkflowId === workflowId) {
				pageState.selectedWorkflowId = null;
				pageState.messages = [];
			}

			modalState = { type: 'none' };
		} catch (err) {
			toastStore.add({
				type: 'error',
				title: getErrorMessage(err),
				message: '',
				persistent: false,
				duration: 5000
			});
		} finally {
			deletingWorkflow = false;
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
			toastStore.add({
				type: 'error',
				title: getErrorMessage(err),
				message: '',
				persistent: false,
				duration: 5000
			});
		}
	}

	// ============================================================================
	// Agent Management Functions
	// ============================================================================

	/**
	 * Handle agent selection change.
	 */
	function handleAgentChange(agentId: string): void {
		pageState.selectedAgentId = agentId;
		loadAgentConfig(agentId);
	}

	/**
	 * Load agent configuration (max iterations and model info).
	 * Also loads the full model data to get context_window and pricing.
	 */
	async function loadAgentConfig(agentId: string): Promise<void> {
		try {
			const config = await agentStore.getAgentConfig(agentId);
			pageState.currentMaxIterations = config.max_tool_iterations ?? 50;

			// Load full model data to get context_window and pricing
			if (config.llm?.model && config.llm?.provider) {
				try {
					const model = await fetchModelByApiName(
						config.llm.model,
						config.llm.provider.toLowerCase() as ProviderType
					);
					tokenStore.updateFromModel(model);
					pageState.currentContextWindow = model.context_window;
				} catch {
					pageState.currentContextWindow = 128000;
				}
			} else {
				pageState.currentContextWindow = 128000;
			}
		} catch {
			pageState.currentMaxIterations = 50;
			pageState.currentContextWindow = 128000;
		}
	}

	/**
	 * Handle max iterations change.
	 */
	function handleIterationsChange(value: number): void {
		pageState.currentMaxIterations = value;
	}

	// ============================================================================
	// Message Handling (delegated to WorkflowExecutorService)
	// ============================================================================

	/**
	 * Handle sending a message with streaming.
	 * Delegates orchestration to WorkflowExecutorService.
	 */
	async function handleSend(message: string): Promise<void> {
		if (!pageState.selectedWorkflowId || !pageState.selectedAgentId || !message.trim()) return;

		const result = await WorkflowExecutorService.execute(
			{
				workflowId: pageState.selectedWorkflowId,
				message,
				agentId: pageState.selectedAgentId,
				locale: $locale
			},
			{
				onUserMessage: (msg) => {
					pageState.messages = [...pageState.messages, msg];
				},
				onAssistantMessage: (msg) => {
					pageState.messages = [...pageState.messages, msg];
				},
				onError: (msg) => {
					pageState.messages = [...pageState.messages, msg];
				}
			}
		);

		// Transfer execution blocks to persisted messageBlocks (SA-019 P5)
		// Blocks snapshot is captured in execute() before the store reset.
		// No ID patching needed: createAssistantMessage uses result.message_id directly.
		if (result.success && result.assistantMessageId && result.blocks && result.blocks.length > 0) {
			messageBlocks.set(result.assistantMessageId, result.blocks);
		}

		// Reload persisted tasks from DB after execution completes (SA-019 P6)
		// executionBlocksStore.reset() clears real-time tasks, so resolvedTasks
		// switches to persistedTasks which must be fresh from DB.
		if (pageState.selectedWorkflowId) {
			try {
				const tasks = await invoke<PersistedTask[]>('list_workflow_tasks', {
					workflowId: pageState.selectedWorkflowId
				});
				persistedTasks = tasks.map((t) => ({
					id: t.id,
					name: t.name,
					description: t.description,
					status: t.status,
					priority: t.priority,
					agent_name: t.agent_assigned ?? undefined,
					duration_ms: t.duration_ms ?? undefined
				}));
			} catch {
				// Tasks are optional; silently continue
			}
		}
	}

	/**
	 * Handle canceling streaming workflow.
	 */
	function handleCancel(): void {
		if (pageState.selectedWorkflowId) {
			WorkflowService.cancel(pageState.selectedWorkflowId);
			streamingStore.reset();
			executionBlocksStore.cancel();
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

		// Load validation settings (needed for concurrent workflow limits)
		await validationSettingsStore.loadSettings().catch(() => {});

		// Initialize background workflows store (owns event listeners)
		await backgroundWorkflowsStore.init();
		backgroundWorkflowsStore.setForwardCallbacks(
			(chunk) => {
				streamingStore.processChunkDirect(chunk);
				executionBlocksStore.processChunk(chunk);
			},
			(complete) => {
				streamingStore.processCompleteDirect(complete);
				executionBlocksStore.complete();
			},
			(payload, workflowId, isViewed) => userQuestionStore.handleQuestionForWorkflow(payload, workflowId, isViewed)
		);

		// Restore last selected workflow from localStorage
		const lastWorkflowId = LocalStorage.get(STORAGE_KEYS.SELECTED_WORKFLOW_ID, null);
		if (lastWorkflowId && $workflows.find(w => w.id === lastWorkflowId)) {
			await selectWorkflow(lastWorkflowId);
		}

		// Initialize validation and user question stores
		await validationStore.init();
		userQuestionStore.init();
	});

	/**
	 * Cleanup on component destroy.
	 */
	onDestroy(() => {
		backgroundWorkflowsStore.destroy();
		streamingStore.cleanup();
		validationStore.cleanup();
		userQuestionStore.cleanup();
	});

	/**
	 * React to pending validation requests.
	 * Opens the validation modal when a new validation request arrives.
	 */
	$effect(() => {
		const request = $pendingValidation;
		if (request) {
			modalState = { type: 'validation', request };
		}
	});

	/**
	 * React to toast navigation requests (e.g., "Go to workflow" button).
	 * Navigates to the target workflow and opens any pending UserQuestion modal.
	 */
	$effect(() => {
		const targetId = $navigationTarget;
		if (targetId) {
			toastStore.clearNavigation();
			selectWorkflow(targetId);
		}
	});
</script>

<div class="agent-page">
	<!-- Left Sidebar - Workflows -->
	<WorkflowSidebar
		bind:collapsed={pageState.leftSidebarCollapsed}
		workflows={$filteredWorkflows}
		selectedWorkflowId={pageState.selectedWorkflowId}
		searchFilter={$workflowSearchFilter}
		error={$workflowsError}
		loading={$workflowsLoading}
		runningWorkflowIds={$runningWorkflowIds$}
		recentlyCompletedIds={$recentlyCompletedIds$}
		questionPendingIds={$questionPendingIds$}
		onsearchchange={(v) => workflowStore.setSearchFilter(v)}
		onselect={(w) => selectWorkflow(w.id)}
		oncreate={() => modalState = { type: 'new-workflow' }}
		ondelete={(w) => modalState = { type: 'delete-workflow', workflowId: w.id }}
		onrename={(w, name) => handleRename(w.id, name)}
		onretry={() => workflowStore.loadWorkflows()}
	/>

	<!-- Main Content -->
	<main class="agent-main">
		{#if pageState.selectedWorkflowId && $selectedWorkflow}
			<!-- Agent Header -->
			<AgentHeader
				workflow={$selectedWorkflow}
				agents={$agents}
				selectedAgentId={pageState.selectedAgentId}
				maxIterations={pageState.currentMaxIterations}
				agentsLoading={$agentsLoading}
				messagesLoading={pageState.messagesLoading}
				onagentchange={handleAgentChange}
				oniterationschange={handleIterationsChange}
			/>

			<!-- Chat Container -->
			<ChatContainer
				messages={pageState.messages}
				messagesLoading={pageState.messagesLoading}
				{messageBlocks}
				executionBlocks={$executionBlocks$}
				isExecuting={$isExecuting$}
				spinnerContext={$spinnerContext$}
				executionTasks={resolvedTasks}
				executionResponse={$executionResponse$}
				disabled={!pageState.selectedAgentId}
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

	<!-- Modals (lazy-loaded for bundle optimization) -->
	{#if modalState.type === 'new-workflow'}
		{#await import('$lib/components/workflow/NewWorkflowModal.svelte') then { default: NewWorkflowModal }}
			<NewWorkflowModal
				open={true}
				agents={$agents}
				selectedAgentId={pageState.selectedAgentId}
				oncreate={handleCreateWorkflow}
				onclose={() => modalState = { type: 'none' }}
			/>
		{/await}
	{:else if modalState.type === 'delete-workflow'}
		{@const workflowId = modalState.workflowId}
		{@const workflow = $workflows.find(w => w.id === workflowId)}
		{#await import('$lib/components/ui/DeleteConfirmModal.svelte') then { default: DeleteConfirmModal }}
			<DeleteConfirmModal
				open={true}
				titleKey="workflow_delete_title"
				confirmMessageKey="workflow_delete_confirm"
				itemName={workflow?.name ?? ''}
				warningMessageKey="workflow_delete_warning"
				deleting={deletingWorkflow}
				deletingLabelKey="workflow_deleting"
				onConfirm={() => handleDeleteWorkflow(workflowId)}
				onCancel={() => modalState = { type: 'none' }}
			/>
		{/await}
	{:else if modalState.type === 'validation'}
		{#await import('$lib/components/workflow/ValidationModal.svelte') then { default: ValidationModal }}
			<ValidationModal
				request={modalState.request}
				open={true}
				onapprove={handleApproveValidation}
				onreject={handleRejectValidation}
				onclose={() => modalState = { type: 'none' }}
			/>
		{/await}
	{/if}

	<!-- User Question Modal -->
	<UserQuestionModal />
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
