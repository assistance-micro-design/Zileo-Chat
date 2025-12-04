<!--
  MessageListSkeleton Component
  A skeleton loading state for the MessageList component.
  Shows placeholder message bubbles during loading.

  @example
  <MessageListSkeleton count={3} />
-->
<script lang="ts">
	import { Skeleton } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';

	/**
	 * MessageListSkeleton props
	 */
	interface Props {
		/** Number of skeleton messages to display */
		count?: number;
	}

	let { count = 3 }: Props = $props();

	/**
	 * Generate skeleton items with alternating alignment
	 */
	const skeletonItems = $derived(
		Array.from({ length: count }, (_, i) => ({
			id: `skeleton-${i}`,
			isUser: i % 2 === 0
		}))
	);
</script>

<div class="message-list-skeleton" role="presentation" aria-label={$i18n('chat_loading_messages')}>
	{#each skeletonItems as item (item.id)}
		<div class="skeleton-message" class:user={item.isUser}>
			<div class="skeleton-avatar">
				<Skeleton variant="circular" size="32px" />
			</div>
			<div class="skeleton-content">
				<Skeleton variant="text" width="80px" height="0.75rem" />
				<div class="skeleton-body">
					<Skeleton variant="text" width={item.isUser ? '60%' : '80%'} />
					<Skeleton variant="text" width={item.isUser ? '40%' : '65%'} />
					{#if !item.isUser}
						<Skeleton variant="text" width="50%" />
					{/if}
				</div>
			</div>
		</div>
	{/each}
</div>

<style>
	.message-list-skeleton {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
		padding: var(--spacing-lg);
	}

	.skeleton-message {
		display: flex;
		gap: var(--spacing-sm);
		max-width: 70%;
	}

	.skeleton-message.user {
		flex-direction: row-reverse;
		margin-left: auto;
	}

	.skeleton-avatar {
		flex-shrink: 0;
	}

	.skeleton-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		flex: 1;
	}

	.skeleton-body {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border-radius: var(--radius-lg, 12px);
	}

	.skeleton-message.user .skeleton-body {
		background: var(--color-accent-light);
	}
</style>
