<!--
  Input Component
  A styled text input with label and help text support.

  @example
  <Input
    label="API Key"
    type="password"
    value={apiKey}
    placeholder="Enter your API key"
    oninput={(e) => apiKey = e.currentTarget.value}
    help="Your API key is stored securely"
  />
-->
<script lang="ts">
	/**
	 * Input props
	 */
	interface Props {
		/** Input type */
		type?: 'text' | 'password' | 'email' | 'number' | 'search' | 'url';
		/** Current value */
		value?: string;
		/** Placeholder text */
		placeholder?: string;
		/** Label text */
		label?: string;
		/** Help text */
		help?: string;
		/** Whether the input is disabled */
		disabled?: boolean;
		/** Whether the input is required */
		required?: boolean;
		/** Unique identifier */
		id?: string;
		/** Input handler */
		oninput?: (event: Event & { currentTarget: HTMLInputElement }) => void;
		/** Change handler */
		onchange?: (event: Event & { currentTarget: HTMLInputElement }) => void;
		/** Focus handler */
		onfocus?: (event: FocusEvent) => void;
		/** Blur handler */
		onblur?: (event: FocusEvent) => void;
	}

	let {
		type = 'text',
		value = '',
		placeholder = '',
		label,
		help,
		disabled = false,
		required = false,
		id,
		oninput,
		onchange,
		onfocus,
		onblur
	}: Props = $props();

	/**
	 * Generate unique ID if not provided
	 */
	const inputId = id ?? `input-${Math.random().toString(36).slice(2, 9)}`;
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

	<input
		{type}
		{value}
		{placeholder}
		{disabled}
		{required}
		id={inputId}
		class="form-input"
		{oninput}
		{onchange}
		{onfocus}
		{onblur}
		aria-describedby={help ? `${inputId}-help` : undefined}
	/>

	{#if help}
		<span id="{inputId}-help" class="form-help">{help}</span>
	{/if}
</div>

<style>
	.required-mark {
		color: var(--color-error);
		margin-left: var(--spacing-xs);
	}
</style>
