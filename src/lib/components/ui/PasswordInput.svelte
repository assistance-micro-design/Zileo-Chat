<!--
  Copyright 2025 Assistance Micro Design

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0
-->

<!--
  PasswordInput Component (v1.2)
  A password input with a "show / hide" eye toggle. Wraps the native
  password input with the same form styling as Input.svelte.

  @example
  <PasswordInput
    label="Bearer token"
    bind:value={token}
    placeholder="Enter the API token"
    help="The token is stored in the OS keychain"
  />
-->
<script lang="ts">
	import { Eye, EyeOff } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	interface Props {
		/** Current value (bindable). */
		value?: string;
		/** Placeholder shown when empty. */
		placeholder?: string;
		/** Optional label rendered above the input. */
		label?: string;
		/** Optional help text rendered under the input. */
		help?: string;
		/** Disable the field. */
		disabled?: boolean;
		/** Mark the field as required (visual + form). */
		required?: boolean;
		/** Optional explicit id (otherwise auto-generated). */
		id?: string;
		/** Optional inline error message (red text under the field). */
		error?: string;
		/** Change handler (raw input event). */
		oninput?: (event: Event & { currentTarget: HTMLInputElement }) => void;
		/** Blur handler (used to trigger live validation). */
		onblur?: (event: FocusEvent) => void;
	}

	let {
		value = $bindable(''),
		placeholder = '',
		label,
		help,
		disabled = false,
		required = false,
		id,
		error,
		oninput,
		onblur
	}: Props = $props();

	const generatedId = `password-${crypto.randomUUID()}`;
	const inputId = $derived(id ?? generatedId);

	let revealed = $state(false);
	const inputType = $derived(revealed ? 'text' : 'password');
	const toggleLabel = $derived(
		revealed ? $i18n('common_hide_password') : $i18n('common_show_password')
	);

	function toggleReveal() {
		revealed = !revealed;
	}
</script>

<div class="form-group">
	{#if label}
		<label class="form-label" for={inputId}>
			{label}
			{#if required}
				<span class="required-mark" aria-hidden="true">*</span>
			{/if}
		</label>
	{/if}

	<div class="password-wrapper" class:has-error={!!error}>
		<input
			type={inputType}
			bind:value
			{placeholder}
			{disabled}
			{required}
			id={inputId}
			class="form-input password-input"
			autocomplete="off"
			spellcheck="false"
			aria-describedby={help || error ? `${inputId}-help` : undefined}
			aria-invalid={error ? 'true' : undefined}
			{oninput}
			{onblur}
		/>
		<button
			type="button"
			class="reveal-toggle"
			onclick={toggleReveal}
			{disabled}
			aria-label={toggleLabel}
			aria-pressed={revealed}
		>
			{#if revealed}
				<EyeOff size={16} aria-hidden="true" />
			{:else}
				<Eye size={16} aria-hidden="true" />
			{/if}
		</button>
	</div>

	{#if error}
		<span id="{inputId}-help" class="form-error">{error}</span>
	{:else if help}
		<span id="{inputId}-help" class="form-help">{help}</span>
	{/if}
</div>

<style>
	.required-mark {
		color: var(--color-error);
		margin-left: var(--spacing-xs);
	}

	.password-wrapper {
		position: relative;
		display: flex;
		align-items: center;
	}

	.password-input {
		flex: 1;
		padding-right: calc(var(--spacing-lg) + 24px);
	}

	.reveal-toggle {
		position: absolute;
		right: var(--spacing-xs);
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		border-radius: var(--border-radius-sm);
		transition:
			color 0.15s,
			background-color 0.15s;
	}

	.reveal-toggle:hover:not(:disabled) {
		color: var(--color-text-primary);
		background-color: var(--color-bg-secondary);
	}

	.reveal-toggle:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}

	.reveal-toggle:disabled {
		cursor: not-allowed;
		opacity: 0.5;
	}

	.password-wrapper.has-error :global(.password-input) {
		border-color: var(--color-error);
	}

	.form-error {
		font-size: var(--font-size-sm);
		color: var(--color-error);
		display: block;
		margin-top: var(--spacing-xs);
	}
</style>
