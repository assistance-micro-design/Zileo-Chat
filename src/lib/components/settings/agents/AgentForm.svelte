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
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

AgentForm - Create/edit form for agent configuration.
Includes LLM settings, tool selection, MCP server selection, and system prompt.
-->

<script lang="ts">
	import { agentStore } from '$lib/stores/agents';
	import {
		loadServers,
		type MCPState,
		createInitialMCPState,
		setServers
	} from '$lib/stores/mcp';
	import {
		loadAllLLMData,
		getModelsByProvider,
		getModelByApiName,
		createInitialLLMState,
		setModels,
		setProviderSettings
	} from '$lib/stores/llm';
	import type { ProviderType, LLMState } from '$types/llm';
	import type { ProviderInfo } from '$types/customProvider';
	import type { AgentConfig, AgentConfigCreate, Lifecycle } from '$types/agent';
	import { Button, Input, Textarea, Card, Badge } from '$lib/components/ui';
	import { onMount } from 'svelte';
	import { i18n, t } from '$lib/i18n';

	/**
	 * Component props
	 */
	interface Props {
		/** Form mode (create or edit) */
		mode: 'create' | 'edit';
		/** Agent to edit (null for create mode) */
		agent: AgentConfig | null;
		/** Cancel callback */
		oncancel: () => void;
	}

	let { mode, agent, oncancel }: Props = $props();

	/** Form state */
	let name = $state('');
	let lifecycle = $state<Lifecycle>('permanent');
	let provider = $state('mistral');
	let model = $state('mistral-large-latest');
	let maxToolIterations = $state(50);
	let selectedTools = $state<string[]>([]);
	let selectedMcpServers = $state<string[]>([]);
	let systemPrompt = $state('');

	// Sync form state when agent prop changes (e.g., switching between edit targets)
	$effect(() => {
		name = agent?.name ?? '';
		lifecycle = agent?.lifecycle ?? 'permanent';
		provider = (agent?.llm.provider ?? 'mistral').toLowerCase();
		model = agent?.llm.model ?? 'mistral-large-latest';
		maxToolIterations = agent?.max_tool_iterations ?? 50;
		selectedTools = agent?.tools ?? [];
		selectedMcpServers = agent?.mcp_servers ?? [];
		systemPrompt = agent?.system_prompt ?? '';
		// Reset validation state when agent changes
		errors = {};
	});

	/** UI state */
	let saving = $state(false);
	let errors = $state<Record<string, string>>({});
	let mcpState = $state<MCPState>(createInitialMCPState());
	let llmState = $state<LLMState>(createInitialLLMState());
	let providerList = $state<ProviderInfo[]>([]);

	/** Available tools (from backend) - reactive to locale */
	const availableTools = $derived([
		{ value: 'MemoryTool', label: $i18n('agents_tool_memory'), description: $i18n('agents_tool_memory_desc') },
		{ value: 'TodoTool', label: $i18n('agents_tool_todo'), description: $i18n('agents_tool_todo_desc') },
		{ value: 'UserQuestionTool', label: $i18n('agents_tool_user_question'), description: $i18n('agents_tool_user_question_desc') },
		{ value: 'CalculatorTool', label: $i18n('agents_tool_calculator'), description: $i18n('agents_tool_calculator_desc') }
	]);

	/** Lifecycle options with descriptions - reactive to locale */
	const lifecycleOptions = $derived([
		{ value: 'permanent' as Lifecycle, label: $i18n('agents_lifecycle_permanent'), description: $i18n('agents_lifecycle_permanent_desc') },
		{ value: 'temporary' as Lifecycle, label: $i18n('agents_lifecycle_temporary'), description: $i18n('agents_lifecycle_temporary_desc') }
	]);

	/** Provider options with details - reactive to locale, includes custom providers */
	const providerOptions = $derived.by(() => {
		if (providerList.length > 0) {
			return providerList.map((p) => ({
				value: p.id,
				label: p.displayName,
				type: p.isCloud ? $i18n('llm_provider_cloud_api') : $i18n('agents_provider_ollama_type')
			}));
		}
		// Fallback when providerList hasn't loaded yet
		return [
			{ value: 'mistral', label: $i18n('agents_provider_mistral'), type: $i18n('agents_provider_mistral_type') },
			{ value: 'ollama', label: $i18n('agents_provider_ollama'), type: $i18n('agents_provider_ollama_type') }
		];
	});

	/**
	 * Converts provider name to ProviderType (lowercase)
	 */
	function toProviderType(providerName: string): ProviderType {
		return providerName.toLowerCase() as ProviderType;
	}

	/** Reactive model list based on selected provider (full model objects) */
	const availableModels = $derived.by(() => {
		const providerType = toProviderType(provider);
		return getModelsByProvider(llmState, providerType);
	});

	/** Selected model object (for auto-populating temperature/maxTokens) */
	const selectedModel = $derived.by(() => {
		const providerType = toProviderType(provider);
		return getModelByApiName(llmState, model, providerType);
	});

	/** Available MCP servers from store */
	const availableMcpServers = $derived(
		mcpState.servers.map((s) => ({
			value: s.name,
			label: s.name,
			description: s.description || $i18n('agents_mcp_no_description')
		}))
	);

	/**
	 * Loads MCP servers and LLM models on mount
	 */
	onMount(async () => {
		// Load MCP servers
		try {
			const servers = await loadServers();
			mcpState = setServers(mcpState, servers);
		} catch (err) {
			console.warn('[AgentForm] Failed to load MCP servers:', err);
			// MCP servers are optional - form still usable without them
		}

		// Load LLM models and provider list
		try {
			const data = await loadAllLLMData();
			providerList = data.providerList;
			for (const [providerId, provSettings] of Object.entries(data.settings)) {
				llmState = setProviderSettings(llmState, providerId, provSettings);
			}
			llmState = setModels(llmState, data.models);
		} catch (err) {
			console.warn('[AgentForm] Failed to load LLM models:', err);
			// Will show empty model list - user will see no-models message
		}
	});

	/**
	 * Updates model when provider changes (reset to first available if current invalid)
	 */
	$effect(() => {
		if (availableModels.length > 0) {
			const currentModelValid = availableModels.some((m) => m.api_name === model);
			if (!currentModelValid) {
				model = availableModels[0].api_name;
			}
		}
	});

	/**
	 * Validates form fields
	 */
	function validate(): boolean {
		errors = {};

		if (!name.trim() || name.length < 1 || name.length > 64) {
			errors.name = t('agents_name_error');
		}

		if (availableModels.length === 0) {
			errors.model = t('agents_no_models_error');
		} else if (!model) {
			errors.model = t('agents_model_required');
		} else if (!selectedModel) {
			errors.model = t('agents_model_not_found');
		}

		if (maxToolIterations < 1 || maxToolIterations > 200) {
			errors.maxToolIterations = t('agents_max_iterations_error');
		}

		if (!systemPrompt.trim()) {
			errors.systemPrompt = t('agents_system_prompt_required');
		} else if (systemPrompt.length > 10000) {
			errors.systemPrompt = t('agents_system_prompt_max');
		}

		return Object.keys(errors).length === 0;
	}

	/**
	 * Handles form submission
	 */
	async function handleSubmit(): Promise<void> {
		if (!validate()) return;
		if (!selectedModel) return;

		saving = true;

		const config: AgentConfigCreate = {
			name: name.trim(),
			lifecycle,
			llm: {
				provider,
				model,
				temperature: selectedModel.temperature_default,
				max_tokens: selectedModel.max_output_tokens
			},
			tools: selectedTools,
			mcp_servers: selectedMcpServers,
			system_prompt: systemPrompt.trim(),
			max_tool_iterations: maxToolIterations
		};

		try {
			if (mode === 'create') {
				await agentStore.createAgent(config);
			} else if (agent) {
				await agentStore.updateAgent(agent.id, config);
			}
		} catch {
			// Error handled by store
		} finally {
			saving = false;
		}
	}

	/**
	 * Toggles tool selection
	 */
	function toggleTool(toolValue: string): void {
		if (selectedTools.includes(toolValue)) {
			selectedTools = selectedTools.filter((t) => t !== toolValue);
		} else {
			selectedTools = [...selectedTools, toolValue];
		}
	}

	/**
	 * Toggles MCP server selection
	 */
	function toggleMcpServer(serverName: string): void {
		if (selectedMcpServers.includes(serverName)) {
			selectedMcpServers = selectedMcpServers.filter((s) => s !== serverName);
		} else {
			selectedMcpServers = [...selectedMcpServers, serverName];
		}
	}

	/**
	 * Formats context window for display (e.g., "128K" for 128000)
	 */
	function formatContextWindow(tokens: number): string {
		if (tokens >= 1_000_000) {
			return `${(tokens / 1_000_000).toFixed(1)}M`;
		}
		if (tokens >= 1_000) {
			return `${Math.round(tokens / 1_000)}K`;
		}
		return tokens.toLocaleString();
	}

	/**
	 * Handles max tool iterations input
	 */
	function handleMaxToolIterationsInput(event: Event & { currentTarget: HTMLInputElement }): void {
		maxToolIterations = parseInt(event.currentTarget.value, 10) || 50;
	}

	/**
	 * Handles system prompt input
	 */
	function handleSystemPromptInput(event: Event & { currentTarget: HTMLTextAreaElement }): void {
		systemPrompt = event.currentTarget.value;
	}
</script>

<Card>
	{#snippet body()}
		<form class="agent-form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
			<h3 class="form-title">
				{mode === 'create' ? $i18n('agents_create_new') : $i18n('agents_edit')}
			</h3>

			<div class="form-grid">
				<!-- Basic Information -->
				<div class="form-section">
					<h4 class="section-title">{$i18n('agents_basic_info')}</h4>

					<Input
						label={$i18n('agents_name_label')}
						value={name}
						oninput={(e) => { name = e.currentTarget.value; }}
						placeholder={$i18n('agents_name_placeholder')}
						required
						help={errors.name || $i18n('agents_name_help')}
					/>

					<div class="field-group" role="group" aria-label={$i18n('agents_lifecycle')}>
						<span class="field-label">{$i18n('agents_lifecycle')}</span>
						<div class="card-selector">
							{#each lifecycleOptions as option (option.value)}
								<button
									type="button"
									class="selector-card"
									class:selected={lifecycle === option.value}
									class:disabled={mode === 'edit'}
									disabled={mode === 'edit'}
									onclick={() => { if (mode !== 'edit') lifecycle = option.value; }}
								>
									<span class="selector-card-title">{option.label}</span>
									<span class="selector-card-description">{option.description}</span>
								</button>
							{/each}
						</div>
						{#if mode === 'edit'}
							<span class="field-help">{$i18n('agents_lifecycle_readonly')}</span>
						{/if}
					</div>
				</div>

				<!-- LLM Configuration -->
				<div class="form-section">
					<h4 class="section-title">{$i18n('agents_llm_config')}</h4>

					<div class="field-group" role="group" aria-label={$i18n('agents_provider')}>
						<span class="field-label">{$i18n('agents_provider')}</span>
						<div class="card-selector">
							{#each providerOptions as option (option.value)}
								<button
									type="button"
									class="selector-card provider-card"
									class:selected={provider === option.value}
									onclick={() => { provider = option.value; }}
								>
									<span class="selector-card-title">{option.label}</span>
									<span class="selector-card-type">{option.type}</span>
								</button>
							{/each}
						</div>
					</div>

					<div class="field-group" role="group" aria-label={$i18n('agents_model')}>
						<span class="field-label">{$i18n('agents_model')}</span>
						{#if availableModels.length === 0}
							<div class="no-models-message">
								<p>{$i18n('agents_no_models', { provider })}</p>
								<p>{$i18n('agents_no_models_hint')}</p>
							</div>
						{:else}
							<div class="model-selector">
								{#each availableModels as m (m.api_name)}
									<button
										type="button"
										class="model-card"
										class:selected={model === m.api_name}
										onclick={() => { model = m.api_name; }}
									>
										<div class="model-card-header">
											<span class="model-card-name">{m.name}</span>
											<div class="model-card-badges">
												{#if m.is_builtin}
													<Badge variant="primary">{$i18n('agents_model_builtin')}</Badge>
												{/if}
												{#if m.is_reasoning}
													<Badge variant="warning">{$i18n('agents_model_reasoning')}</Badge>
												{/if}
											</div>
										</div>
										<code class="model-card-api">{m.api_name}</code>
										<div class="model-card-specs">
											<span class="model-card-spec">{formatContextWindow(m.context_window)} ctx</span>
											<span class="model-card-spec">{formatContextWindow(m.max_output_tokens)} out</span>
											<span class="model-card-spec">T: {m.temperature_default}</span>
										</div>
									</button>
								{/each}
							</div>
							{#if errors.model}
								<span class="field-error">{errors.model}</span>
							{/if}
						{/if}
					</div>

					<Input
						type="number"
						label={$i18n('agents_max_iterations_label')}
						value={String(maxToolIterations)}
						oninput={handleMaxToolIterationsInput}
						help={errors.maxToolIterations || $i18n('agents_max_iterations_help')}
					/>
				</div>

				<!-- Tools -->
				<div class="form-section">
					<h4 class="section-title">{$i18n('agents_tools_section')}</h4>
					<p class="section-help">{$i18n('agents_tools_help')}</p>

					<div class="checkbox-group">
						{#each availableTools as tool (tool.value)}
							<label class="checkbox-item">
								<input
									type="checkbox"
									checked={selectedTools.includes(tool.value)}
									onchange={() => toggleTool(tool.value)}
								/>
								<div class="checkbox-content">
									<span class="checkbox-label">{tool.label}</span>
									<span class="checkbox-description">{tool.description}</span>
								</div>
							</label>
						{/each}
					</div>
				</div>

				<!-- MCP Servers -->
				<div class="form-section">
					<h4 class="section-title">{$i18n('agents_mcp_section')}</h4>
					<p class="section-help">{$i18n('agents_mcp_help')}</p>

					{#if availableMcpServers.length === 0}
						<p class="no-servers">
							{$i18n('agents_mcp_none')}
						</p>
					{:else}
						<div class="checkbox-group">
							{#each availableMcpServers as server (server.value)}
								<label class="checkbox-item">
									<input
										type="checkbox"
										checked={selectedMcpServers.includes(server.value)}
										onchange={() => toggleMcpServer(server.value)}
									/>
									<div class="checkbox-content">
										<span class="checkbox-label">{server.label}</span>
										<span class="checkbox-description">{server.description}</span>
									</div>
								</label>
							{/each}
						</div>
					{/if}
				</div>

				<!-- System Prompt -->
				<div class="form-section full-width">
					<h4 class="section-title">{$i18n('agents_system_prompt')}</h4>

					<Textarea
						label={$i18n('agents_system_prompt_label')}
						value={systemPrompt}
						oninput={handleSystemPromptInput}
						rows={8}
						placeholder={$i18n('agents_system_prompt_placeholder')}
						required
						help={errors.systemPrompt || $i18n('agents_system_prompt_chars', { count: systemPrompt.length })}
					/>
				</div>
			</div>

			<div class="form-actions">
				<Button variant="ghost" type="button" onclick={oncancel} disabled={saving}>
					{$i18n('common_cancel')}
				</Button>
				<Button variant="primary" type="submit" disabled={saving}>
					{saving ? $i18n('agents_saving') : mode === 'create' ? $i18n('agents_create') : $i18n('agents_save_changes')}
				</Button>
			</div>
		</form>
	{/snippet}
</Card>

<style>
	.agent-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.form-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.form-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-xl);
	}

	.form-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.form-section.full-width {
		grid-column: 1 / -1;
	}

	.section-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		margin: 0;
	}

	.section-help {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
	}

	/* Field Group */
	.field-group {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.field-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.field-help {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.field-error {
		font-size: var(--font-size-xs);
		color: var(--color-error);
	}

	/* Card Selector (Provider, Lifecycle) */
	.card-selector {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-sm);
	}

	.selector-card {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--spacing-xs);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border: 2px solid var(--color-border);
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: border-color var(--transition-fast), background-color var(--transition-fast);
		text-align: left;
	}

	.selector-card:hover:not(.disabled) {
		border-color: var(--color-primary);
		background: var(--color-bg-hover);
	}

	.selector-card.selected {
		border-color: var(--color-primary);
		background: color-mix(in srgb, var(--color-primary) 10%, transparent);
	}

	.selector-card.disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.selector-card-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.selector-card-description,
	.selector-card-type {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	/* Model Selector */
	.model-selector {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
		max-height: 300px;
		overflow-y: auto;
	}

	.model-card {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border: 2px solid var(--color-border);
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: border-color var(--transition-fast), background-color var(--transition-fast);
		text-align: left;
	}

	.model-card:hover {
		border-color: var(--color-primary);
		background: var(--color-bg-hover);
	}

	.model-card.selected {
		border-color: var(--color-primary);
		background: color-mix(in srgb, var(--color-primary) 10%, transparent);
	}

	.model-card-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-sm);
	}

	.model-card-name {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.model-card-badges {
		display: flex;
		gap: var(--spacing-xs);
	}

	.model-card-api {
		font-size: var(--font-size-xs);
		font-family: var(--font-mono);
		color: var(--color-text-tertiary);
		background: var(--color-bg-tertiary);
		padding: 2px var(--spacing-xs);
		border-radius: var(--border-radius-sm);
	}

	.model-card-specs {
		display: flex;
		gap: var(--spacing-md);
		margin-top: var(--spacing-xs);
	}

	.model-card-spec {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	/* Checkbox Group (Tools, MCP Servers) */
	.checkbox-group {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.checkbox-item {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
		cursor: pointer;
		padding: var(--spacing-sm);
		border-radius: var(--border-radius-md);
		transition: background var(--transition-fast);
	}

	.checkbox-item:hover {
		background: var(--color-bg-hover);
	}

	.checkbox-item input {
		width: 16px;
		height: 16px;
		margin-top: 2px;
		cursor: pointer;
	}

	.checkbox-content {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.checkbox-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
	}

	.checkbox-description {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	/* Empty States */
	.no-servers,
	.no-models-message {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-style: italic;
		margin: 0;
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
	}

	.no-models-message p {
		margin: 0;
	}

	.no-models-message p + p {
		margin-top: var(--spacing-xs);
	}

	/* Form Actions */
	.form-actions {
		display: flex;
		gap: var(--spacing-md);
		justify-content: flex-end;
		padding-top: var(--spacing-lg);
		border-top: 1px solid var(--color-border);
	}

	/* Responsive */
	@media (max-width: 768px) {
		.form-grid {
			grid-template-columns: 1fr;
		}

		.card-selector {
			grid-template-columns: 1fr;
		}
	}
</style>
