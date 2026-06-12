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

TodoTasksBlock Component - Displays TodoTool tasks grouped by agent.
Positioned after the ExecutionSpinner in ChatContainer.
Visible during and after execution.
-->

<script lang="ts">
	import { ListTodo, CircleCheckBig, Circle, Loader, Ban } from '@lucide/svelte';
	import type { TodoTaskDisplay } from '$types/chat-block';
	import { i18n } from '$lib/i18n';
	import { formatDuration } from '$lib/utils/duration';

	interface Props {
		/** Tasks to display */
		tasks: TodoTaskDisplay[];
	}

	let { tasks }: Props = $props();

	/**
	 * Group tasks by agent name.
	 * Returns an array of [agentName, tasks] tuples preserving insertion order.
	 * Tasks without agent_name are grouped under 'main'.
	 */
	function groupByAgent(taskList: TodoTaskDisplay[]): [string, TodoTaskDisplay[]][] {
		const order: string[] = [];
		const groups: Record<string, TodoTaskDisplay[]> = {};
		for (const task of taskList) {
			const key = task.agent_name ?? 'main';
			if (groups[key]) {
				groups[key].push(task);
			} else {
				order.push(key);
				groups[key] = [task];
			}
		}
		return order.map((key): [string, TodoTaskDisplay[]] => [key, groups[key] ?? []]);
	}

	let grouped = $derived(groupByAgent(tasks));
	let hasMultipleAgents = $derived(grouped.length > 1);

	let completedCount = $derived(tasks.filter((t) => t.status === 'completed').length);
	/** Completion ratio (0-100) driving the conic-gradient progress ring. */
	let completionPercent = $derived(
		tasks.length > 0 ? Math.round((completedCount / tasks.length) * 100) : 0
	);

	/**
	 * Get CSS class for task status.
	 */
	function statusClass(status: TodoTaskDisplay['status']): string {
		switch (status) {
			case 'completed':
				return 'status-completed';
			case 'in_progress':
				return 'status-in-progress';
			case 'blocked':
				return 'status-blocked';
			default:
				return 'status-pending';
		}
	}

	/**
	 * Get display label for priority.
	 */
	function priorityLabel(priority: number): string {
		return `P${priority}`;
	}
</script>

{#if tasks.length > 0}
	<div class="todo-tasks-block" role="region" aria-label={$i18n('chat_tasks_arialabel')}>
		<div class="tasks-header">
			<span class="progress-ring" style="--p: {completionPercent}" aria-hidden="true"></span>
			<ListTodo size={14} class="tasks-icon" />
			<span class="tasks-title">{$i18n('chat_tasks_title')}</span>
			<span class="tasks-count">{completedCount}/{tasks.length}</span>
		</div>

		<div class="tasks-body">
			{#each grouped as [agentName, agentTasks] (agentName)}
				{#if hasMultipleAgents}
					<div class="agent-group-header">
						<span class="agent-group-name">{agentName}</span>
					</div>
				{/if}

				<ul class="task-list">
					{#each agentTasks as task (task.id)}
						<li class="task-item {statusClass(task.status)}">
							<span class="task-status-icon">
								{#if task.status === 'completed'}
									<CircleCheckBig size={14} />
								{:else if task.status === 'in_progress'}
									<Loader size={14} class="spinning" />
								{:else if task.status === 'blocked'}
									<Ban size={14} />
								{:else}
									<Circle size={14} />
								{/if}
							</span>

							<span class="task-name">{task.name}</span>

							<span class="task-meta">
								{#if task.priority <= 2}
									<span class="task-priority priority-high">{priorityLabel(task.priority)}</span>
								{/if}
								{#if task.duration_ms}
									<span class="task-duration">{formatDuration(task.duration_ms)}</span>
								{/if}
							</span>
						</li>
					{/each}
				</ul>
			{/each}
		</div>
	</div>
{/if}

<style>
	/* The tasks channel (brand turquoise) is published on the block root so
	   the execution-thread rail can tint its node when threaded. */
	.todo-tasks-block {
		--blk-channel: var(--channel-tasks);
		--blk-channel-soft: var(--channel-tasks-soft);
		background: var(--surface-1);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		box-shadow: var(--shadow-xs);
		overflow: hidden;
		animation: fadeIn var(--transition-base);
	}

	.tasks-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		padding: var(--spacing-xs) var(--spacing-sm);
		background: var(--blk-channel-soft);
		border-bottom: 1px solid var(--color-border-light);
	}

	/* Conic completion ring; the textual count beside it doubles as the
	   fallback should conic-gradient misbehave on WebKitGTK. */
	.progress-ring {
		--p: 0;
		width: 22px;
		height: 22px;
		border-radius: 50%;
		background: conic-gradient(var(--blk-channel) calc(var(--p) * 1%), var(--color-bg-active) 0);
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.progress-ring::after {
		content: '';
		width: 15px;
		height: 15px;
		border-radius: 50%;
		background: var(--surface-1);
	}

	.tasks-header :global(.tasks-icon) {
		color: var(--blk-channel);
		flex-shrink: 0;
	}

	.tasks-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
	}

	.tasks-count {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin-left: auto;
	}

	.tasks-body {
		padding: var(--spacing-xs) 0;
	}

	.agent-group-header {
		padding: var(--spacing-xs) var(--spacing-sm) 2px;
	}

	.agent-group-name {
		font-size: var(--font-size-xs);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}

	.task-list {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.task-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		padding: 3px var(--spacing-sm);
		font-size: var(--font-size-sm);
		transition: background-color 0.1s ease;
	}

	.task-item:hover {
		background: var(--color-bg-hover);
	}

	.task-status-icon {
		flex-shrink: 0;
		display: flex;
		align-items: center;
	}

	.status-completed .task-status-icon {
		color: var(--color-success);
	}

	.status-in-progress .task-status-icon {
		color: var(--color-accent-deep);
	}

	.status-blocked .task-status-icon {
		color: var(--color-danger);
	}

	.status-pending .task-status-icon {
		color: var(--color-text-tertiary);
	}

	.task-name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--color-text-primary);
	}

	.status-completed .task-name {
		color: var(--color-text-tertiary);
		text-decoration: line-through;
	}

	.task-meta {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		flex-shrink: 0;
	}

	.task-priority {
		font-size: var(--font-size-xs);
		font-weight: var(--font-weight-medium);
		padding: 0 4px;
		border-radius: var(--border-radius-sm);
	}

	.priority-high {
		background: var(--color-secondary-light);
		color: var(--color-secondary-deep);
		font-weight: var(--font-weight-semibold);
	}

	.task-duration {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.task-status-icon :global(.spinning) {
		animation: spin 1.5s linear infinite;
	}

	@media (prefers-reduced-motion: reduce) {
		.todo-tasks-block {
			animation: none;
		}

		.task-status-icon :global(.spinning) {
			animation: none;
		}
	}
</style>
