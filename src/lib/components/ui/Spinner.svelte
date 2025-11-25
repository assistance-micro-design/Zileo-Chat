<!--
  Spinner Component
  A loading spinner with configurable size.

  @example
  <Spinner />
  <Spinner size="lg" />
  <Spinner size={32} />
-->
<script lang="ts">
	/**
	 * Spinner props
	 */
	interface Props {
		/** Size preset or pixel value */
		size?: 'sm' | 'md' | 'lg' | number;
		/** Accessible label */
		label?: string;
	}

	let { size = 'md', label = 'Loading' }: Props = $props();

	/**
	 * Compute actual size in pixels
	 */
	const pixelSize = $derived(
		typeof size === 'number' ? size : size === 'sm' ? 16 : size === 'lg' ? 32 : 20
	);
</script>

<span
	class="spinner"
	style="width: {pixelSize}px; height: {pixelSize}px;"
	role="status"
	aria-label={label}
>
	<span class="sr-only">{label}</span>
</span>

<style>
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}
</style>
