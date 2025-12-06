<!--
  Copyright 2025 Assistance Micro Design

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

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
