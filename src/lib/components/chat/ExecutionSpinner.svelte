<!--
  Copyright 2025 Assistance Micro Design
  SPDX-License-Identifier: Apache-2.0

  ExecutionSpinner Component - SA-019 P3
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
	.execution-spinner {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-sm) var(--spacing-md);
		color: var(--color-text-secondary);
		font-size: var(--font-size-sm);
		animation: fadeIn 0.2s ease-in;
	}

	.spinner-text {
		font-style: italic;
	}

	@keyframes fadeIn {
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
