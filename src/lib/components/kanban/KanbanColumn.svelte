<!--
  Copyright 2025 Assistance Micro Design

  KanbanColumn — visual column for a single Kanban status (todo/doing/review/done).
  Renders its cards via a snippet so the parent controls card rendering.
  When empty, shows a sober icon + label centered in the body.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { i18n } from '$lib/i18n';
	import { pauseOnScroll } from '$lib/actions/pauseOnScroll';
	import { ClipboardList } from '@lucide/svelte';
	import type { KanbanCard, KanbanColumn as Col } from '$types/kanban';

	interface Props {
		/** Column status (also doubles as its identifier). */
		column: Col;
		/** Display label, fully translated. */
		title: string;
		/** Cards belonging to this column, already sorted by `column_order`. */
		cards: KanbanCard[];
		/** Render slot for each card. */
		card: Snippet<[KanbanCard, number]>;
	}

	let { column, title, cards, card }: Props = $props();
</script>

<section class="kanban-column" data-column={column} aria-label={title}>
	<header class="kanban-column-head">
		<span class="kanban-column-dot" aria-hidden="true"></span>
		<h3 class="kanban-column-title">{title}</h3>
		<span class="kanban-column-count" aria-label={`${cards.length}`}>{cards.length}</span>
	</header>

	<div class="kanban-column-body" role="list" {@attach pauseOnScroll()}>
		{#if cards.length === 0}
			<div class="kanban-column-empty" aria-hidden="true">
				<ClipboardList size={28} strokeWidth={1.5} />
				<span>{$i18n('kanban_column_empty')}</span>
			</div>
		{:else}
			{#each cards as c, i (c.id)}
				<div role="listitem" class="kanban-card-slot">
					{@render card(c, i)}
				</div>
			{/each}
		{/if}
	</div>
</section>

<style>
	/* Each column carries a status tint (--col-tint) driving its glowing dot
	   in the header; cards reuse the same variable for their side rib. */
	.kanban-column {
		--col-tint: var(--color-text-tertiary);
		display: flex;
		flex-direction: column;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		min-height: 200px;
		max-height: 100%;
		overflow: hidden;
	}
	.kanban-column[data-column='todo'] {
		--col-tint: var(--color-status-idle);
	}
	.kanban-column[data-column='doing'] {
		--col-tint: var(--color-status-running);
	}
	.kanban-column[data-column='review'] {
		--col-tint: var(--color-warning);
	}
	.kanban-column[data-column='done'] {
		--col-tint: var(--color-online);
	}
	.kanban-column-head {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: 0.6rem 0.8rem;
		border-bottom: 1px solid var(--color-border-light);
	}
	.kanban-column-dot {
		width: 9px;
		height: 9px;
		border-radius: 50%;
		background: var(--col-tint);
		box-shadow: 0 0 6px var(--col-tint);
		flex-shrink: 0;
	}
	.kanban-column[data-column='doing'] .kanban-column-dot {
		animation: pulse 2s ease-in-out infinite;
	}
	.kanban-column-title {
		margin: 0;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
	}
	.kanban-column-count {
		margin-left: auto;
		font-size: var(--font-size-2xs);
		font-weight: var(--font-weight-semibold);
		font-variant-numeric: tabular-nums;
		color: var(--color-text-secondary);
		background: var(--surface-1);
		border: 1px solid var(--color-border);
		padding: 0.05rem 0.5rem;
		border-radius: var(--border-radius-full);
	}
	@media (prefers-reduced-motion: reduce) {
		.kanban-column[data-column='doing'] .kanban-column-dot {
			animation: none;
		}
	}
	.kanban-column-body {
		flex: 1;
		overflow-y: auto;
		padding: 0.5rem;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	.kanban-column-empty {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		color: var(--color-text-muted);
		font-size: var(--font-size-sm);
		opacity: 0.6;
		user-select: none;
	}
	.kanban-card-slot {
		display: contents;
	}
</style>
