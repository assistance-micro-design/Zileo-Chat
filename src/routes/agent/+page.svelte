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
	import type { ChatBlock, TodoTaskDisplay } from '$types/chat-block';

	// Store imports
	import {
		workflowStore,
		workflows,
		selectedWorkflow,
		filteredWorkflows,
		workflowSearchFilter,
		workflowsError,
		workflowsLoading,
		statusFilter as statusFilter$,
		statusCounts as statusCounts$
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
	import {
		folderStore,
		folders as folders$,
		expandedFolderIds as expandedFolderIds$
	} from '$lib/stores/folders';
	import { withToastError } from '$lib/utils/async';
	import { getErrorMessage } from '$lib/utils/error';
	import { isUuid } from '$lib/utils/uuid';
	import { ITERATIONS_LIMITS } from '$lib/utils/constants';
	import type { Workflow, WorkflowFolder, PersistedTask } from '$types/workflow';
	import type { ProviderType } from '$types/llm';

	/**
	 * Aggregated page state interface for cleaner state management.
	 * Groups 8 related UI/data variables into single reactive object.
	 */
	interface PageState {
		leftSidebarCollapsed: boolean;
		selectedWorkflowId: string | null;
		selectedAgentId: string | null;
		currentMaxIterations: number;
		currentContextWindow: number;
		messagesLoading: boolean;
	}

	/**
	 * Maps persisted tasks from Rust snake_case format to TodoTaskDisplay.
	 */
	function mapPersistedTasks(tasks: PersistedTask[]): TodoTaskDisplay[] {
		return tasks.map((t) => ({
			id: t.id,
			name: t.name,
			description: t.description,
			status: t.status,
			priority: t.priority,
			agent_name: t.agent_assigned,
			duration_ms: t.duration_ms
		}));
	}

	/** Initial page state with localStorage restoration */
	const initialPageState: PageState = {
		leftSidebarCollapsed: LocalStorage.get(STORAGE_KEYS.LEFT_SIDEBAR_COLLAPSED, false),
		selectedWorkflowId: null,
		selectedAgentId: null,
		currentMaxIterations: ITERATIONS_LIMITS.DEFAULT,
		currentContextWindow: 128000,
		messagesLoading: false
	};

	/** Modal state - single union type instead of 3 booleans */
	let modalState = $state<ModalState>({ type: 'none' });

	/** Whether a workflow delete operation is in progress */
	let deletingWorkflow = $state(false);

	/** Aggregated page state */
	let pageState = $state<PageState>(initialPageState);

	/** Chat messages (read-only, only reassigned — no deep proxy needed) */
	let messages = $state.raw<Message[]>([]);

	/** Persisted blocks per message */
	let messageBlocks = new SvelteMap<string, ChatBlock[]>();

	/** Persisted tasks for the current workflow (read-only, only reassigned) */
	let persistedTasks = $state.raw<TodoTaskDisplay[]>([]);

	/**
	 * Resolves a raw agent identifier (UUID or live name) to a display name.
	 *
	 * - Non-UUID strings are returned as-is (live stream already sends names).
	 * - UUIDs are looked up in the `$agents` store.
	 * - Orphan UUIDs (deleted agents) fall back to a localized "Unknown agent" label.
	 */
	function resolveAgentName(rawName: string | undefined): string | undefined {
		if (!rawName) return undefined;
		if (!isUuid(rawName)) return rawName;
		const found = $agents.find((a) => a.id === rawName);
		if (found) return found.name;
		return $i18n('agent_unknown');
	}

	/** Resolved tasks: real-time store during execution of THIS workflow, persisted otherwise.
	 *  Resolves agent UUIDs to display names with orphan-safe fallback. */
	let resolvedTasks = $derived(
		($isExecuting$ && $executionWorkflowId$ === pageState.selectedWorkflowId
			? $executionTasks$
			: persistedTasks
		).map((t) => ({
			...t,
			agent_name: resolveAgentName(t.agent_name)
		}))
	);

	/**
	 * Load workflow data (messages and persisted blocks).
	 */
	async function loadWorkflowData(workflowId: string): Promise<void> {
		pageState.messagesLoading = true;

		try {
			// Load messages
			const result = await MessageService.loadWithSubAgents(workflowId);
			messages = result.messages;
			if (result.error) {
				toastStore.add({
					type: 'error',
					title: result.error,
					message: '',
					persistent: false,
					duration: 5000
				});
			}

			// Load persisted execution blocks for all messages
			messageBlocks.clear();
			try {
				const blocks = await BlockService.loadForMessages(result.messages);
				for (const [id, b] of blocks) {
					messageBlocks.set(id, b);
				}
			} catch {
				// Already cleared above
			}

			// Load persisted tasks for this workflow
			persistedTasks = [];
			try {
				const tasks = await invoke<PersistedTask[]>('list_workflow_tasks', { workflowId });
				persistedTasks = mapPersistedTasks(tasks);
			} catch {
				// Tasks are optional; silently continue if loading fails
			}
		} finally {
			pageState.messagesLoading = false;
		}
	}

	/**
	 * Create a new workflow.
	 */
	async function handleCreateWorkflow(name: string, agentId: string): Promise<void> {
		const id = await workflowStore.createWorkflow(name, agentId);

		pageState.selectedWorkflowId = id;
		messages = [];

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
			await workflowStore.deleteWorkflow(workflowId);

			if (pageState.selectedWorkflowId === workflowId) {
				pageState.selectedWorkflowId = null;
				messages = [];
			}

			modalState = { type: 'none' };
		} catch (err: unknown) {
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
	 * Batch delete workflows.
	 * Shows a toast if some workflows were skipped due to running status.
	 *
	 * @param ids - Array of workflow IDs to delete
	 * @returns Result with deleted count and skipped running IDs
	 */
	async function handleBatchDelete(ids: string[]): Promise<{ deleted: number; skipped_running: string[] }> {
		const result = await workflowStore.deleteBatch(ids);

		if (result.skipped_running.length > 0) {
			toastStore.add({
				type: 'warning',
				title: $i18n('sidebar_selection_running_skipped', { count: result.skipped_running.length }),
				message: '',
				persistent: false,
				duration: 5000
			});
		}

		// Clear selection if current workflow was deleted
		if (pageState.selectedWorkflowId && ids.includes(pageState.selectedWorkflowId) && !result.skipped_running.includes(pageState.selectedWorkflowId)) {
			pageState.selectedWorkflowId = null;
			messages = [];
		}

		return result;
	}

	/**
	 * Rename a workflow.
	 */
	const handleRename = withToastError(async (workflowId: string, newName: string) => {
		await workflowStore.renameWorkflow(workflowId, newName);
	});

	/**
	 * Create a new folder with a default name and color.
	 */
	const handleCreateFolder = withToastError(async () => {
		const colors = ['#3b82f6', '#ef4444', '#10b981', '#f59e0b', '#8b5cf6', '#ec4899'];
		const color = colors[($folders$).length % colors.length];
		await folderStore.createFolder($i18n('sidebar_folder_create'), color);
	});

	/**
	 * Rename a folder.
	 */
	const handleRenameFolder = withToastError(async (folder: WorkflowFolder, name: string) => {
		await folderStore.renameFolder(folder.id, name);
	});

	/**
	 * Delete a folder (workflows become uncategorized).
	 */
	const handleDeleteFolder = withToastError(async (folder: WorkflowFolder) => {
		await folderStore.deleteFolder(folder.id);
		workflowStore.detachFromFolder(folder.id);
	});

	/**
	 * Toggle pinned state for a workflow.
	 */
	const handleTogglePin = withToastError(async (workflow: Workflow) => {
		await workflowStore.togglePinned(workflow.id);
	});

	/**
	 * Move a workflow to a folder (or remove from folder).
	 */
	const handleMoveToFolder = withToastError(async (workflow: Workflow, folderId: string | null) => {
		await workflowStore.moveToFolder(workflow.id, folderId);
	});

	/**
	 * Move multiple workflows to a folder via drag & drop.
	 */
	const handleWorkflowMove = withToastError(async (workflowIds: string[], folderId: string | null) => {
		if (workflowIds.length === 1) {
			await workflowStore.moveToFolder(workflowIds[0], folderId);
		} else {
			await workflowStore.moveBatchToFolder(workflowIds, folderId);
		}
	});

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
			pageState.currentMaxIterations = config.max_tool_iterations ?? ITERATIONS_LIMITS.DEFAULT;

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
			pageState.currentMaxIterations = ITERATIONS_LIMITS.DEFAULT;
			pageState.currentContextWindow = 128000;
		}
	}

	/**
	 * Handle max iterations change.
	 */
	function handleIterationsChange(value: number): void {
		pageState.currentMaxIterations = value;
	}

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
					messages = [...messages, msg];
				},
				onAssistantMessage: (msg) => {
					messages = [...messages, msg];
				},
				onError: (msg) => {
					messages = [...messages, msg];
				}
			}
		);

		// Transfer execution blocks to persisted messageBlocks
		// Blocks snapshot is captured in execute() before the store reset.
		// No ID patching needed: createAssistantMessage uses result.message_id directly.
		if (result.success && result.assistantMessageId && result.blocks && result.blocks.length > 0) {
			messageBlocks.set(result.assistantMessageId, result.blocks);
		}

		// Reload persisted tasks from DB after execution completes
		// executionBlocksStore.reset() clears real-time tasks, so resolvedTasks
		// switches to persistedTasks which must be fresh from DB.
		if (pageState.selectedWorkflowId) {
			try {
				const tasks = await invoke<PersistedTask[]>('list_workflow_tasks', {
					workflowId: pageState.selectedWorkflowId
				});
				persistedTasks = mapPersistedTasks(tasks);
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

	/**
	 * Initialize component on mount.
	 */
	onMount(async () => {
		// Load workflows, folders, and agents
		await workflowStore.loadWorkflows();
		await folderStore.loadFolders();
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

		// Restore status filter from localStorage
		const savedFilter = LocalStorage.get(STORAGE_KEYS.STATUS_FILTER, 'all');
		if (savedFilter !== 'all') {
			workflowStore.setStatusFilter(savedFilter);
		}

		// Restore last selected workflow from localStorage.
		// If the active status filter would hide it, clear the filter so the
		// restored workflow remains visible in the sidebar.
		const lastWorkflowId = LocalStorage.get(STORAGE_KEYS.SELECTED_WORKFLOW_ID, null);
		if (lastWorkflowId && $workflows.find(w => w.id === lastWorkflowId)) {
			if (!$filteredWorkflows.find(w => w.id === lastWorkflowId)) {
				workflowStore.setStatusFilter('all');
			}
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
	 * Persist sidebar collapsed state to localStorage.
	 */
	$effect(() => {
		LocalStorage.set(STORAGE_KEYS.LEFT_SIDEBAR_COLLAPSED, pageState.leftSidebarCollapsed);
	});

	/**
	 * Persist status filter to localStorage and sync to store.
	 */
	$effect(() => {
		const filter = $statusFilter$;
		LocalStorage.set(STORAGE_KEYS.STATUS_FILTER, filter);
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
		activeStatusFilter={$statusFilter$}
		statusCounts={$statusCounts$}
		folders={$folders$}
		expandedFolderIds={$expandedFolderIds$}
		runningWorkflowIds={$runningWorkflowIds$}
		recentlyCompletedIds={$recentlyCompletedIds$}
		questionPendingIds={$questionPendingIds$}
		onsearchchange={(v) => workflowStore.setSearchFilter(v)}
		onstatusfilterchange={(f) => workflowStore.setStatusFilter(f)}
		onselect={(w) => selectWorkflow(w.id)}
		oncreate={() => modalState = { type: 'new-workflow' }}
		ondelete={(w) => modalState = { type: 'delete-workflow', workflowId: w.id }}
		onrename={(w, name) => handleRename(w.id, name)}
		onretry={() => workflowStore.loadWorkflows()}
		onbatchdelete={handleBatchDelete}
		onfoldertoggle={(id) => folderStore.toggleExpanded(id)}
		onfoldercreate={handleCreateFolder}
		onfolderrename={handleRenameFolder}
		onfolderdelete={handleDeleteFolder}
		ontogglepin={handleTogglePin}
		onmoveto={handleMoveToFolder}
		onworkflowmove={handleWorkflowMove}
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
				messages={messages}
				messagesLoading={pageState.messagesLoading}
				{messageBlocks}
				executionBlocks={$executionBlocks$}
				isExecuting={$isExecuting$}
				spinnerContext={$spinnerContext$}
				executionTasks={resolvedTasks}
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
