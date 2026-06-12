<!--
  Copyright 2025 Assistance Micro Design
  SPDX-License-Identifier: Apache-2.0

  ExecutionSpinner Component
  Animated spinner with contextual text shown between execution blocks.
-->

<script lang="ts">
	import { Spinner } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';

	interface Props {
		context: string | null;
		active: boolean;
	}

	let { context, active }: Props = $props();

	const displayText = $derived(
		context
			? $i18n('chat_spinner_tool_call').replace('{tool}', context)
			: $i18n('chat_spinner_processing')
	);
</script>

{#if active}
	<div class="execution-spinner" role="status" aria-live="polite">
		<Spinner size="sm" />
		<span class="spinner-text">{displayText}</span>
	</div>
{/if}

<style>
	/* Status pill: compact, on its own surface so it reads as a live step of
	   the execution thread rather than a bare line of text. */
	.execution-spinner {
		display: inline-flex;
		align-items: center;
		align-self: flex-start;
		gap: var(--spacing-sm);
		padding: 0.35rem 0.8rem;
		margin: var(--spacing-xs) 0;
		color: var(--color-text-secondary);
		font-size: var(--font-size-xs);
		background: var(--surface-1);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-full);
		box-shadow: var(--shadow-xs);
		animation: fadeInOpacity var(--transition-base);
	}

	.spinner-text {
		font-style: italic;
	}

	/* Opacity-only fade: the global fadeIn also slides vertically, which is
	   unwanted between execution blocks. Distinct name avoids shadowing. */
	@keyframes fadeInOpacity {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.execution-spinner {
			animation: none;
		}
	}
</style>
