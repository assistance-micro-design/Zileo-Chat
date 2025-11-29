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
	import type { AgentConfig, AgentConfigCreate, Lifecycle } from '$types/agent';
	import { Button, Input, Textarea, Card, Badge } from '$lib/components/ui';
	import { onMount } from 'svelte';

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
	let name = $state(agent?.name ?? '');
	let lifecycle = $state<Lifecycle>(agent?.lifecycle ?? 'permanent');
	let provider = $state(agent?.llm.provider ?? 'Mistral');
	let model = $state(agent?.llm.model ?? 'mistral-large-latest');
	let maxToolIterations = $state(agent?.max_tool_iterations ?? 50);
	let selectedTools = $state<string[]>(agent?.tools ?? []);
	let selectedMcpServers = $state<string[]>(agent?.mcp_servers ?? []);
	let systemPrompt = $state(agent?.system_prompt ?? '');

	/** UI state */
	let saving = $state(false);
	let errors = $state<Record<string, string>>({});
	let mcpState = $state<MCPState>(createInitialMCPState());
	let llmState = $state<LLMState>(createInitialLLMState());

	/** Available tools (from backend) */
	const availableTools = [
		{ value: 'MemoryTool', label: 'Memory Tool', description: 'Store and retrieve persistent memories' },
		{ value: 'TodoTool', label: 'Todo Tool', description: 'Manage task lists and track progress' }
	];

	/** Lifecycle options with descriptions */
	const lifecycleOptions = [
		{ value: 'permanent' as Lifecycle, label: 'Permanent', description: 'Persists across sessions' },
		{ value: 'temporary' as Lifecycle, label: 'Temporary', description: 'Deleted after session ends' }
	];

	/** Provider options with details */
	const providerOptions = [
		{ value: 'Mistral', label: 'Mistral', type: 'Cloud API' },
		{ value: 'Ollama', label: 'Ollama', type: 'Local Server' }
	];

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
			description: s.description || 'No description'
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
		} catch {
			// Silently fail - MCP servers are optional
		}

		// Load LLM models
		try {
			const data = await loadAllLLMData();
			llmState = setProviderSettings(llmState, 'mistral', data.mistral);
			llmState = setProviderSettings(llmState, 'ollama', data.ollama);
			llmState = setModels(llmState, data.models);
		} catch {
			// Silently fail - will show empty model list
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
			errors.name = 'Name must be 1-64 characters';
		}

		if (availableModels.length === 0) {
			errors.model = 'No models available - add models in Models section first';
		} else if (!model) {
			errors.model = 'Model is required';
		} else if (!selectedModel) {
			errors.model = 'Selected model not found';
		}

		if (maxToolIterations < 1 || maxToolIterations > 200) {
			errors.maxToolIterations = 'Max iterations must be between 1 and 200';
		}

		if (!systemPrompt.trim()) {
			errors.systemPrompt = 'System prompt is required';
		} else if (systemPrompt.length > 10000) {
			errors.systemPrompt = 'System prompt must be under 10000 characters';
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
				{mode === 'create' ? 'Create New Agent' : 'Edit Agent'}
			</h3>

			<div class="form-grid">
				<!-- Basic Information -->
				<div class="form-section">
					<h4 class="section-title">Basic Information</h4>

					<Input
						label="Agent Name"
						value={name}
						oninput={(e) => { name = e.currentTarget.value; }}
						placeholder="My Custom Agent"
						required
						help={errors.name || 'A unique name for this agent (1-64 characters)'}
					/>

					<div class="field-group" role="group" aria-label="Lifecycle">
						<span class="field-label">Lifecycle</span>
						<div class="card-selector">
							{#each lifecycleOptions as option}
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
							<span class="field-help">Lifecycle cannot be changed after creation</span>
						{/if}
					</div>
				</div>

				<!-- LLM Configuration -->
				<div class="form-section">
					<h4 class="section-title">LLM Configuration</h4>

					<div class="field-group" role="group" aria-label="Provider">
						<span class="field-label">Provider</span>
						<div class="card-selector">
							{#each providerOptions as option}
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

					<div class="field-group" role="group" aria-label="Model">
						<span class="field-label">Model</span>
						{#if availableModels.length === 0}
							<div class="no-models-message">
								<p>No models configured for {provider}.</p>
								<p>Add models in the Models section above.</p>
							</div>
						{:else}
							<div class="model-selector">
								{#each availableModels as m}
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
													<Badge variant="primary">Builtin</Badge>
												{/if}
												{#if m.is_reasoning}
													<Badge variant="warning">Reasoning</Badge>
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
						label="Max Tool Iterations"
						value={String(maxToolIterations)}
						oninput={handleMaxToolIterationsInput}
						help={errors.maxToolIterations || 'Maximum tool execution loops (1-200)'}
					/>
				</div>

				<!-- Tools -->
				<div class="form-section">
					<h4 class="section-title">Tools</h4>
					<p class="section-help">Select tools this agent can use</p>

					<div class="checkbox-group">
						{#each availableTools as tool}
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
					<h4 class="section-title">MCP Servers</h4>
					<p class="section-help">Select MCP servers this agent can access</p>

					{#if availableMcpServers.length === 0}
						<p class="no-servers">
							No MCP servers configured. Add servers in the MCP Settings section.
						</p>
					{:else}
						<div class="checkbox-group">
							{#each availableMcpServers as server}
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
					<h4 class="section-title">System Prompt</h4>

					<Textarea
						label="Instructions for the agent"
						value={systemPrompt}
						oninput={handleSystemPromptInput}
						rows={8}
						placeholder="You are a helpful AI assistant specialized in..."
						required
						help={errors.systemPrompt || `${systemPrompt.length}/10000 characters`}
					/>
				</div>
			</div>

			<div class="form-actions">
				<Button variant="ghost" type="button" onclick={oncancel} disabled={saving}>
					Cancel
				</Button>
				<Button variant="primary" type="submit" disabled={saving}>
					{saving ? 'Saving...' : mode === 'create' ? 'Create Agent' : 'Save Changes'}
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
		transition: all var(--transition-fast);
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
		transition: all var(--transition-fast);
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
