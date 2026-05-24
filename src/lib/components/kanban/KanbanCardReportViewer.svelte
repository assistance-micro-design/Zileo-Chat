<!--
  Copyright 2025 Assistance Micro Design

  KanbanCardReportViewer — modal that displays the card metadata, the prompt
  used (resolved from prompt_id or inline_prompt), the workflow link and the
  Kanban agent's meta-interaction history (compose + analyze iterations).
  Actions: validate (todo→done), improve prompt, delete.
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';
	import { Modal, Button, Badge } from '$lib/components/ui';
	import MarkdownRenderer from '$lib/components/ui/MarkdownRenderer.svelte';
	import ToolCallBlock from '$lib/components/chat/ToolCallBlock.svelte';
	import {
		CheckCircle2,
		Wand2,
		Trash2,
		ExternalLink,
		ChevronDown,
		RefreshCw
	} from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import type { KanbanCard } from '$types/kanban';
	import type { AgentSummary } from '$types/agent';
	import type { PromptSummary } from '$types/prompt';
	import type { KanbanCardInteraction, InteractionIteration } from '$types/kanban_interaction';
	import { loadCardInteractions } from '$lib/services/kanban_interaction.service';
	import { getErrorMessage } from '$lib/utils/error';

	interface Props {
		open: boolean;
		card: KanbanCard | null;
		agents: AgentSummary[];
		prompts: PromptSummary[];
		onclose: () => void;
		onvalidate?: (card: KanbanCard) => Promise<void>;
		onimprove?: (card: KanbanCard) => void;
		ondelete?: (card: KanbanCard) => void;
		onreanalyze?: (card: KanbanCard) => Promise<void>;
	}

	let {
		open,
		card,
		agents,
		prompts,
		onclose,
		onvalidate,
		onimprove,
		ondelete,
		onreanalyze
	}: Props = $props();

	/**
	 * True while a manual re-analyze is in flight. Disables the button and
	 * surfaces a spinner so the user can't fire overlapping analyses (each one
	 * runs a full LLM tool loop). Reset when the call settles.
	 */
	let reanalyzing = $state(false);
	let reanalyzeError = $state<string | null>(null);

	async function handleReanalyze(c: KanbanCard): Promise<void> {
		if (reanalyzing) return;
		reanalyzing = true;
		reanalyzeError = null;
		try {
			await onreanalyze?.(c);
		} catch (e) {
			reanalyzeError = getErrorMessage(e);
		} finally {
			reanalyzing = false;
		}
	}

	const variables = $derived(card ? safeParseVariables(card.variables) : {});
	const targetAgent = $derived(card ? agents.find((a) => a.id === card.target_agent_id) : null);
	const prompt = $derived(card?.prompt_id ? prompts.find((p) => p.id === card.prompt_id) : null);

	/** Persisted meta-interaction history for this card (compose + analyze). */
	let interactions = $state<KanbanCardInteraction[]>([]);
	let interactionsLoading = $state(false);
	let interactionsError = $state<string | null>(null);
	/** Snapshot trigger: refetch when modal opens for a new card. */
	let loadedCardId = $state<string | null>(null);

	$effect(() => {
		// Snapshot at mount: load when the modal opens with a card whose
		// history we haven't fetched yet in this open session.
		if (open && card && card.id !== loadedCardId) {
			loadedCardId = card.id;
			fetchInteractions(card.id);
		}
		if (!open) {
			loadedCardId = null;
		}
	});

	async function fetchInteractions(cardId: string): Promise<void> {
		interactionsLoading = true;
		interactionsError = null;
		try {
			interactions = await loadCardInteractions(cardId);
		} catch (e) {
			interactionsError = getErrorMessage(e);
			interactions = [];
		} finally {
			interactionsLoading = false;
		}
	}

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

	function formatCost(usd: number): string {
		if (usd === 0) return '0.0000';
		if (usd < 0.0001) return usd.toExponential(2);
		return usd.toFixed(4);
	}

	function interactionLabel(kind: 'compose' | 'analyze'): string {
		return kind === 'compose' ? $i18n('kanban_history_compose') : $i18n('kanban_history_analyze');
	}

	/** Tracks which (interactionId, iterationIndex) reasoning panels are expanded. */
	let expandedReasoning = $state<Record<string, boolean>>({});

	function toggleReasoning(interactionId: string, iter: InteractionIteration): void {
		const key = `${interactionId}:${iter.iteration_index}`;
		expandedReasoning[key] = !expandedReasoning[key];
	}

	function isReasoningOpen(interactionId: string, iter: InteractionIteration): boolean {
		return expandedReasoning[`${interactionId}:${iter.iteration_index}`] === true;
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

				{#if reanalyzeError}
					<section class="error-block" role="alert">
						<h4>{$i18n('kanban_card_reanalyze')}</h4>
						<p class="multiline">{reanalyzeError}</p>
					</section>
				{/if}

				{#if interactionsLoading}
					<section>
						<h4>{$i18n('kanban_history_title')}</h4>
						<p class="muted">{$i18n('kanban_history_loading')}</p>
					</section>
				{:else if interactionsError}
					<section class="error-block" role="alert">
						<h4>{$i18n('kanban_history_title')}</h4>
						<p class="multiline">{interactionsError}</p>
					</section>
				{:else if interactions.length > 0}
					<section class="history-section">
						<h4>{$i18n('kanban_history_title')}</h4>
						{#each interactions as interaction (interaction.id)}
							<article class="interaction-card">
								<header class="interaction-header">
									<span class="interaction-kind">{interactionLabel(interaction.kind)}</span>
									<span class="meta-pill">{interaction.provider} · {interaction.model_id_used}</span
									>
									<span class="meta-pill">
										{$i18n('kanban_history_total_cost', {
											cost: formatCost(interaction.total_cost_usd)
										})}
									</span>
									<span class="meta-pill">
										{$i18n('kanban_history_tokens', {
											input: interaction.total_tokens_input,
											output: interaction.total_tokens_output
										})}
									</span>
								</header>
								{#if interaction.iterations.length === 0}
									<p class="muted">{$i18n('kanban_history_no_iterations')}</p>
								{:else}
									<ol class="iteration-list">
										{#each interaction.iterations as iter (iter.iteration_index)}
											<li class="iteration-item">
												<header class="iteration-header">
													<span class="iteration-title">
														{$i18n('kanban_history_iteration', { index: iter.iteration_index })}
													</span>
													<span class="meta-pill">
														{$i18n('kanban_history_tokens', {
															input: iter.tokens_input,
															output: iter.tokens_output
														})}
													</span>
													<span class="meta-pill">
														{$i18n('kanban_history_cost', { cost: formatCost(iter.cost_usd) })}
													</span>
												</header>
												{#if iter.reasoning}
													<button
														type="button"
														class="reasoning-toggle"
														aria-expanded={isReasoningOpen(interaction.id, iter)}
														onclick={() => toggleReasoning(interaction.id, iter)}
													>
														<ChevronDown
															size={14}
															style={isReasoningOpen(interaction.id, iter)
																? 'transform: rotate(0deg)'
																: 'transform: rotate(-90deg)'}
														/>
														{$i18n('kanban_history_reasoning')}
													</button>
													{#if isReasoningOpen(interaction.id, iter)}
														<div class="reasoning-body multiline">{iter.reasoning}</div>
													{/if}
												{/if}
												{#if iter.tool_calls.length > 0}
													<div class="tool-calls">
														{#each iter.tool_calls as call, idx (idx)}
															<ToolCallBlock
																toolName={call.tool_name}
																toolType={call.mcp_server ? 'mcp' : 'local'}
																serverName={call.mcp_server}
																inputParams={call.input_json}
																outputResult={call.output_json}
																success={call.success}
																durationMs={call.duration_ms}
																collapsed={true}
															/>
														{/each}
													</div>
												{/if}
												{#if iter.response_content}
													<section class="response-block">
														<h5>{$i18n('kanban_history_response')}</h5>
														<p class="multiline">{iter.response_content}</p>
													</section>
												{/if}
											</li>
										{/each}
									</ol>
								{/if}
							</article>
						{/each}
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
			{#if card.column === 'review' && card.workflow_id && onreanalyze}
				<Button variant="secondary" disabled={reanalyzing} onclick={() => handleReanalyze(card)}>
					<RefreshCw size={14} class={reanalyzing ? 'spin' : ''} />
					{reanalyzing ? $i18n('kanban_card_reanalyzing') : $i18n('kanban_card_reanalyze')}
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
	h5 {
		margin: 0.4rem 0 0.2rem;
		font-size: 0.85rem;
		color: var(--color-text-muted);
	}
	.multiline {
		margin: 0;
		white-space: pre-wrap;
	}
	.muted {
		color: var(--color-text-muted);
		font-size: 0.85rem;
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
	.history-section {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
	}
	.interaction-card {
		border: 1px solid var(--color-border);
		border-radius: 0.5rem;
		padding: 0.6rem 0.75rem;
		background: var(--color-bg-secondary);
	}
	.interaction-header,
	.iteration-header {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem;
		align-items: center;
		margin-bottom: 0.4rem;
	}
	.interaction-kind {
		font-weight: 600;
	}
	.iteration-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	.iteration-item {
		border-left: 2px solid var(--color-border);
		padding-left: 0.5rem;
	}
	.iteration-title {
		font-weight: 600;
		font-size: 0.85rem;
	}
	.reasoning-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		background: none;
		border: none;
		padding: 0.2rem 0;
		font-size: 0.82rem;
		cursor: pointer;
		color: var(--color-text-muted);
	}
	.reasoning-toggle:hover {
		color: var(--color-text);
	}
	.reasoning-body {
		font-size: 0.82rem;
		color: var(--color-text-muted);
		padding: 0.3rem 0.5rem;
		background: var(--color-bg);
		border-radius: 0.3rem;
		margin: 0.2rem 0;
	}
	.tool-calls {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		margin-top: 0.3rem;
	}
	.response-block {
		margin-top: 0.4rem;
	}
	:global(.spin) {
		animation: kanban-reanalyze-spin 1s linear infinite;
	}
	@keyframes kanban-reanalyze-spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
