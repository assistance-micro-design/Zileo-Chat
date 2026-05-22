<!--
  Copyright 2025 Assistance Micro Design

  KanbanBoard — 4-column board (todo / doing / review / done).
  Layout-only: column transitions are driven by the backend (workflow
  lifecycle), so the board does not expose any drop handler.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { i18n } from '$lib/i18n';
	import type { KanbanCard, KanbanColumn as Col } from '$types/kanban';
	import KanbanColumn from './KanbanColumn.svelte';

	interface Props {
		/** Cards grouped by column. */
		cardsByColumn: Record<Col, KanbanCard[]>;
		/** Render slot for each card; receives (card, index). */
		card: Snippet<[KanbanCard, number]>;
	}

	let { cardsByColumn, card }: Props = $props();

	const columns: { id: Col; labelKey: string }[] = [
		{ id: 'todo', labelKey: 'kanban_col_todo' },
		{ id: 'doing', labelKey: 'kanban_col_doing' },
		{ id: 'review', labelKey: 'kanban_col_review' },
		{ id: 'done', labelKey: 'kanban_col_done' }
	];
</script>

<div class="kanban-board" role="region" aria-label={$i18n('kanban_board_aria')}>
	{#each columns as col (col.id)}
		<KanbanColumn
			column={col.id}
			title={$i18n(col.labelKey)}
			cards={cardsByColumn[col.id]}
			{card}
		/>
	{/each}
</div>

<style>
	.kanban-board {
		display: grid;
		grid-template-columns: repeat(4, minmax(220px, 1fr));
		gap: 0.75rem;
		flex: 1;
		min-height: 0;
		width: 100%;
		padding: 0.5rem 0;
	}
	@media (max-width: 960px) {
		.kanban-board {
			grid-template-columns: repeat(2, minmax(220px, 1fr));
		}
	}
	@media (max-width: 560px) {
		.kanban-board {
			grid-template-columns: 1fr;
		}
	}
</style>
