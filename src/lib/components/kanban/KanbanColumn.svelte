<!--
  Copyright 2025 Assistance Micro Design

  KanbanColumn — droppable column for a single Kanban status (todo/doing/review/done).
  Renders its cards via a snippet so the parent controls card rendering.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { KanbanCard, KanbanColumn as Col } from '$types/kanban';
	import { hasCardDragData, getCardIdsFromDrag } from '$lib/utils/dragDrop';

	interface Props {
		/** Column status (also doubles as its identifier). */
		column: Col;
		/** Display label, fully translated. */
		title: string;
		/** Cards belonging to this column, already sorted by `column_order`. */
		cards: KanbanCard[];
		/** Render slot for each card. */
		card: Snippet<[KanbanCard, number]>;
		/** Called when one or more cards are dropped on this column. */
		ondrop?: (cardIds: string[], targetColumn: Col, targetOrder: number) => void;
	}

	let { column, title, cards, card, ondrop }: Props = $props();

	let dragOver = $state(false);

	function handleDragOver(event: DragEvent): void {
		if (!hasCardDragData(event)) return;
		event.preventDefault();
		if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
		dragOver = true;
	}

	function handleDragLeave(): void {
		dragOver = false;
	}

	function handleDrop(event: DragEvent): void {
		dragOver = false;
		const ids = getCardIdsFromDrag(event);
		if (!ids || ids.length === 0 || !ondrop) return;
		event.preventDefault();
		const targetOrder = cards.length;
		ondrop(ids, column, targetOrder);
	}
</script>

<section class="kanban-column" class:drag-over={dragOver} data-column={column} aria-label={title}>
	<header class="kanban-column-head">
		<h3 class="kanban-column-title">{title}</h3>
		<span class="kanban-column-count" aria-label={`${cards.length}`}>{cards.length}</span>
	</header>

	<div
		class="kanban-column-body"
		role="list"
		ondragover={handleDragOver}
		ondragleave={handleDragLeave}
		ondrop={handleDrop}
	>
		{#each cards as c, i (c.id)}
			<div role="listitem" class="kanban-card-slot">
				{@render card(c, i)}
			</div>
		{/each}
	</div>
</section>

<style>
	.kanban-column {
		display: flex;
		flex-direction: column;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: 8px;
		min-height: 200px;
		max-height: 100%;
		overflow: hidden;
		transition: border-color var(--transition-fast);
	}
	.kanban-column.drag-over {
		border-color: var(--color-accent);
		box-shadow: 0 0 0 2px var(--color-accent) inset;
	}
	.kanban-column-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.6rem 0.8rem;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-tertiary);
	}
	.kanban-column-title {
		margin: 0;
		font-size: 0.95rem;
		font-weight: 600;
	}
	.kanban-column-count {
		font-size: 0.8rem;
		color: var(--color-text-muted);
		background: var(--color-bg-primary);
		padding: 0.1rem 0.5rem;
		border-radius: 999px;
	}
	.kanban-column-body {
		flex: 1;
		overflow-y: auto;
		padding: 0.5rem;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	.kanban-card-slot {
		display: contents;
	}
</style>
