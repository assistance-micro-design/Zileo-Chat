<!--
  Copyright 2025 Assistance Micro Design

  KanbanCardItem — card vignette displayed in a KanbanColumn.
  Shows title, agent name, status, optional error summary, and exposes a
  small action menu (view, delete) via callbacks. The parent owns the actions.
  Column transitions are driven by the backend (workflow lifecycle), not by
  drag & drop.
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';
	import { Badge, Button, Spinner } from '$lib/components/ui';
	import {
		Bot,
		Eye,
		Trash2,
		FileText,
		Wand2,
		Repeat,
		Copy,
		Pencil,
		Clock,
		RotateCcw
	} from '@lucide/svelte';
	import type { KanbanCard, KanbanCardStatus } from '$types/kanban';
	import { runningWorkflows } from '$lib/stores/background-workflows';

	interface Props {
		card: KanbanCard;
		/** Name of the target agent (resolved by parent). */
		targetAgentName?: string;
		/** True when a kanban_schedule row currently points to this card. */
		hasSchedule?: boolean;
		/** True while the Kanban agent is finalizing the report for this card. */
		isAnalyzing?: boolean;
		/** Callback for "view report" action (Review / Done). */
		onview?: (card: KanbanCard) => void;
		/** Callback for "improve prompt" action (Review / Done). */
		onimprove?: (card: KanbanCard) => void;
		/** Callback for "delete" action. */
		ondelete?: (card: KanbanCard) => void;
		/** Callback for "manage recurrence" action (completed cards only). */
		onschedule?: (card: KanbanCard) => void;
		/** Callback for "duplicate as template" action (completed cards only). */
		onduplicate?: (card: KanbanCard) => void;
		/** Callback for "edit card" action (any card except running). */
		onedit?: (card: KanbanCard) => void;
		/** Callback for "retry" action: re-queue a failed/rejected review card for
		 *  a fresh run (K5). */
		onretry?: (card: KanbanCard) => void;
	}

	let {
		card,
		targetAgentName,
		hasSchedule = false,
		isAnalyzing = false,
		onview,
		onimprove,
		ondelete,
		onschedule,
		onduplicate,
		onedit,
		onretry
	}: Props = $props();

	const isRunning = $derived(card.column === 'doing');
	const isReviewable = $derived(card.column === 'review' || card.column === 'done');
	/**
	 * Eligible for "manage recurrence" / "duplicate as template": review (success)
	 * or done. Excludes failed cards (column=review, status=failed) on purpose.
	 */
	const isCompleted = $derived(
		(card.column === 'review' && card.status === 'review') ||
			(card.column === 'done' && card.status === 'done')
	);
	/**
	 * Eligible for "retry" (K5): a review card that FAILED or was REJECTED
	 * (carries an error_summary). These would otherwise pile up in review with
	 * no affordance. Re-queuing relaunches a fresh run (Phase 1 Review→Todo).
	 * `needs_improvement` is excluded on purpose — it is handled by the
	 * improve-prompt flow (K4), which re-queues after editing the prompt. A
	 * success awaiting validation has no error_summary, so it is excluded too.
	 */
	const canRetry = $derived(
		card.column === 'review' && (card.status === 'failed' || !!card.error_summary)
	);
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

	/**
	 * Display status disambiguates the raw `card.status` against the column.
	 * The backend sets `status='done'` on a successful workflow that lands in
	 * the review column (awaiting user validation) and again when the user
	 * validates it into the done column. Showing the same "Terminé" badge in
	 * both places is misleading; same story for a scheduled template in todo
	 * which is not actually "ready to execute" — it's parked for its tick.
	 */
	type DisplayStatus = KanbanCardStatus | 'awaiting_review' | 'scheduled';
	const displayStatus = $derived<DisplayStatus>(
		card.column === 'review' && card.status === 'done'
			? 'awaiting_review'
			: card.column === 'todo' && card.status === 'ready' && hasSchedule
				? 'scheduled'
				: card.status
	);

	/** Status badge variant — handles both raw and synthesized statuses. */
	function badgeVariantFor(s: DisplayStatus): 'primary' | 'success' | 'warning' | 'error' {
		switch (s) {
			case 'awaiting_review':
				return 'warning';
			case 'scheduled':
				return 'primary';
		}
		switch (s as KanbanCardStatus) {
			case 'done':
				return 'success';
			case 'failed':
				return 'error';
			case 'review':
				return 'warning';
			case 'proposed':
				// Generated, awaiting human validation — same "needs attention"
				// amber as review (BLOQUANT-2), not the misleading default primary.
				return 'warning';
			default:
				return 'primary';
		}
	}

	/**
	 * Days remaining before the scheduler auto-purges this card. Returns
	 * `null` when the countdown doesn't apply: cards outside the `done`
	 * column, or `done` cards attached to an enabled schedule (templates
	 * are never purged). Mirrors `DONE_CARD_TTL_DAYS` (3) on the backend.
	 */
	const DONE_CARD_TTL_DAYS = 3;
	const purgeCountdownDays = $derived.by<number | null>(() => {
		if (card.column !== 'done' || hasSchedule) return null;
		const updatedMs = Date.parse(card.updated_at);
		if (!Number.isFinite(updatedMs)) return null;
		const elapsedDays = (Date.now() - updatedMs) / 86_400_000;
		return Math.max(0, Math.ceil(DONE_CARD_TTL_DAYS - elapsedDays));
	});

	function handleDelete(): void {
		ondelete?.(card);
	}
</script>

<article class="kanban-card" class:running={isRunning}>
	<header class="card-head">
		<h4 class="card-title">{card.title}</h4>
		<div class="card-head-badges">
			{#if hasSchedule}
				<Badge variant="primary">
					<Repeat size={11} aria-hidden="true" />
					{$i18n('kanban_schedule_active_badge')}
				</Badge>
			{/if}
			<Badge variant={badgeVariantFor(displayStatus)}>
				{$i18n(`kanban_status_${displayStatus}`)}
			</Badge>
			{#if purgeCountdownDays !== null}
				<span
					class="card-purge-countdown"
					class:imminent={purgeCountdownDays === 0}
					title={$i18n('kanban_card_purge_tooltip')}
				>
					<Clock size={11} aria-hidden="true" />
					{purgeCountdownDays === 0
						? $i18n('kanban_card_purge_imminent')
						: $i18n('kanban_card_purge_in_days').replace('{days}', String(purgeCountdownDays))}
				</span>
			{/if}
		</div>
	</header>

	{#if targetAgentName}
		<p class="card-meta">
			<Bot size={13} aria-hidden="true" />
			{targetAgentName}
		</p>
	{/if}

	{#if card.description}
		<p class="card-description">{card.description}</p>
	{/if}

	{#if isRunning && liveProgress}
		<p class="card-progress" aria-live="polite">
			<Spinner size="sm" />
			<span>{$i18n('kanban_card_running')}</span>
		</p>
	{/if}

	{#if isAnalyzing}
		<p class="card-analyzing" aria-live="polite">
			<Spinner size="sm" />
			<span>{$i18n('kanban_card_analyzing')}</span>
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
		{#if canRetry && onretry}
			<Button
				type="button"
				variant="ghost"
				size="sm"
				onclick={() => onretry?.(card)}
				ariaLabel={$i18n('kanban_card_retry_aria')}
			>
				<RotateCcw size={14} />
				{$i18n('kanban_card_retry')}
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
		{#if isCompleted && onschedule}
			<Button
				type="button"
				variant="ghost"
				size="sm"
				onclick={() => onschedule?.(card)}
				ariaLabel={$i18n('kanban_card_schedule_aria')}
			>
				<Repeat size={14} />
				{$i18n('kanban_card_schedule')}
			</Button>
		{/if}
		{#if !isRunning && onedit}
			<Button
				type="button"
				variant="ghost"
				size="sm"
				onclick={() => onedit?.(card)}
				ariaLabel={$i18n('kanban_card_edit_aria')}
			>
				<Pencil size={14} />
				{$i18n('kanban_card_edit')}
			</Button>
		{/if}
		{#if isCompleted && onduplicate}
			<Button
				type="button"
				variant="ghost"
				size="sm"
				onclick={() => onduplicate?.(card)}
				ariaLabel={$i18n('kanban_card_duplicate_template_aria')}
			>
				<Copy size={14} />
				{$i18n('kanban_card_duplicate_template')}
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
	/* The status rib inherits --col-tint from the hosting column (custom
	   properties cascade through the display:contents card slot). */
	.kanban-card {
		position: relative;
		background: var(--color-surface);
		border: 1px solid var(--color-border);
		border-left: 3px solid var(--col-tint, var(--color-border));
		border-radius: var(--border-radius-md);
		padding: 0.6rem 0.7rem;
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
		box-shadow: var(--shadow-xs);
		transition:
			box-shadow var(--transition-base),
			transform var(--transition-base);
	}
	.kanban-card:hover {
		box-shadow: var(--shadow-md);
		transform: translateY(-1px);
	}
	.kanban-card.running {
		border-left-color: var(--color-status-running);
	}
	.kanban-card.running::after {
		content: '';
		position: absolute;
		inset: 0;
		border-radius: inherit;
		pointer-events: none;
		box-shadow: inset 0 0 0 1px rgba(59, 130, 246, 0.25);
		animation: pulse 2.4s ease-in-out infinite;
	}
	.card-head {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: 0.5rem;
	}
	.card-head-badges {
		display: flex;
		gap: 0.25rem;
		flex-wrap: wrap;
		justify-content: flex-end;
	}
	.card-title {
		margin: 0;
		font-size: var(--font-size-sm);
		font-weight: 600;
		line-height: 1.3;
		flex: 1;
		min-width: 0;
	}
	/* Neutral pill (border + tertiary surface), matching the mock's
	   auto-purge countdown badge. */
	.card-purge-countdown {
		display: inline-flex;
		align-items: center;
		gap: 0.2rem;
		font-size: var(--font-size-2xs);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-secondary);
		padding: 0.1rem 0.45rem;
		border-radius: var(--border-radius-full);
		background: var(--color-bg-tertiary);
		border: 1px solid var(--color-border);
		white-space: nowrap;
	}
	.card-purge-countdown.imminent {
		color: var(--color-warning);
	}
	.card-meta {
		margin: 0;
		display: flex;
		align-items: center;
		gap: 0.25rem;
		font-size: var(--font-size-2xs);
		color: var(--color-text-tertiary);
	}
	.card-description {
		margin: 0;
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		display: -webkit-box;
		-webkit-line-clamp: 3;
		line-clamp: 3;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}
	/* Live activity lines read in the info channel, like the mock's
	   in-progress card. */
	.card-progress {
		margin: 0;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: var(--font-size-xs);
		color: var(--color-info);
		font-style: italic;
	}
	.card-analyzing {
		margin: 0;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: var(--font-size-xs);
		color: var(--color-info);
		font-style: italic;
	}
	.card-error {
		margin: 0;
		font-size: var(--font-size-xs);
		color: var(--color-error);
		white-space: pre-wrap;
		background: var(--color-error-light);
		border-radius: var(--border-radius-sm);
		padding: 0.35rem 0.5rem;
	}
	/* Actions revealed on hover or keyboard focus (desktop-first choice:
	   to revisit if touch usage ever appears). */
	.card-actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.25rem 0.35rem;
		margin-top: 0.25rem;
		min-width: 0;
		opacity: 0.45;
		transition: opacity var(--transition-fast);
	}
	.kanban-card:hover .card-actions,
	.kanban-card:focus-within .card-actions {
		opacity: 1;
	}
	@media (prefers-reduced-motion: reduce) {
		.kanban-card.running::after {
			animation: none;
		}
	}
	.card-workflow-link {
		color: var(--color-text-muted);
		display: inline-flex;
		align-items: center;
	}
</style>
