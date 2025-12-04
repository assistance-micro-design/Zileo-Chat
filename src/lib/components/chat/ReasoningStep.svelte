<!--
  ReasoningStep Component
  A collapsible reasoning step that shows the agent's thought process.
  Supports expand/collapse toggle with smooth animation.

  @example
  <ReasoningStep step="Analyzing user request..." expanded={false} />
  <ReasoningStep step="Planning workflow execution..." expanded={true} />
-->
<script lang="ts">
	import { ChevronDown, ChevronRight, Brain } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * ReasoningStep props
	 */
	interface Props {
		/** Reasoning step content */
		step: string;
		/** Whether the step is expanded */
		expanded?: boolean;
		/** Step number (optional) */
		stepNumber?: number;
	}

	let { step, expanded = $bindable(false), stepNumber }: Props = $props();

	/**
	 * Toggle expanded state
	 */
	function toggle(): void {
		expanded = !expanded;
	}
</script>

<div class="reasoning-step" class:expanded>
	<button
		type="button"
		class="reasoning-header"
		onclick={toggle}
		aria-expanded={expanded}
		aria-controls="reasoning-content"
	>
		<span class="reasoning-icon">
			<Brain size={14} />
		</span>
		{#if stepNumber !== undefined}
			<span class="step-number">{$i18n('chat_step').replace('{number}', String(stepNumber))}</span>
		{/if}
		<span class="reasoning-preview">
			{expanded ? step : step.slice(0, 50) + (step.length > 50 ? '...' : '')}
		</span>
		<span class="expand-icon">
			{#if expanded}
				<ChevronDown size={14} />
			{:else}
				<ChevronRight size={14} />
			{/if}
		</span>
	</button>
	{#if expanded}
		<div id="reasoning-content" class="reasoning-content">
			{step}
		</div>
	{/if}
</div>

<style>
	.reasoning-step {
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-sm);
		border: 1px solid var(--color-border-light);
		overflow: hidden;
		font-size: var(--font-size-xs);
	}

	.reasoning-step.expanded {
		background: var(--color-bg-secondary);
	}

	.reasoning-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		width: 100%;
		padding: var(--spacing-sm) var(--spacing-md);
		background: none;
		border: none;
		cursor: pointer;
		text-align: left;
		color: var(--color-text-secondary);
		transition: all var(--transition-fast);
	}

	.reasoning-header:hover {
		background: var(--color-bg-hover);
		color: var(--color-text-primary);
	}

	.reasoning-icon {
		color: var(--color-accent);
		display: flex;
		align-items: center;
	}

	.step-number {
		font-weight: var(--font-weight-medium);
		color: var(--color-text-tertiary);
		min-width: 50px;
	}

	.reasoning-preview {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.expand-icon {
		color: var(--color-text-tertiary);
		display: flex;
		align-items: center;
	}

	.reasoning-content {
		padding: var(--spacing-md);
		padding-top: 0;
		color: var(--color-text-primary);
		line-height: var(--line-height-relaxed);
		white-space: pre-wrap;
		word-break: break-word;
		border-top: 1px solid var(--color-border-light);
		margin-top: var(--spacing-sm);
	}
</style>
