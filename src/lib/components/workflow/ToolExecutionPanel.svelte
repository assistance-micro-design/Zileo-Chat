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
  ToolExecutionPanel Component
  Displays tool execution history for a workflow.
  Shows both real-time executions (during streaming) and persisted history.

  Phase 3: Tool Execution Persistence

  @example
  <ToolExecutionPanel
    executions={toolExecutions}
    isStreaming={isStreaming}
    activeTools={activeTools}
  />
-->
<script lang="ts">
	import type { ToolExecution, WorkflowToolExecution, ToolExecutionStatus } from '$types/tool';
	import { formatToolDuration, getToolTypeDisplay, getToolIdentifier } from '$types/tool';
	import { Wrench, Clock, CheckCircle2, XCircle, Loader2, ChevronDown, ChevronUp } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * Active tool execution (during streaming)
	 */
	interface ActiveTool {
		name: string;
		status: ToolExecutionStatus;
		startedAt: number;
		duration?: number;
		error?: string;
	}

	/**
	 * ToolExecutionPanel props
	 */
	interface Props {
		/** Persisted tool executions from database */
		executions?: ToolExecution[];
		/** Real-time tool executions from current workflow */
		workflowExecutions?: WorkflowToolExecution[];
		/** Active tools during streaming */
		activeTools?: ActiveTool[];
		/** Whether streaming is active */
		isStreaming?: boolean;
		/** Whether panel is collapsed by default */
		collapsed?: boolean;
	}

	let {
		executions = [],
		workflowExecutions = [],
		activeTools = [],
		isStreaming = false,
		collapsed = true
	}: Props = $props();

	/** Internal expanded state */
	let expanded = $state(!collapsed);

	/**
	 * Toggle panel expansion
	 */
	function toggleExpanded(): void {
		expanded = !expanded;
	}

	/**
	 * Get status CSS class
	 */
	function getStatusClass(status: ToolExecutionStatus): string {
		switch (status) {
			case 'completed':
				return 'status-success';
			case 'error':
				return 'status-error';
			case 'running':
				return 'status-running';
			default:
				return 'status-pending';
		}
	}

	/**
	 * Format success/failure for historical executions
	 */
	function getHistoricalStatus(success: boolean): ToolExecutionStatus {
		return success ? 'completed' : 'error';
	}

	/**
	 * All executions to display (combines active + historical)
	 */
	const displayExecutions = $derived.by(() => {
		const items: Array<{
			id: string;
			name: string;
			type: string;
			serverName?: string;
			status: ToolExecutionStatus;
			duration?: number;
			error?: string;
			iteration: number;
			isActive: boolean;
		}> = [];

		// Add active tools first (during streaming)
		for (const tool of activeTools) {
			items.push({
				id: `active-${tool.name}-${tool.startedAt}`,
				name: tool.name,
				type: 'unknown',
				status: tool.status,
				duration: tool.duration,
				error: tool.error,
				iteration: 0,
				isActive: true
			});
		}

		// Add workflow executions (from current result)
		for (let i = 0; i < workflowExecutions.length; i++) {
			const exec = workflowExecutions[i];
			items.push({
				id: `workflow-${i}`,
				name: getToolIdentifier(exec),
				type: getToolTypeDisplay(exec.tool_type as 'local' | 'mcp'),
				serverName: exec.server_name,
				status: getHistoricalStatus(exec.success),
				duration: exec.duration_ms,
				error: exec.error_message,
				iteration: exec.iteration,
				isActive: false
			});
		}

		// Add persisted executions (from database)
		for (const exec of executions) {
			items.push({
				id: exec.id,
				name: getToolIdentifier(exec),
				type: getToolTypeDisplay(exec.tool_type),
				serverName: exec.server_name,
				status: getHistoricalStatus(exec.success),
				duration: exec.duration_ms,
				error: exec.error_message,
				iteration: exec.iteration,
				isActive: false
			});
		}

		return items;
	});

	/**
	 * Count of total executions
	 */
	const executionCount = $derived(displayExecutions.length);

	/**
	 * Count of successful executions
	 */
	const successCount = $derived(displayExecutions.filter(e => e.status === 'completed').length);

	/**
	 * Count of failed executions
	 */
	const errorCount = $derived(displayExecutions.filter(e => e.status === 'error').length);

	/**
	 * Whether there are any executions to show
	 */
	const hasExecutions = $derived(executionCount > 0 || isStreaming);
</script>

{#if hasExecutions}
	<div class="tool-execution-panel" class:expanded>
		<!-- Header -->
		<button
			class="panel-header"
			onclick={toggleExpanded}
			aria-expanded={expanded}
			aria-controls="tool-execution-list"
		>
			<div class="header-left">
				<Wrench size={16} class="panel-icon" />
				<span class="panel-title">{$i18n('workflow_tools_title')}</span>
				<span class="execution-count">{executionCount}</span>
				{#if successCount > 0}
					<span class="count-badge success">{successCount}</span>
				{/if}
				{#if errorCount > 0}
					<span class="count-badge error">{errorCount}</span>
				{/if}
			</div>
			<div class="header-right">
				{#if isStreaming}
					<Loader2 size={14} class="streaming-indicator" />
				{/if}
				{#if expanded}
					<ChevronUp size={16} />
				{:else}
					<ChevronDown size={16} />
				{/if}
			</div>
		</button>

		<!-- Execution List -->
		{#if expanded}
			<div id="tool-execution-list" class="execution-list" role="list">
				{#each displayExecutions as exec (exec.id)}
					<div
						class="execution-item {getStatusClass(exec.status)}"
						class:active={exec.isActive}
						role="listitem"
					>
						<div class="execution-status">
							{#if exec.status === 'running'}
								<Loader2 size={14} class="spinning" />
							{:else if exec.status === 'completed'}
								<CheckCircle2 size={14} />
							{:else if exec.status === 'error'}
								<XCircle size={14} />
							{:else}
								<Clock size={14} />
							{/if}
						</div>
						<div class="execution-details">
							<div class="execution-name">
								{exec.name}
								{#if exec.type !== 'unknown'}
									<span class="execution-type">{exec.type}</span>
								{/if}
							</div>
							{#if exec.error}
								<div class="execution-error">{exec.error}</div>
							{/if}
						</div>
						{#if exec.duration !== undefined}
							<div class="execution-duration">
								{formatToolDuration(exec.duration)}
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</div>
{/if}

<style>
	.tool-execution-panel {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-md);
		overflow: hidden;
		transition: all var(--transition-base, 200ms) ease-out;
	}

	.tool-execution-panel.expanded {
		box-shadow: var(--shadow-sm);
	}

	.panel-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: var(--spacing-sm) var(--spacing-md);
		background: transparent;
		border: none;
		cursor: pointer;
		font: inherit;
		text-align: left;
		color: var(--color-text-primary);
	}

	.panel-header:hover {
		background: var(--color-bg-tertiary);
	}

	.header-left {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.header-left :global(.panel-icon) {
		color: var(--color-text-tertiary);
	}

	.panel-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
	}

	.execution-count {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		background: var(--color-bg-tertiary);
		padding: 0 var(--spacing-xs);
		border-radius: var(--radius-full);
	}

	.count-badge {
		font-size: var(--font-size-xs);
		padding: 0 var(--spacing-xs);
		border-radius: var(--radius-full);
	}

	.count-badge.success {
		color: var(--color-success);
		background: var(--color-success-bg, rgba(34, 197, 94, 0.1));
	}

	.count-badge.error {
		color: var(--color-error);
		background: var(--color-error-bg, rgba(239, 68, 68, 0.1));
	}

	.header-right {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		color: var(--color-text-tertiary);
	}

	.header-right :global(.streaming-indicator) {
		animation: spin 1s linear infinite;
		color: var(--color-accent);
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	.execution-list {
		max-height: 200px;
		overflow-y: auto;
		border-top: 1px solid var(--color-border);
		animation: slideDown 200ms ease-out;
	}

	@keyframes slideDown {
		from {
			opacity: 0;
			max-height: 0;
		}
		to {
			opacity: 1;
			max-height: 200px;
		}
	}

	.execution-item {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
		padding: var(--spacing-xs) var(--spacing-md);
		border-bottom: 1px solid var(--color-border-light, rgba(0, 0, 0, 0.05));
		animation: fadeInItem 150ms ease-out;
		transition: background-color var(--transition-fast, 150ms) ease;
	}

	@keyframes fadeInItem {
		from {
			opacity: 0;
			transform: translateX(-8px);
		}
		to {
			opacity: 1;
			transform: translateX(0);
		}
	}

	.execution-item:last-child {
		border-bottom: none;
	}

	.execution-item.active {
		background: var(--color-bg-tertiary);
	}

	.execution-status {
		flex-shrink: 0;
		margin-top: 2px;
	}

	.execution-item.status-success .execution-status {
		color: var(--color-success);
	}

	.execution-item.status-error .execution-status {
		color: var(--color-error);
	}

	.execution-item.status-running .execution-status {
		color: var(--color-accent);
	}

	.execution-item.status-pending .execution-status {
		color: var(--color-text-tertiary);
	}

	.execution-item :global(.spinning) {
		animation: spin 1s linear infinite;
	}

	.execution-details {
		flex: 1;
		min-width: 0;
	}

	.execution-name {
		font-size: var(--font-size-sm);
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.execution-type {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		background: var(--color-bg-tertiary);
		padding: 0 var(--spacing-xs);
		border-radius: var(--radius-sm);
		font-family: var(--font-sans);
	}

	.execution-error {
		font-size: var(--font-size-xs);
		color: var(--color-error);
		margin-top: var(--spacing-xs);
	}

	.execution-duration {
		flex-shrink: 0;
		font-size: var(--font-size-xs);
		font-family: var(--font-mono);
		color: var(--color-text-tertiary);
	}
</style>
