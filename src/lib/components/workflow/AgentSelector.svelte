<!--
  AgentSelector Component
  A dropdown selector for choosing an agent.
  Displays agent name with status indicator and model info.
  Supports both Agent and AgentSummary types for flexibility.

  @example
  <AgentSelector agents={availableAgents} selected={currentAgentId} onselect={handleAgentSelect} />
-->
<script lang="ts">
	import type { Agent, AgentSummary } from '$types/agent';
	import Select from '$lib/components/ui/Select.svelte';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import StatusIndicator from '$lib/components/ui/StatusIndicator.svelte';
	import { Bot } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * Union type for items that can be displayed in the selector.
	 * Supports both full Agent type and lightweight AgentSummary.
	 */
	type AgentItem = Agent | AgentSummary;

	/**
	 * Type guard to check if item is a full Agent (has status property)
	 */
	function isAgent(item: AgentItem): item is Agent {
		return 'status' in item;
	}

	/**
	 * Type guard to check if item is an AgentSummary (has provider property)
	 */
	function isSummary(item: AgentItem): item is AgentSummary {
		return 'provider' in item && 'tools_count' in item;
	}

	/**
	 * AgentSelector props
	 */
	interface Props {
		/** Array of available agents (supports Agent or AgentSummary) */
		agents: AgentItem[];
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
	 * Get status for the selected agent (only if full Agent type)
	 */
	const agentStatus = $derived(
		selectedAgent && isAgent(selectedAgent) ? selectedAgent.status : null
	);

	/**
	 * Get model info for display
	 */
	const modelInfo = $derived(() => {
		if (!selectedAgent) return null;
		if (isSummary(selectedAgent)) {
			return `${selectedAgent.provider} / ${selectedAgent.model}`;
		}
		return null;
	});

	/**
	 * Get capabilities or tools count
	 */
	const toolsInfo = $derived(() => {
		if (!selectedAgent) return null;
		if (isSummary(selectedAgent)) {
			const tools = selectedAgent.tools_count;
			const mcp = selectedAgent.mcp_servers_count;
			const parts: string[] = [];
			if (tools > 0) {
				const toolLabel = tools > 1 ? $i18n('workflow_agent_tools') : $i18n('workflow_agent_tool');
				parts.push(`${tools} ${toolLabel}`);
			}
			if (mcp > 0) parts.push(`${mcp} MCP`);
			return parts.length > 0 ? parts.join(', ') : null;
		}
		if (isAgent(selectedAgent) && selectedAgent.capabilities.length > 0) {
			const caps = selectedAgent.capabilities;
			return caps.slice(0, 3).join(', ') + (caps.length > 3 ? ` +${caps.length - 3}` : '');
		}
		return null;
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
		{#if agentStatus}
			<StatusIndicator status={agentStatus === 'busy' ? 'running' : 'idle'} size="sm" />
		{/if}
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
			{#if modelInfo()}
				<span class="agent-model">{modelInfo()}</span>
			{/if}
			{#if toolsInfo()}
				<span class="agent-capabilities">{toolsInfo()}</span>
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
