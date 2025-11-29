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
		createInitialLLMState,
		setModels,
		setProviderSettings
	} from '$lib/stores/llm';
	import type { ProviderType, LLMState } from '$types/llm';
	import type { AgentConfig, AgentConfigCreate, Lifecycle } from '$types/agent';
	import { Button, Input, Textarea, Select, Card } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
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
	let temperature = $state(agent?.llm.temperature ?? 0.7);
	let maxTokens = $state(agent?.llm.max_tokens ?? 4096);
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

	/** Lifecycle options */
	const lifecycleOptions: SelectOption[] = [
		{ value: 'permanent', label: 'Permanent' },
		{ value: 'temporary', label: 'Temporary' }
	];

	/** Provider options */
	const providerOptions: SelectOption[] = [
		{ value: 'Mistral', label: 'Mistral AI' },
		{ value: 'Ollama', label: 'Ollama (Local)' }
	];

	/**
	 * Converts provider name to ProviderType (lowercase)
	 */
	function toProviderType(providerName: string): ProviderType {
		return providerName.toLowerCase() as ProviderType;
	}

	/** Reactive model options based on selected provider (from LLM store) */
	const modelOptions = $derived.by(() => {
		const providerType = toProviderType(provider);
		const models = getModelsByProvider(llmState, providerType);
		return models.map((m) => ({
			value: m.api_name,
			label: m.name
		}));
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
		if (modelOptions.length > 0) {
			const currentModelValid = modelOptions.some((o) => o.value === model);
			if (!currentModelValid) {
				model = modelOptions[0].value;
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

		if (modelOptions.length === 0) {
			errors.model = 'No models available - add models in Models section first';
		} else if (!model) {
			errors.model = 'Model is required';
		}

		if (temperature < 0 || temperature > 2) {
			errors.temperature = 'Temperature must be between 0 and 2';
		}

		if (maxTokens < 256 || maxTokens > 128000) {
			errors.maxTokens = 'Max tokens must be between 256 and 128000';
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

		saving = true;

		const config: AgentConfigCreate = {
			name: name.trim(),
			lifecycle,
			llm: {
				provider,
				model,
				temperature,
				max_tokens: maxTokens
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
	 * Handles provider change
	 */
	function handleProviderChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		provider = event.currentTarget.value;
	}

	/**
	 * Handles model change
	 */
	function handleModelChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		model = event.currentTarget.value;
	}

	/**
	 * Handles lifecycle change
	 */
	function handleLifecycleChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		lifecycle = event.currentTarget.value as Lifecycle;
	}

	/**
	 * Handles temperature input
	 */
	function handleTemperatureInput(event: Event & { currentTarget: HTMLInputElement }): void {
		temperature = parseFloat(event.currentTarget.value) || 0;
	}

	/**
	 * Handles max tokens input
	 */
	function handleMaxTokensInput(event: Event & { currentTarget: HTMLInputElement }): void {
		maxTokens = parseInt(event.currentTarget.value, 10) || 256;
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

					<Select
						label="Lifecycle"
						value={lifecycle}
						options={lifecycleOptions}
						onchange={handleLifecycleChange}
						disabled={mode === 'edit'}
						help={mode === 'edit' ? 'Lifecycle cannot be changed after creation' : 'Permanent agents persist across sessions'}
					/>
				</div>

				<!-- LLM Configuration -->
				<div class="form-section">
					<h4 class="section-title">LLM Configuration</h4>

					<Select
						label="Provider"
						value={provider}
						options={providerOptions}
						onchange={handleProviderChange}
					/>

					{#if modelOptions.length === 0}
						<div class="no-models-message">
							<p>No models configured for {provider}.</p>
							<p>Add models in the Models section above.</p>
						</div>
					{:else}
						<Select
							label="Model"
							value={model}
							options={modelOptions}
							onchange={handleModelChange}
							help={errors.model || undefined}
						/>
					{/if}

					<div class="number-inputs">
						<Input
							type="number"
							label="Temperature"
							value={String(temperature)}
							oninput={handleTemperatureInput}
							help={errors.temperature || '0 = deterministic, 2 = creative'}
						/>

						<Input
							type="number"
							label="Max Tokens"
							value={String(maxTokens)}
							oninput={handleMaxTokensInput}
							help={errors.maxTokens || 'Maximum response length'}
						/>
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

	.number-inputs {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-md);
	}

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

	.form-actions {
		display: flex;
		gap: var(--spacing-md);
		justify-content: flex-end;
		padding-top: var(--spacing-lg);
		border-top: 1px solid var(--color-border);
	}

	@media (max-width: 768px) {
		.form-grid {
			grid-template-columns: 1fr;
		}

		.number-inputs {
			grid-template-columns: 1fr;
		}
	}
</style>
