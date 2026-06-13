<!--
  Copyright 2025 Assistance Micro Design

  KanbanSupervisorNotice — advisory nudge shown inside the card creator when a
  global supervisor role is unset (info) or dangling (warning, agent deleted).
  Renders nothing when the role is correctly configured ('ok'). The configure
  link routes to Settings › Kanban and closes the (root-mounted) creator modal
  so the user lands on the settings page unobstructed.
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';
	import { Info, TriangleAlert } from '@lucide/svelte';
	import { cardCreatorStore } from '$lib/stores/card-creator';
	import type { SupervisorRoleState } from '$lib/utils/kanban-supervisors';

	interface Props {
		/** Configuration state for this role. */
		state: SupervisorRoleState;
		/** i18n key for the "unset" (info) message. */
		unsetKey: string;
		/** i18n key for the "dangling" (warning) message. */
		danglingKey: string;
	}

	let { state, unsetKey, danglingKey }: Props = $props();
</script>

{#if state !== 'ok'}
	<p
		class="notice"
		class:warning={state === 'dangling'}
		role={state === 'dangling' ? 'alert' : 'note'}
	>
		{#if state === 'dangling'}
			<TriangleAlert size={16} aria-hidden="true" />
		{:else}
			<Info size={16} aria-hidden="true" />
		{/if}
		<span class="notice-text">
			{$i18n(state === 'dangling' ? danglingKey : unsetKey)}
		</span>
		<a class="configure-link" href="/settings/kanban" onclick={() => cardCreatorStore.close()}>
			{$i18n('kanban_supervisor_configure_link')}
		</a>
	</p>
{/if}

<style>
	.notice {
		margin: 0;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
		padding: var(--spacing-sm) var(--spacing-md);
		font-size: var(--font-size-sm);
		color: var(--color-info);
		background: var(--color-info-light);
		border: 1px solid rgba(59, 130, 246, 0.3);
		border-radius: var(--border-radius-md);
	}
	.notice.warning {
		color: var(--color-warning);
		background: var(--color-warning-bg);
		border-color: var(--color-warning);
	}
	.notice :global(svg) {
		flex-shrink: 0;
	}
	.notice-text {
		flex: 1;
		min-width: 0;
	}
	.configure-link {
		flex-shrink: 0;
		font-weight: var(--font-weight-medium);
		color: inherit;
		text-decoration: underline;
	}
</style>
