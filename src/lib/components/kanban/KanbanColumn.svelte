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
	.kanban-column {
		display: flex;
		flex-direction: column;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: 8px;
		min-height: 200px;
		max-height: 100%;
		overflow: hidden;
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
		font-size: var(--font-size-base);
		font-weight: 600;
	}
	.kanban-column-count {
		font-size: var(--font-size-xs);
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
