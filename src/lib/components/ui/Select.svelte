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
	 * Generate unique ID if not provided
	 */
	const selectId = id ?? `select-${Math.random().toString(36).slice(2, 9)}`;
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
		{#each options as option}
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
</style>
