<!--
  Textarea Component
  A styled multi-line text input with label and help text.

  @example
  <Textarea
    label="System Prompt"
    value={systemPrompt}
    placeholder="Enter system prompt..."
    rows={6}
    oninput={(e) => systemPrompt = e.currentTarget.value}
  />
-->
<script lang="ts">
	/**
	 * Textarea props
	 */
	interface Props {
		/** Current value */
		value?: string;
		/** Placeholder text */
		placeholder?: string;
		/** Label text */
		label?: string;
		/** Help text */
		help?: string;
		/** Number of visible rows */
		rows?: number;
		/** Whether the textarea is disabled */
		disabled?: boolean;
		/** Whether input is required */
		required?: boolean;
		/** Unique identifier */
		id?: string;
		/** Input handler */
		oninput?: (event: Event & { currentTarget: HTMLTextAreaElement }) => void;
		/** Change handler */
		onchange?: (event: Event & { currentTarget: HTMLTextAreaElement }) => void;
	}

	let {
		value = '',
		placeholder = '',
		label,
		help,
		rows = 4,
		disabled = false,
		required = false,
		id,
		oninput,
		onchange
	}: Props = $props();

	/**
	 * Generate unique ID if not provided
	 */
	const textareaId = id ?? `textarea-${Math.random().toString(36).slice(2, 9)}`;
</script>

<div class="form-group">
	{#if label}
		<label class="form-label" for={textareaId}>
			{label}
			{#if required}
				<span class="required-mark" aria-hidden="true">*</span>
			{/if}
		</label>
	{/if}

	<textarea
		{value}
		{placeholder}
		{disabled}
		{required}
		{rows}
		id={textareaId}
		class="form-textarea"
		{oninput}
		{onchange}
		aria-describedby={help ? `${textareaId}-help` : undefined}
	></textarea>

	{#if help}
		<span id="{textareaId}-help" class="form-help">{help}</span>
	{/if}
</div>

<style>
	.required-mark {
		color: var(--color-error);
		margin-left: var(--spacing-xs);
	}
</style>
