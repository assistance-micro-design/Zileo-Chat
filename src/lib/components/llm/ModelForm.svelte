<!--
  ModelForm Component
  Form for creating or editing LLM models.
  Supports full validation and handles both create and edit modes.

  @example
  <ModelForm
    mode="create"
    provider="mistral"
    onsubmit={(data) => handleCreate(data)}
    oncancel={() => closeModal()}
  />

  <ModelForm
    mode="edit"
    model={existingModel}
    onsubmit={(data) => handleUpdate(model.id, data)}
    oncancel={() => closeModal()}
    saving={isSaving}
  />
-->
<script lang="ts">
	import { Input, Select, Button } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui';
	import type { CreateModelRequest, UpdateModelRequest, LLMModel, ProviderType } from '$types/llm';

	/**
	 * ModelForm props
	 */
	interface Props {
		/** Form mode - create for new models, edit for existing */
		mode: 'create' | 'edit';
		/** Existing model data (required for edit mode) */
		model?: LLMModel;
		/** Default provider for create mode */
		provider?: ProviderType;
		/** Callback when form is submitted with valid data */
		onsubmit: (data: CreateModelRequest | UpdateModelRequest) => void;
		/** Callback when form is cancelled */
		oncancel: () => void;
		/** Whether a save operation is in progress */
		saving?: boolean;
	}

	let { mode, model, provider, onsubmit, oncancel, saving = false }: Props = $props();

	/** Provider options for the select dropdown */
	const providerOptions: SelectOption[] = [
		{ value: 'mistral', label: 'Mistral' },
		{ value: 'ollama', label: 'Ollama' }
	];

	/** Form data state */
	let formData = $state({
		provider: (model?.provider ?? provider ?? 'mistral') as ProviderType,
		name: model?.name ?? '',
		api_name: model?.api_name ?? '',
		context_window: model?.context_window ?? 32000,
		max_output_tokens: model?.max_output_tokens ?? 4096,
		temperature_default: model?.temperature_default ?? 0.7
	});

	/** Validation errors state */
	let errors = $state<Record<string, string>>({});

	/** Whether the form has been touched (submitted at least once) */
	let touched = $state(false);

	/**
	 * Validates the form data.
	 * @returns true if all validations pass
	 */
	function validate(): boolean {
		const newErrors: Record<string, string> = {};

		// Name validation
		const trimmedName = formData.name.trim();
		if (!trimmedName) {
			newErrors.name = 'Name is required';
		} else if (trimmedName.length > 64) {
			newErrors.name = 'Name must be 64 characters or less';
		}

		// API name validation
		const trimmedApiName = formData.api_name.trim();
		if (!trimmedApiName) {
			newErrors.api_name = 'API name is required';
		} else if (trimmedApiName.length > 128) {
			newErrors.api_name = 'API name must be 128 characters or less';
		} else if (!/^[a-zA-Z0-9._\-/:]+$/.test(trimmedApiName)) {
			newErrors.api_name =
				'API name can only contain letters, numbers, dots, hyphens, underscores, slashes, and colons';
		}

		// Context window validation
		if (formData.context_window < 1024) {
			newErrors.context_window = 'Minimum 1,024 tokens';
		} else if (formData.context_window > 2000000) {
			newErrors.context_window = 'Maximum 2,000,000 tokens';
		}

		// Max output tokens validation
		if (formData.max_output_tokens < 256) {
			newErrors.max_output_tokens = 'Minimum 256 tokens';
		} else if (formData.max_output_tokens > 128000) {
			newErrors.max_output_tokens = 'Maximum 128,000 tokens';
		}

		// Temperature validation
		if (formData.temperature_default < 0) {
			newErrors.temperature_default = 'Minimum is 0';
		} else if (formData.temperature_default > 2) {
			newErrors.temperature_default = 'Maximum is 2';
		}

		errors = newErrors;
		return Object.keys(newErrors).length === 0;
	}

	/**
	 * Handles form submission.
	 * Validates data before calling onsubmit.
	 */
	function handleSubmit(): void {
		touched = true;

		if (!validate()) {
			return;
		}

		if (mode === 'create') {
			const createData: CreateModelRequest = {
				provider: formData.provider,
				name: formData.name.trim(),
				api_name: formData.api_name.trim(),
				context_window: formData.context_window,
				max_output_tokens: formData.max_output_tokens,
				temperature_default: formData.temperature_default
			};
			onsubmit(createData);
		} else {
			// For edit mode, only include changed fields
			const updateData: UpdateModelRequest = {};

			if (model && formData.name.trim() !== model.name) {
				updateData.name = formData.name.trim();
			}
			if (model && formData.api_name.trim() !== model.api_name) {
				updateData.api_name = formData.api_name.trim();
			}
			if (model && formData.context_window !== model.context_window) {
				updateData.context_window = formData.context_window;
			}
			if (model && formData.max_output_tokens !== model.max_output_tokens) {
				updateData.max_output_tokens = formData.max_output_tokens;
			}
			if (model && formData.temperature_default !== model.temperature_default) {
				updateData.temperature_default = formData.temperature_default;
			}

			onsubmit(updateData);
		}
	}

	/**
	 * Handles provider change in create mode
	 */
	function handleProviderChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		formData.provider = event.currentTarget.value as ProviderType;
	}

	/**
	 * Handles number input change with proper conversion
	 */
	function handleNumberInput(
		field: 'context_window' | 'max_output_tokens' | 'temperature_default',
		event: Event & { currentTarget: HTMLInputElement }
	): void {
		const value = parseFloat(event.currentTarget.value);
		if (!isNaN(value)) {
			if (field === 'temperature_default') {
				formData[field] = value;
			} else {
				formData[field] = Math.floor(value);
			}
		}
	}

	/** Whether the model is builtin (edit restrictions apply) */
	const isBuiltin = $derived(model?.is_builtin ?? false);

	/** Form title based on mode */
	const formTitle = $derived(mode === 'create' ? 'Create Custom Model' : 'Edit Model');

	/** Submit button text */
	const submitText = $derived(saving ? 'Saving...' : mode === 'create' ? 'Create Model' : 'Save Changes');
</script>

<form class="model-form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
	<h3 class="form-title">{formTitle}</h3>

	{#if isBuiltin}
		<div class="builtin-notice">
			<p>This is a builtin model. Only the default temperature can be modified.</p>
		</div>
	{/if}

	{#if mode === 'create'}
		<Select
			label="Provider"
			value={formData.provider}
			options={providerOptions}
			onchange={handleProviderChange}
			required
		/>
	{/if}

	<Input
		label="Display Name"
		bind:value={formData.name}
		placeholder="e.g., My Custom Model"
		help="Human-readable name for the model"
		disabled={isBuiltin || saving}
		required
	/>
	{#if touched && errors.name}
		<span class="error-text">{errors.name}</span>
	{/if}

	<Input
		label="API Name"
		bind:value={formData.api_name}
		placeholder="e.g., my-custom-model"
		help="Model identifier used in API calls"
		disabled={isBuiltin || saving}
		required
	/>
	{#if touched && errors.api_name}
		<span class="error-text">{errors.api_name}</span>
	{/if}

	<div class="form-row">
		<div class="form-field">
			<Input
				label="Context Window"
				type="number"
				value={formData.context_window.toString()}
				oninput={(e) => handleNumberInput('context_window', e)}
				help="Max context length (tokens)"
				disabled={isBuiltin || saving}
			/>
			{#if touched && errors.context_window}
				<span class="error-text">{errors.context_window}</span>
			{/if}
		</div>

		<div class="form-field">
			<Input
				label="Max Output Tokens"
				type="number"
				value={formData.max_output_tokens.toString()}
				oninput={(e) => handleNumberInput('max_output_tokens', e)}
				help="Max generation length"
				disabled={isBuiltin || saving}
			/>
			{#if touched && errors.max_output_tokens}
				<span class="error-text">{errors.max_output_tokens}</span>
			{/if}
		</div>
	</div>

	<Input
		label="Default Temperature"
		type="number"
		value={formData.temperature_default.toString()}
		oninput={(e) => handleNumberInput('temperature_default', e)}
		help="Sampling temperature (0.0 - 2.0)"
		disabled={saving}
	/>
	{#if touched && errors.temperature_default}
		<span class="error-text">{errors.temperature_default}</span>
	{/if}

	<div class="form-actions">
		<Button variant="ghost" onclick={oncancel} disabled={saving}>
			Cancel
		</Button>
		<Button variant="primary" type="submit" disabled={saving}>
			{submitText}
		</Button>
	</div>
</form>

<style>
	.model-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		max-width: 500px;
	}

	.form-title {
		margin: 0 0 var(--spacing-sm) 0;
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.builtin-notice {
		padding: var(--spacing-sm) var(--spacing-md);
		background-color: var(--color-warning-bg);
		border-radius: var(--radius-md);
		border-left: 3px solid var(--color-warning);
	}

	.builtin-notice p {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-warning);
	}

	.form-row {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--spacing-md);
	}

	.form-field {
		display: flex;
		flex-direction: column;
	}

	.error-text {
		font-size: var(--font-size-sm);
		color: var(--color-error);
		margin-top: calc(-1 * var(--spacing-xs));
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-sm);
		margin-top: var(--spacing-md);
		padding-top: var(--spacing-md);
		border-top: 1px solid var(--color-border);
	}

	/* Responsive: stack form row on small screens */
	@media (max-width: 480px) {
		.form-row {
			grid-template-columns: 1fr;
		}
	}
</style>
