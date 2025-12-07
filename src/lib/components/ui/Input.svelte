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
		/** Step increment for number inputs (use "any" for decimals) */
		step?: string | number;
		/** Minimum value for number inputs */
		min?: string | number;
		/** Maximum value for number inputs */
		max?: string | number;
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
		value = $bindable(''),
		placeholder = '',
		label,
		help,
		disabled = false,
		required = false,
		id,
		step,
		min,
		max,
		oninput,
		onchange,
		onfocus,
		onblur
	}: Props = $props();

	/**
	 * Generate stable ID - uses provided id or generates once on mount
	 * Using $derived to make the reactive relationship explicit
	 */
	const generatedId = `input-${Math.random().toString(36).slice(2, 9)}`;
	const inputId = $derived(id ?? generatedId);
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
		bind:value
		{placeholder}
		{disabled}
		{required}
		{step}
		{min}
		{max}
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
