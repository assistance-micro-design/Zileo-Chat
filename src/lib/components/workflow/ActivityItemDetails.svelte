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
  ActivityItemDetails Component
  Displays expandable task details section within an ActivityItem.
  Shows description, priority, and assigned agent information.

  OPT-MSG-6: Extracted from ActivityItem.svelte for better maintainability.

  @example
  <ActivityItemDetails activity={taskActivity} />
-->
<script lang="ts">
	import type { WorkflowActivityEvent } from '$types/activity';
	import { User, Flag } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * ActivityItemDetails props
	 */
	interface Props {
		/** Activity event with task details to display */
		activity: WorkflowActivityEvent;
	}

	let { activity }: Props = $props();

	/**
	 * Format priority for display
	 */
	function formatPriority(priority: number | undefined): string {
		if (priority === undefined) return '';
		const key = `workflow_activity_priority_${priority}`;
		return $i18n(key) || `${priority}`;
	}
</script>

<div class="task-details" role="region" aria-label="Task details">
	{#if activity.description}
		<div class="task-detail-row">
			<span class="task-detail-label">{$i18n('workflow_activity_description')}</span>
			<span class="task-detail-value">{activity.description}</span>
		</div>
	{/if}
	{#if activity.metadata?.priority}
		<div class="task-detail-row">
			<Flag size={12} class="task-detail-icon" />
			<span class="task-detail-label">{$i18n('workflow_activity_priority')}</span>
			<span class="task-detail-value priority-{activity.metadata.priority}">
				{formatPriority(activity.metadata.priority)}
			</span>
		</div>
	{/if}
	{#if activity.metadata?.agentAssigned}
		<div class="task-detail-row">
			<User size={12} class="task-detail-icon" />
			<span class="task-detail-label">{$i18n('workflow_activity_agent')}</span>
			<span class="task-detail-value">{activity.metadata.agentAssigned}</span>
		</div>
	{/if}
</div>

<style>
	.task-details {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		margin-top: var(--spacing-sm);
		padding: var(--spacing-sm);
		background: var(--color-bg-tertiary);
		border-radius: var(--radius-sm);
		border-left: 2px solid var(--color-accent);
		animation: slideDown 150ms ease-out;
	}

	@keyframes slideDown {
		from {
			opacity: 0;
			transform: translateY(-4px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.task-detail-row {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-xs);
		font-size: var(--font-size-xs);
	}

	.task-detail-row :global(.task-detail-icon) {
		flex-shrink: 0;
		margin-top: 2px;
		color: var(--color-text-tertiary);
	}

	.task-detail-label {
		flex-shrink: 0;
		color: var(--color-text-tertiary);
		min-width: 70px;
	}

	.task-detail-value {
		color: var(--color-text-secondary);
		word-break: break-word;
	}

	/* Priority colors */
	.task-detail-value.priority-1 {
		color: var(--color-error);
		font-weight: 600;
	}

	.task-detail-value.priority-2 {
		color: var(--color-warning);
		font-weight: 500;
	}

	.task-detail-value.priority-3 {
		color: var(--color-text-primary);
	}

	.task-detail-value.priority-4,
	.task-detail-value.priority-5 {
		color: var(--color-text-tertiary);
	}
</style>
