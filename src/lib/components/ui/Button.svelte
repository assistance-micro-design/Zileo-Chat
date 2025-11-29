<!--
  Button Component
  A versatile button component with multiple variants and sizes.

  @example
  <Button variant="primary" onclick={() => handleClick()}>Click me</Button>
  <Button variant="secondary" size="lg">Large Button</Button>
  <Button variant="ghost" disabled>Disabled</Button>
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	/**
	 * Button component props
	 */
	interface Props {
		/** Visual variant of the button */
		variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
		/** Size of the button */
		size?: 'sm' | 'md' | 'lg' | 'icon';
		/** Whether the button is disabled */
		disabled?: boolean;
		/** HTML button type */
		type?: 'button' | 'submit' | 'reset';
		/** Click event handler */
		onclick?: () => void;
		/** Accessible label for icon-only buttons */
		ariaLabel?: string;
		/** Tooltip text shown on hover */
		title?: string;
		/** Button content */
		children: Snippet;
	}

	let {
		variant = 'primary',
		size = 'md',
		disabled = false,
		type = 'button',
		onclick,
		ariaLabel,
		title,
		children
	}: Props = $props();

	/**
	 * Compute CSS classes based on props
	 */
	const classes = $derived(
		['btn', `btn-${variant}`, size !== 'md' ? `btn-${size}` : ''].filter(Boolean).join(' ')
	);
</script>

<button {type} {disabled} class={classes} {onclick} aria-label={ariaLabel} aria-disabled={disabled} {title}>
	{@render children()}
</button>
