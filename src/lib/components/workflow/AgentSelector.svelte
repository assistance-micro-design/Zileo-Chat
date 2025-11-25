<!--
  AgentSelector Component
  A dropdown selector for choosing an agent.
  Displays agent name with status indicator.

  @example
  <AgentSelector agents={availableAgents} selected={currentAgentId} onselect={handleAgentSelect} />
-->
<script lang="ts">
	import type { Agent } from '$types/agent';
	import Select from '$lib/components/ui/Select.svelte';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import StatusIndicator from '$lib/components/ui/StatusIndicator.svelte';
	import { Bot } from 'lucide-svelte';

	/**
	 * AgentSelector props
	 */
	interface Props {
		/** Array of available agents */
		agents: Agent[];
		/** ID of the currently selected agent */
		selected?: string;
		/** Selection handler */
		onselect?: (agentId: string) => void;
		/** Whether the selector is disabled */
		disabled?: boolean;
		/** Label for the selector */
		label?: string;
	}

	let { agents, selected = $bindable(''), onselect, disabled = false, label = 'Agent' }: Props = $props();

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
		{#if selectedAgent}
			<StatusIndicator status={selectedAgent.status === 'busy' ? 'running' : 'idle'} size="sm" />
		{/if}
	</div>
	<Select
		{options}
		value={selected}
		{disabled}
		placeholder="Select an agent..."
		onchange={handleChange}
	/>
	{#if selectedAgent}
		<div class="agent-info">
			<span class="agent-lifecycle">{selectedAgent.lifecycle}</span>
			{#if selectedAgent.capabilities.length > 0}
				<span class="agent-capabilities">
					{selectedAgent.capabilities.slice(0, 3).join(', ')}
					{#if selectedAgent.capabilities.length > 3}
						+{selectedAgent.capabilities.length - 3} more
					{/if}
				</span>
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

	.agent-capabilities {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
