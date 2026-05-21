<!--
  Copyright 2025 Assistance Micro Design

  KanbanCardReportViewer — modal that displays the card metadata, the prompt
  used (resolved from prompt_id or inline_prompt) and the workflow link.
  Actions: validate (todo→done), improve prompt, delete.
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';
	import { Modal, Button, Badge } from '$lib/components/ui';
	import MarkdownRenderer from '$lib/components/ui/MarkdownRenderer.svelte';
	import { CheckCircle2, Wand2, Trash2, ExternalLink } from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import type { KanbanCard } from '$types/kanban';
	import type { AgentSummary } from '$types/agent';
	import type { PromptSummary } from '$types/prompt';

	interface Props {
		open: boolean;
		card: KanbanCard | null;
		agents: AgentSummary[];
		prompts: PromptSummary[];
		onclose: () => void;
		onvalidate?: (card: KanbanCard) => Promise<void>;
		onimprove?: (card: KanbanCard) => void;
		ondelete?: (card: KanbanCard) => Promise<void>;
	}

	let { open, card, agents, prompts, onclose, onvalidate, onimprove, ondelete }: Props = $props();

	const variables = $derived(card ? safeParseVariables(card.variables) : {});
	const targetAgent = $derived(card ? agents.find((a) => a.id === card.target_agent_id) : null);
	const prompt = $derived(card?.prompt_id ? prompts.find((p) => p.id === card.prompt_id) : null);

	function safeParseVariables(raw: string): Record<string, string> {
		if (!raw) return {};
		try {
			const parsed = JSON.parse(raw);
			if (parsed && typeof parsed === 'object') {
				return Object.fromEntries(
					Object.entries(parsed).map(([k, v]) => [k, typeof v === 'string' ? v : String(v)])
				);
			}
		} catch {
			// fall through to empty
		}
		return {};
	}

	async function openWorkflow(): Promise<void> {
		if (!card?.workflow_id) return;
		await goto(`/agent?workflow=${card.workflow_id}`);
	}
</script>

<Modal {open} title={card?.title ?? ''} {onclose}>
	{#snippet body()}
		{#if card}
			<div class="report-section">
				<div class="meta-row">
					<Badge variant={card.status === 'failed' ? 'error' : 'primary'}
						>{$i18n(`kanban_status_${card.status}`)}</Badge
					>
					{#if targetAgent}
						<span class="meta-pill">{targetAgent.name}</span>
					{/if}
					{#if prompt}
						<span class="meta-pill">{prompt.name}</span>
					{/if}
				</div>

				{#if card.description}
					<section>
						<h4>{$i18n('kanban_field_description')}</h4>
						<p class="multiline">{card.description}</p>
					</section>
				{/if}

				{#if card.inline_prompt}
					<section>
						<h4>{$i18n('kanban_field_inline_prompt')}</h4>
						<MarkdownRenderer content={card.inline_prompt} />
					</section>
				{/if}

				{#if Object.keys(variables).length > 0}
					<section>
						<h4>{$i18n('kanban_field_variables')}</h4>
						<dl class="variables-list">
							{#each Object.entries(variables) as [name, value] (name)}
								<dt>{name}</dt>
								<dd>{value}</dd>
							{/each}
						</dl>
					</section>
				{/if}

				{#if card.error_summary}
					<section class="error-block" role="alert">
						<h4>{$i18n('kanban_field_error')}</h4>
						<p class="multiline">{card.error_summary}</p>
					</section>
				{/if}

				{#if card.workflow_id}
					<section>
						<Button variant="ghost" size="sm" onclick={openWorkflow}>
							<ExternalLink size={14} />
							{$i18n('kanban_open_workflow')}
						</Button>
					</section>
				{/if}
			</div>
		{/if}
	{/snippet}
	{#snippet footer()}
		{#if card}
			{#if card.column === 'review' && onvalidate}
				<Button variant="primary" onclick={() => onvalidate?.(card)}>
					<CheckCircle2 size={14} />
					{$i18n('kanban_card_validate')}
				</Button>
			{/if}
			{#if onimprove && (card.column === 'review' || card.column === 'done') && card.prompt_id}
				<Button variant="secondary" onclick={() => onimprove?.(card)}>
					<Wand2 size={14} />
					{$i18n('kanban_card_improve')}
				</Button>
			{/if}
			{#if ondelete}
				<Button variant="danger" onclick={() => ondelete?.(card)}>
					<Trash2 size={14} />
					{$i18n('kanban_card_delete')}
				</Button>
			{/if}
		{/if}
		<Button variant="ghost" onclick={onclose}>{$i18n('common_close')}</Button>
	{/snippet}
</Modal>

<style>
	.report-section {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}
	.meta-row {
		display: flex;
		gap: 0.5rem;
		flex-wrap: wrap;
		align-items: center;
	}
	.meta-pill {
		font-size: 0.78rem;
		padding: 0.15rem 0.5rem;
		border-radius: 999px;
		background: var(--color-bg-secondary);
		color: var(--color-text-muted);
	}
	h4 {
		margin: 0 0 0.3rem;
		font-size: 0.95rem;
	}
	.multiline {
		margin: 0;
		white-space: pre-wrap;
	}
	.variables-list {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 0.25rem 0.6rem;
		font-size: 0.85rem;
	}
	.variables-list dt {
		font-weight: 600;
		color: var(--color-text-muted);
	}
	.variables-list dd {
		margin: 0;
	}
	.error-block {
		border-left: 3px solid var(--color-error);
		padding-left: 0.6rem;
	}
</style>
