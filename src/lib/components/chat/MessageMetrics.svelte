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
  MessageMetrics Component
  Displays structured metrics below assistant message bubbles.
  Shows model, tokens, duration, cost, and sub-agent chips.

  @example
  <MessageMetrics message={assistantMessage} />
-->
<script lang="ts">
	import type { Message } from '$types/message';
	import { Clock, Cpu, ArrowDownToLine, CircleDollarSign, Users } from '@lucide/svelte';
	import { formatTokenCount } from '$lib/utils/activity';
	import { formatDuration } from '$lib/utils/duration';

	/**
	 * MessageMetrics props
	 */
	interface Props {
		/** Assistant message with metrics */
		message: Message;
	}

	let { message }: Props = $props();

	const hasMetrics = $derived(
		(message.tokens_input ?? 0) > 0 ||
			(message.tokens_output ?? 0) > 0 ||
			message.model !== undefined ||
			message.duration_ms !== undefined
	);
</script>

{#if hasMetrics}
	<div class="message-metrics">
		{#if message.model}
			<span class="metric" title={message.provider ? `${message.provider} / ${message.model}` : message.model}>
				<Cpu size={12} />
				{message.provider ? `${message.provider} / ` : ''}{message.model}
			</span>
		{/if}

		{#if (message.tokens_input ?? 0) > 0 || (message.tokens_output ?? 0) > 0}
			<span class="metric">
				<ArrowDownToLine size={12} />
				{formatTokenCount(message.tokens_input ?? 0)} / {formatTokenCount(message.tokens_output ?? 0)}
			</span>
		{/if}

		{#if message.duration_ms}
			<span class="metric">
				<Clock size={12} />
				{formatDuration(message.duration_ms)}
			</span>
		{/if}

		{#if message.cost_usd && message.cost_usd > 0}
			<span class="metric">
				<CircleDollarSign size={12} />
				${message.cost_usd < 0.01 ? message.cost_usd.toFixed(4) : message.cost_usd.toFixed(2)}
			</span>
		{/if}
	</div>

	{#if message.sub_agents && message.sub_agents.length > 0}
		<div class="sub-agents-bar">
			<Users size={12} />
			{#each message.sub_agents as agent (agent.name)}
				<span class="sub-agent-chip" class:error={agent.status === 'error'}>
					<span class="agent-name">{agent.name}</span>
					{#if agent.tokens_input || agent.tokens_output}
						<span class="agent-tokens">
							{formatTokenCount(agent.tokens_input ?? 0)}/{formatTokenCount(agent.tokens_output ?? 0)}
						</span>
					{/if}
					{#if agent.duration_ms}
						<span class="agent-duration">{formatDuration(agent.duration_ms)}</span>
					{/if}
				</span>
			{/each}
		</div>
	{/if}
{/if}

<style>
	.message-metrics {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding-top: var(--spacing-xs);
		font-size: var(--font-size-xs);
		flex-wrap: wrap;
	}

	.metric {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		color: var(--color-text-tertiary);
		font-family: var(--font-mono);
	}

	.sub-agents-bar {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding-top: var(--spacing-xs);
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		flex-wrap: wrap;
	}

	.sub-agent-chip {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-xs);
		padding: 2px var(--spacing-sm);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-full);
		font-family: var(--font-mono);
	}

	.sub-agent-chip.error {
		background: var(--color-error-bg, rgba(239, 68, 68, 0.1));
		color: var(--color-error, #ef4444);
	}

	.agent-name {
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
	}

	.sub-agent-chip.error .agent-name {
		color: var(--color-error, #ef4444);
	}

	.agent-tokens,
	.agent-duration {
		color: var(--color-text-tertiary);
	}
</style>
