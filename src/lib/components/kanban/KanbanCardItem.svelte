<!--
  Copyright 2025 Assistance Micro Design

  KanbanCardItem — a draggable card vignette displayed in a KanbanColumn.
  Shows title, agent name, status, optional error summary, and exposes a
  small action menu (view, delete) via callbacks. The parent owns the actions.
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';
	import { Badge, Button } from '$lib/components/ui';
	import { Eye, Trash2, FileText, Wand2 } from '@lucide/svelte';
	import type { KanbanCard, KanbanCardStatus } from '$types/kanban';
	import { setCardDragData } from '$lib/utils/dragDrop';
	import { runningWorkflows } from '$lib/stores/background-workflows';

	interface Props {
		card: KanbanCard;
		/** Name of the target agent (resolved by parent). */
		targetAgentName?: string;
		/** Callback for "view report" action (Review / Done). */
		onview?: (card: KanbanCard) => void;
		/** Callback for "improve prompt" action (Review / Done). */
		onimprove?: (card: KanbanCard) => void;
		/** Callback for "delete" action. */
		ondelete?: (card: KanbanCard) => void;
	}

	let { card, targetAgentName, onview, onimprove, ondelete }: Props = $props();

	const isRunning = $derived(card.column === 'doing');
	const isReviewable = $derived(card.column === 'review' || card.column === 'done');
	/** Progress chunks for this card (if workflow is running). */
	const liveProgress = $derived(
		card.workflow_id ? $runningWorkflows.find((w) => w.workflowId === card.workflow_id) : undefined
	);
	/**
	 * A 'doing' card whose workflow is no longer tracked by the background
	 * runner is stuck (crash, missed `workflow_complete` event, app restarted
	 * mid-run). Allow the user to clean it up with an explicit confirmation
	 * instead of leaving it pinned in the column.
	 */
	const isStuck = $derived(isRunning && !liveProgress);

	/** Status badge variant. */
	function badgeVariantFor(status: KanbanCardStatus): 'primary' | 'success' | 'warning' | 'error' {
		switch (status) {
			case 'done':
				return 'success';
			case 'failed':
				return 'error';
			case 'review':
				return 'warning';
			default:
				return 'primary';
		}
	}

	function handleDragStart(event: DragEvent): void {
		setCardDragData(event, [card.id]);
	}

	function handleDelete(): void {
		if (isStuck) {
			const ok = confirm($i18n('kanban_confirm_force_delete_stuck'));
			if (!ok) return;
		}
		ondelete?.(card);
	}
</script>

<article
	class="kanban-card"
	class:running={isRunning}
	draggable={!isRunning}
	ondragstart={handleDragStart}
	aria-grabbed={isRunning ? 'false' : undefined}
>
	<header class="card-head">
		<h4 class="card-title">{card.title}</h4>
		<Badge variant={badgeVariantFor(card.status)}>{$i18n(`kanban_status_${card.status}`)}</Badge>
	</header>

	{#if targetAgentName}
		<p class="card-meta">{targetAgentName}</p>
	{/if}

	{#if card.description}
		<p class="card-description">{card.description}</p>
	{/if}

	{#if isRunning && liveProgress}
		<p class="card-progress" aria-live="polite">
			{$i18n('kanban_card_running')}
		</p>
	{/if}

	{#if card.error_summary}
		<p class="card-error" role="alert">{card.error_summary}</p>
	{/if}

	<footer class="card-actions">
		{#if isReviewable && onview}
			<Button
				type="button"
				variant="ghost"
				size="sm"
				onclick={() => onview?.(card)}
				ariaLabel={$i18n('kanban_card_view_report')}
			>
				<Eye size={14} />
				{$i18n('kanban_card_view')}
			</Button>
		{/if}
		{#if isReviewable && onimprove && card.prompt_id}
			<Button
				type="button"
				variant="ghost"
				size="sm"
				onclick={() => onimprove?.(card)}
				ariaLabel={$i18n('kanban_card_improve_prompt')}
			>
				<Wand2 size={14} />
				{$i18n('kanban_card_improve')}
			</Button>
		{/if}
		{#if card.workflow_id}
			<span class="card-workflow-link" title={card.workflow_id} aria-hidden="true">
				<FileText size={12} />
			</span>
		{/if}
		{#if ondelete && (!isRunning || isStuck)}
			<Button
				type="button"
				variant="ghost"
				size="sm"
				onclick={handleDelete}
				ariaLabel={isStuck ? $i18n('kanban_card_force_delete') : $i18n('kanban_card_delete')}
			>
				<Trash2 size={14} />
			</Button>
		{/if}
	</footer>
</article>

<style>
	.kanban-card {
		background: var(--color-surface);
		border: 1px solid var(--color-border);
		border-radius: 6px;
		padding: 0.6rem 0.7rem;
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
		cursor: grab;
		transition: box-shadow var(--transition-fast);
	}
	.kanban-card:hover {
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
	}
	.kanban-card.running {
		cursor: default;
		opacity: 0.85;
		border-style: dashed;
	}
	.card-head {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: 0.5rem;
	}
	.card-title {
		margin: 0;
		font-size: 0.9rem;
		font-weight: 600;
		line-height: 1.3;
		flex: 1;
		min-width: 0;
	}
	.card-meta {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-muted);
	}
	.card-description {
		margin: 0;
		font-size: 0.82rem;
		color: var(--color-text);
		display: -webkit-box;
		-webkit-line-clamp: 3;
		line-clamp: 3;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}
	.card-progress {
		margin: 0;
		font-size: 0.78rem;
		color: var(--color-accent);
		font-style: italic;
	}
	.card-error {
		margin: 0;
		font-size: 0.78rem;
		color: var(--color-error);
	}
	.card-actions {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		margin-top: 0.25rem;
	}
	.card-workflow-link {
		margin-left: auto;
		color: var(--color-text-muted);
	}
</style>
