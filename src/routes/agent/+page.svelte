<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0
-->

<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { Workflow, WorkflowResult } from '$types/workflow';

  let workflows = $state<Workflow[]>([]);
  let selectedWorkflow = $state<string | null>(null);
  let userInput = $state('');
  let result = $state<WorkflowResult | null>(null);
  let loading = $state(false);

  async function loadWorkflows() {
    try {
      workflows = await invoke<Workflow[]>('load_workflows');
    } catch (err) {
      console.error('Failed to load workflows:', err);
    }
  }

  async function createWorkflow() {
    const name = prompt('Workflow name:');
    if (!name) return;

    try {
      const id = await invoke<string>('create_workflow', {
        name,
        agentId: 'simple_agent',
      });

      await loadWorkflows();
      selectedWorkflow = id;
    } catch (err) {
      alert('Failed to create workflow: ' + err);
    }
  }

  async function executeWorkflow() {
    if (!selectedWorkflow || !userInput.trim()) return;

    loading = true;
    try {
      result = await invoke<WorkflowResult>('execute_workflow', {
        workflowId: selectedWorkflow,
        message: userInput,
        agentId: 'simple_agent',
      });
      userInput = '';
    } catch (err) {
      alert('Execution failed: ' + err);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    loadWorkflows();
  });
</script>

<div class="agent-page">
  <aside class="workflow-list">
    <h2>Workflows</h2>
    <button onclick={createWorkflow}>+ New Workflow</button>

    <ul>
      {#each workflows as workflow}
        <li class:active={selectedWorkflow === workflow.id}>
          <button
            type="button"
            class="workflow-item"
            onclick={() => (selectedWorkflow = workflow.id)}
          >
            <span class="workflow-name">{workflow.name}</span>
            <span class="status">{workflow.status}</span>
          </button>
        </li>
      {/each}
    </ul>
  </aside>

  <section class="main-area">
    {#if selectedWorkflow}
      <div class="input-area">
        <textarea
          bind:value={userInput}
          placeholder="Enter your message..."
          disabled={loading}
        ></textarea>
        <button onclick={executeWorkflow} disabled={loading || !userInput.trim()}>
          {loading ? 'Executing...' : 'Send'}
        </button>
      </div>

      {#if result}
        <div class="output-area">
          <h3>Result</h3>
          <pre>{result.report}</pre>
          <div class="metrics">
            <span>Duration: {result.metrics.duration_ms}ms</span>
            <span>Provider: {result.metrics.provider}</span>
          </div>
        </div>
      {/if}
    {:else}
      <p class="empty-state">Select or create a workflow</p>
    {/if}
  </section>
</div>

<style>
  .agent-page {
    display: flex;
    height: 100%;
  }

  .workflow-list {
    width: 250px;
    border-right: 1px solid var(--color-border);
    padding: 1rem;
    background: var(--color-bg-secondary);
  }

  .workflow-list h2 {
    margin: 0 0 1rem 0;
    font-size: 1.25rem;
  }

  .workflow-list button {
    width: 100%;
    margin-bottom: 1rem;
    padding: 0.5rem;
    background: var(--color-accent);
    color: white;
    border: none;
    border-radius: 0.375rem;
    cursor: pointer;
    font-size: 0.875rem;
    transition: opacity 0.2s;
  }

  .workflow-list button:hover {
    opacity: 0.9;
  }

  .workflow-list ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .workflow-list li {
    margin-bottom: 0.5rem;
  }

  .workflow-item {
    width: 100%;
    padding: 0.75rem;
    background: transparent;
    border: none;
    border-radius: 0.375rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    cursor: pointer;
    transition: background 0.2s;
    color: inherit;
    font: inherit;
    text-align: left;
  }

  .workflow-item:hover {
    background: var(--color-bg-hover);
  }

  .workflow-list li.active .workflow-item {
    background: var(--color-accent);
    color: white;
  }

  .workflow-name {
    flex: 1;
  }

  .status {
    font-size: 0.75rem;
    opacity: 0.7;
  }

  .main-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 1rem;
  }

  .input-area {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }

  .input-area textarea {
    flex: 1;
    padding: 0.75rem;
    border: 1px solid var(--color-border);
    border-radius: 0.375rem;
    resize: vertical;
    min-height: 100px;
    font-family: inherit;
  }

  .input-area button {
    padding: 0.75rem 1.5rem;
    background: var(--color-accent);
    color: white;
    border: none;
    border-radius: 0.375rem;
    cursor: pointer;
    transition: opacity 0.2s;
  }

  .input-area button:hover:not(:disabled) {
    opacity: 0.9;
  }

  .input-area button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .output-area {
    flex: 1;
    overflow: auto;
  }

  .output-area h3 {
    margin: 0 0 1rem 0;
  }

  .output-area pre {
    background: var(--color-bg-secondary);
    padding: 1rem;
    border-radius: 0.375rem;
    overflow-x: auto;
    white-space: pre-wrap;
    word-wrap: break-word;
  }

  .metrics {
    display: flex;
    gap: 1rem;
    margin-top: 1rem;
    font-size: 0.875rem;
    color: var(--color-text-secondary);
  }

  .empty-state {
    text-align: center;
    color: var(--color-text-secondary);
    padding: 2rem;
  }
</style>
