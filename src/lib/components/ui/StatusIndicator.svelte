<!--
  StatusIndicator Component
  A small dot indicator showing workflow/process status.
  Includes animation for running state.

  @example
  <StatusIndicator status="idle" />
  <StatusIndicator status="running" />
  <StatusIndicator status="completed" />
  <StatusIndicator status="error" />
-->
<script lang="ts">
	/**
	 * Possible status states
	 */
	export type Status = 'idle' | 'running' | 'completed' | 'error';

	/**
	 * StatusIndicator props
	 */
	interface Props {
		/** Current status */
		status: Status;
		/** Optional size modifier */
		size?: 'sm' | 'md' | 'lg';
	}

	let { status, size = 'md' }: Props = $props();

	/**
	 * Get aria label for accessibility
	 */
	const ariaLabel = $derived(`Status: ${status}`);
</script>

<span
	class="status-indicator status-{status}"
	class:status-sm={size === 'sm'}
	class:status-lg={size === 'lg'}
	role="status"
	aria-label={ariaLabel}
></span>

<style>
	.status-sm {
		width: 6px;
		height: 6px;
	}

	.status-lg {
		width: 12px;
		height: 12px;
	}
</style>
