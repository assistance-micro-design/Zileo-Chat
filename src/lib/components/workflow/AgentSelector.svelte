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
  AgentSelector Component
  A dropdown selector for choosing an agent.
  Displays agent name with status indicator and model info.
  Supports both Agent and AgentSummary types for flexibility.

  @example
  <AgentSelector agents={availableAgents} selected={currentAgentId} onselect={handleAgentSelect} />
-->
<script lang="ts">
	import type { AgentSummary } from '$types/agent';
	import Select from '$lib/components/ui/Select.svelte';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import { Bot } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * AgentSelector props
	 */
	interface Props {
		/** Array of available agents */
		agents: AgentSummary[];
		/** ID of the currently selected agent */
		selected?: string;
		/** Selection handler */
		onselect?: (agentId: string) => void;
		/** Whether the selector is disabled */
		disabled?: boolean;
		/** Label for the selector */
		label?: string;
	}

	let {
		agents,
		selected = $bindable(''),
		onselect,
		disabled = false,
		label = 'Agent'
	}: Props = $props();

	/**
	 * Convert agents to select options
	 */
	const options = $derived<SelectOption[]>(
		agents.map((agent) => ({
			value: agent.id,
			label: agent.name
		}))
	);

	/**
	 * Get the currently selected agent
	 */
	const selectedAgent = $derived(agents.find((a) => a.id === selected));

	/**
	 * Get model info for display
	 */
	const modelInfo = $derived.by(() => {
		if (!selectedAgent) return null;
		return `${selectedAgent.provider} / ${selectedAgent.model}`;
	});

	/**
	 * Get tools count for display
	 */
	const toolsInfo = $derived.by(() => {
		if (!selectedAgent) return null;
		const tools = selectedAgent.tools_count;
		const mcp = selectedAgent.mcp_servers_count;
		const parts: string[] = [];
		if (tools > 0) {
			const toolLabel = tools > 1 ? $i18n('workflow_agent_tools') : $i18n('workflow_agent_tool');
			parts.push(`${tools} ${toolLabel}`);
		}
		if (mcp > 0) parts.push(`${mcp} MCP`);
		return parts.length > 0 ? parts.join(', ') : null;
	});

	/**
	 * Handle selection change
	 */
	function handleChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		const value = event.currentTarget.value;
		selected = value;
		onselect?.(value);
	}
</script>

<div class="agent-selector">
	<div class="selector-header">
		<Bot size={16} />
		<span class="selector-label">{label}</span>
	</div>
	<Select
		{options}
		value={selected}
		{disabled}
		placeholder={$i18n('workflow_agent_select')}
		onchange={handleChange}
	/>
	{#if selectedAgent}
		<div class="agent-info">
			<span class="agent-lifecycle">{selectedAgent.lifecycle}</span>
			{#if modelInfo}
				<span class="agent-model">{modelInfo}</span>
			{/if}
			{#if toolsInfo}
				<span class="agent-capabilities">{toolsInfo}</span>
			{/if}
		</div>
	{/if}
</div>

<style>
	.agent-selector {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.selector-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		color: var(--color-text-secondary);
	}

	.selector-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.agent-info {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.agent-lifecycle {
		text-transform: capitalize;
		padding: var(--spacing-xs) var(--spacing-sm);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-sm);
	}

	.agent-model {
		font-family: var(--font-mono);
		color: var(--color-text-secondary);
	}

	.agent-capabilities {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
