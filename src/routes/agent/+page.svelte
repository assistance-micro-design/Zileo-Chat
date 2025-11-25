<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0
-->

<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { Workflow, WorkflowResult } from '$types/workflow';

  /** Workflow state */
  let workflows = $state<Workflow[]>([]);
  let selectedWorkflow = $state<string | null>(null);

  /** Input/Output state */
  let userInput = $state('');
  let result = $state<WorkflowResult | null>(null);
  let loading = $state(false);

  /** UI state */
  let searchFilter = $state('');

  /**
   * Get status class for workflow status indicator
   */
  function getStatusClass(status: string): string {
    switch (status) {
      case 'running':
        return 'status-running';
      case 'completed':
        return 'status-completed';
      case 'error':
        return 'status-error';
      default:
        return 'status-idle';
    }
  }

  /**
   * Get filtered workflows based on search
   */
  function filteredWorkflows(): Workflow[] {
    if (!searchFilter.trim()) return workflows;
    const filter = searchFilter.toLowerCase();
    return workflows.filter((w) => w.name.toLowerCase().includes(filter));
  }

  /**
   * Loads all workflows from backend
   */
  async function loadWorkflows() {
    try {
      workflows = await invoke<Workflow[]>('load_workflows');
    } catch (err) {
      console.error('Failed to load workflows:', err);
    }
  }

  /**
   * Creates a new workflow with user-provided name
   */
  async function createWorkflow() {
    const name = prompt('Workflow name:');
    if (!name) return;

    try {
      const id = await invoke<string>('create_workflow', {
        name,
        agentId: 'simple_agent'
      });

      await loadWorkflows();
      selectedWorkflow = id;
    } catch (err) {
      alert('Failed to create workflow: ' + err);
    }
  }

  /**
   * Executes the selected workflow with user input
   */
  async function executeWorkflow() {
    if (!selectedWorkflow || !userInput.trim()) return;

    loading = true;
    try {
      result = await invoke<WorkflowResult>('execute_workflow', {
        workflowId: selectedWorkflow,
        message: userInput,
        agentId: 'simple_agent'
      });
      userInput = '';
    } catch (err) {
      alert('Execution failed: ' + err);
    } finally {
      loading = false;
    }
  }

  /**
   * Handles keyboard shortcuts in textarea
   */
  function handleKeydown(event: KeyboardEvent) {
    if (event.ctrlKey && event.key === 'Enter') {
      event.preventDefault();
      executeWorkflow();
    }
  }

  /**
   * Get the currently selected workflow object
   */
  function currentWorkflow(): Workflow | undefined {
    return workflows.find((w) => w.id === selectedWorkflow);
  }

  $effect(() => {
    loadWorkflows();
  });
</script>

<div class="agent-page">
  <!-- Workflow Sidebar -->
  <aside class="workflow-sidebar">
    <div class="sidebar-header">
      <div class="flex justify-between items-center">
        <h2 class="sidebar-title">Workflows</h2>
        <button class="btn btn-primary btn-sm btn-icon" onclick={createWorkflow} title="New workflow">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M5 12h14" />
            <path d="M12 5v14" />
          </svg>
        </button>
      </div>
      <div class="search-box mt-md">
        <svg class="search-icon" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8" />
          <path d="m21 21-4.3-4.3" />
        </svg>
        <input
          type="search"
          class="search-input"
          placeholder="Filter workflows..."
          bind:value={searchFilter}
        />
      </div>
    </div>

    <nav class="sidebar-nav">
      {#if filteredWorkflows().length === 0}
        <p class="empty-message">No workflows found</p>
      {:else}
        {#each filteredWorkflows() as workflow}
          <button
            type="button"
            class="workflow-item"
            class:active={selectedWorkflow === workflow.id}
            onclick={() => (selectedWorkflow = workflow.id)}
          >
            <span class="status-indicator {getStatusClass(workflow.status)}"></span>
            <span class="workflow-name">{workflow.name}</span>
          </button>
        {/each}
      {/if}
    </nav>
  </aside>

  <!-- Agent Main Area -->
  <main class="agent-main">
    {#if selectedWorkflow}
      <!-- Agent Header -->
      <div class="agent-header">
        <div class="flex items-center gap-md">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            class="agent-icon"
          >
            <path d="M12 8V4H8" />
            <rect width="16" height="12" x="4" y="8" rx="2" />
            <path d="M2 14h2" />
            <path d="M20 14h2" />
            <path d="M15 13v2" />
            <path d="M9 13v2" />
          </svg>
          <div>
            <h2 class="agent-title">{currentWorkflow()?.name || 'Agent'}</h2>
            <p class="text-sm text-secondary">Simple Agent</p>
          </div>
        </div>
      </div>

      <!-- Input Section -->
      <div class="input-section">
        <textarea
          class="prompt-textarea"
          placeholder="Enter your message..."
          bind:value={userInput}
          disabled={loading}
          onkeydown={handleKeydown}
        ></textarea>
        <div class="flex items-center gap-md mt-md">
          <div class="flex-1"></div>
          <span class="text-sm text-secondary">Ctrl+Enter to send</span>
          <button
            class="btn btn-primary"
            onclick={executeWorkflow}
            disabled={loading || !userInput.trim()}
          >
            {#if loading}
              <div class="spinner"></div>
              Executing...
            {:else}
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path d="m22 2-7 20-4-9-9-4Z" />
                <path d="M22 2 11 13" />
              </svg>
              Send
            {/if}
          </button>
        </div>
      </div>

      <!-- Output Section -->
      <div class="output-section">
        {#if result}
          <!-- User Message -->
          <div class="message user">
            <div class="message-header">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                class="icon-secondary"
              >
                <path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2" />
                <circle cx="12" cy="7" r="4" />
              </svg>
              <span class="text-sm font-medium">You</span>
            </div>
            <div class="message-content user-content">
              Your request was processed
            </div>
          </div>

          <!-- Agent Response -->
          <div class="message agent">
            <div class="message-header">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                class="icon-accent"
              >
                <path d="M12 8V4H8" />
                <rect width="16" height="12" x="4" y="8" rx="2" />
                <path d="M2 14h2" />
                <path d="M20 14h2" />
                <path d="M15 13v2" />
                <path d="M9 13v2" />
              </svg>
              <span class="text-sm font-medium">Agent</span>
            </div>
            <div class="message-content">
              <pre class="report-output">{result.report}</pre>
            </div>
          </div>
        {:else}
          <div class="empty-state">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="48"
              height="48"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
              class="empty-icon"
            >
              <path d="M12 8V4H8" />
              <rect width="16" height="12" x="4" y="8" rx="2" />
              <path d="M2 14h2" />
              <path d="M20 14h2" />
              <path d="M15 13v2" />
              <path d="M9 13v2" />
            </svg>
            <p>Enter a message to start the conversation</p>
          </div>
        {/if}
      </div>

      <!-- Metrics Bar -->
      {#if result}
        <div class="metrics-bar">
          <div class="metric">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              class="icon-secondary"
            >
              <path d="M22 12h-4l-3 9L9 3l-3 9H2" />
            </svg>
            <span class="text-secondary">Duration:</span>
            <span class="font-semibold">{result.metrics.duration_ms}ms</span>
          </div>
          <div class="metric">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              class="icon-accent"
            >
              <path d="M13 2 3 14h9l-1 8 10-12h-9l1-8z" />
            </svg>
            <span class="font-semibold">{result.metrics.provider}</span>
          </div>
          {#if result.metrics.tokens_input || result.metrics.tokens_output}
            <div class="metric">
              <span class="text-secondary">Tokens:</span>
              <span class="font-semibold">{result.metrics.tokens_input + result.metrics.tokens_output}</span>
            </div>
          {/if}
        </div>
      {/if}
    {:else}
      <!-- Empty State -->
      <div class="empty-state-full">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="64"
          height="64"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="empty-icon"
        >
          <path d="M12 8V4H8" />
          <rect width="16" height="12" x="4" y="8" rx="2" />
          <path d="M2 14h2" />
          <path d="M20 14h2" />
          <path d="M15 13v2" />
          <path d="M9 13v2" />
        </svg>
        <h3>Select or create a workflow</h3>
        <p class="text-secondary">Choose an existing workflow from the sidebar or create a new one to get started.</p>
        <button class="btn btn-primary mt-lg" onclick={createWorkflow}>
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M5 12h14" />
            <path d="M12 5v14" />
          </svg>
          New Workflow
        </button>
      </div>
    {/if}
  </main>
</div>

<style>
  .agent-page {
    display: flex;
    height: 100%;
  }

  /* Workflow Sidebar */
  .workflow-sidebar {
    width: 280px;
    background: var(--color-bg-secondary);
    border-right: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .sidebar-title {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
  }

  .mt-md {
    margin-top: var(--spacing-md);
  }

  .mt-lg {
    margin-top: var(--spacing-lg);
  }

  .workflow-item {
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
    padding: var(--spacing-md);
    border-radius: var(--border-radius-md);
    cursor: pointer;
    transition: all var(--transition-fast);
    border: 1px solid transparent;
    background: transparent;
    width: 100%;
    text-align: left;
    font: inherit;
    color: inherit;
    margin-bottom: var(--spacing-xs);
  }

  .workflow-item:hover {
    background: var(--color-bg-hover);
  }

  .workflow-item.active {
    background: var(--color-accent-light);
    border-color: var(--color-accent);
  }

  .workflow-item.active .workflow-name {
    color: var(--color-accent);
  }

  .workflow-name {
    flex: 1;
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
  }

  .empty-message {
    padding: var(--spacing-lg);
    text-align: center;
    color: var(--color-text-tertiary);
    font-size: var(--font-size-sm);
  }

  /* Agent Main Area */
  .agent-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: var(--color-bg-primary);
    overflow: hidden;
  }

  .agent-header {
    padding: var(--spacing-lg);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .agent-icon {
    color: var(--color-accent);
  }

  .agent-title {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
  }

  /* Input Section */
  .input-section {
    padding: var(--spacing-lg);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-primary);
  }

  .prompt-textarea {
    width: 100%;
    min-height: 100px;
    padding: var(--spacing-md);
    font-size: var(--font-size-sm);
    font-family: var(--font-family);
    color: var(--color-text-primary);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--border-radius-md);
    resize: vertical;
    transition: all var(--transition-fast);
  }

  .prompt-textarea:focus {
    outline: none;
    border-color: var(--color-accent);
    box-shadow: 0 0 0 3px var(--color-accent-light);
  }

  .prompt-textarea:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Output Section */
  .output-section {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-lg);
  }

  .message {
    margin-bottom: var(--spacing-lg);
    animation: fadeIn 0.3s ease-in;
  }

  .message-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    margin-bottom: var(--spacing-sm);
  }

  .message-content {
    padding: var(--spacing-md);
    background: var(--color-bg-secondary);
    border-radius: var(--border-radius-md);
    line-height: var(--line-height-relaxed);
  }

  .message.user .message-content {
    background: var(--color-accent-light);
    margin-left: var(--spacing-2xl);
  }

  .message.agent .message-content {
    margin-right: var(--spacing-2xl);
  }

  .report-output {
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    white-space: pre-wrap;
    word-wrap: break-word;
    background: var(--color-bg-tertiary);
    padding: var(--spacing-md);
    border-radius: var(--border-radius-sm);
    overflow-x: auto;
    margin: 0;
  }

  .icon-secondary {
    color: var(--color-text-secondary);
  }

  .icon-accent {
    color: var(--color-accent);
  }

  /* Metrics Bar */
  .metrics-bar {
    padding: var(--spacing-md) var(--spacing-lg);
    background: var(--color-bg-secondary);
    border-top: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    gap: var(--spacing-lg);
    font-size: var(--font-size-sm);
  }

  .metric {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
  }

  /* Empty States */
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: var(--spacing-2xl);
    color: var(--color-text-secondary);
    text-align: center;
  }

  .empty-state-full {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: var(--spacing-2xl);
    color: var(--color-text-secondary);
    text-align: center;
  }

  .empty-icon {
    color: var(--color-text-tertiary);
    margin-bottom: var(--spacing-lg);
  }

  .empty-state-full h3 {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    margin-bottom: var(--spacing-sm);
  }
</style>
