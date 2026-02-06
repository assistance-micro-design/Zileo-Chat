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
  Select Component
  A styled dropdown select with label and help text.

  @example
  <Select
    label="Provider"
    value={selectedProvider}
    options={[
      { value: 'mistral', label: 'Mistral' },
      { value: 'ollama', label: 'Ollama' }
    ]}
    onchange={(e) => selectedProvider = e.currentTarget.value}
  />
-->
<script lang="ts">
	/**
	 * Option type definition
	 */
	export interface SelectOption {
		/** Option value */
		value: string;
		/** Display label */
		label: string;
		/** Whether option is disabled */
		disabled?: boolean;
	}

	/**
	 * Select props
	 */
	interface Props {
		/** Current selected value */
		value?: string;
		/** Available options */
		options: SelectOption[];
		/** Label text */
		label?: string;
		/** Help text */
		help?: string;
		/** Placeholder option text */
		placeholder?: string;
		/** Whether the select is disabled */
		disabled?: boolean;
		/** Whether selection is required */
		required?: boolean;
		/** Unique identifier */
		id?: string;
		/** Change handler */
		onchange?: (event: Event & { currentTarget: HTMLSelectElement }) => void;
	}

	let {
		value = '',
		options,
		label,
		help,
		placeholder,
		disabled = false,
		required = false,
		id,
		onchange
	}: Props = $props();

	/**
	 * Generate stable ID - uses provided id or generates once on mount
	 * Using $derived to make the reactive relationship explicit
	 */
	const generatedId = `select-${Math.random().toString(36).slice(2, 9)}`;
	const selectId = $derived(id ?? generatedId);
</script>

<div class="form-group">
	{#if label}
		<label class="form-label" for={selectId}>
			{label}
			{#if required}
				<span class="required-mark" aria-hidden="true">*</span>
			{/if}
		</label>
	{/if}

	<select
		{value}
		{disabled}
		{required}
		id={selectId}
		class="form-select"
		{onchange}
		aria-describedby={help ? `${selectId}-help` : undefined}
	>
		{#if placeholder}
			<option value="" disabled>{placeholder}</option>
		{/if}
		{#each options as option (option.value)}
			<option value={option.value} disabled={option.disabled}>
				{option.label}
			</option>
		{/each}
	</select>

	{#if help}
		<span id="{selectId}-help" class="form-help">{help}</span>
	{/if}
</div>

<style>
	.required-mark {
		color: var(--color-error);
		margin-left: var(--spacing-xs);
	}

	/* Option styling for proper dark/light theme support */
	select option {
		background-color: var(--color-bg-primary);
		color: var(--color-text-primary);
		padding: var(--spacing-sm);
	}

	/* Ensure dark theme colors are applied */
	:global([data-theme='dark']) select option {
		background-color: var(--color-bg-primary);
		color: var(--color-text-primary);
	}
</style>
