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
	import { agentStore, agents } from '$lib/stores/agents';
	import { loadServers, type MCPState, createInitialMCPState, setServers } from '$lib/stores/mcp';
	import {
		loadAllLLMData,
		getModelsByProvider,
		getModelByApiName,
		createInitialLLMState,
		setModels,
		setProviderSettings
	} from '$lib/stores/llm';
	import type { LLMState } from '$types/llm';
	import type { ProviderInfo } from '$types/custom-provider';
	import type { AgentConfig, AgentKind, Lifecycle, ReasoningEffort } from '$types/agent';
	import type { SkillSummary } from '$types/skill';
	import { Button, Input, Textarea, Card, Select, Switch } from '$lib/components/ui';
	import { TriangleAlert } from '@lucide/svelte';
	import { tauriInvoke } from '$lib/tauri';
	import { onMount } from 'svelte';
	import { i18n, t } from '$lib/i18n';
	import {
		getReasoningHelp,
		getReasoningOptions,
		normalizeReasoningEffortForProvider
	} from '$lib/utils/agent-reasoning';
	import {
		buildAgentCreatePayload,
		buildAgentUpdatePayload,
		buildAvailableMcpServers,
		buildAvailableSkills,
		buildProviderOptions,
		toProviderType,
		toggleSelection,
		validateAgentForm
	} from './AgentForm.helpers';
	import {
		attachSettingsRefreshListener,
		dispatchSettingsRefresh
	} from '$lib/utils/settings-refresh';
	import { isDirty } from '$lib/utils/dirty';
	import AgentFolders from './AgentFolders.svelte';
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

	/**
	 * Canonical form values for a given agent (create defaults when null).
	 * Single source for both seeding the editable state and computing the
	 * unsaved-changes indicator, so the two can never drift apart.
	 */
	function formValuesOf(source: AgentConfig | null) {
		return {
			name: source?.name ?? '',
			lifecycle: source?.lifecycle ?? ('permanent' as Lifecycle),
			provider: (source?.llm.provider ?? 'mistral').toLowerCase(),
			model: source?.llm.model ?? 'mistral-large-latest',
			maxToolIterations: source?.max_tool_iterations ?? 50,
			reasoningEffort: source?.reasoning_effort,
			selectedTools: source?.tools ?? [],
			selectedMcpServers: source?.mcp_servers ?? [],
			selectedSkills: source?.skills ?? [],
			selectedFolders: source?.folders ?? [],
			systemPrompt: source?.system_prompt ?? '',
			kind: source?.kind ?? undefined,
			autoAnalyzeReports: source?.auto_analyze_reports ?? false
		};
	}

	/** Form state */
	let name = $state('');
	let lifecycle = $state<Lifecycle>('permanent');
	let provider = $state('mistral');
	let model = $state('mistral-large-latest');
	let maxToolIterations = $state(50);
	let reasoningEffort = $state<ReasoningEffort | undefined>(undefined);
	let selectedTools = $state<string[]>([]);
	let selectedMcpServers = $state<string[]>([]);
	let selectedSkills = $state<string[]>([]);
	let selectedFolders = $state<string[]>([]);
	let systemPrompt = $state('');
	let kind = $state<AgentKind | undefined>(undefined);
	let autoAnalyzeReports = $state(false);

	/** True when the form values diverge from the agent they were seeded with. */
	const dirty = $derived(
		isDirty(formValuesOf(agent), {
			name,
			lifecycle,
			provider,
			model,
			maxToolIterations,
			reasoningEffort,
			selectedTools,
			selectedMcpServers,
			selectedSkills,
			selectedFolders,
			systemPrompt,
			kind,
			autoAnalyzeReports
		})
	);

	// Sync form state when agent prop changes (e.g., switching between edit targets)
	$effect(() => {
		const seed = formValuesOf(agent);
		name = seed.name;
		lifecycle = seed.lifecycle;
		provider = seed.provider;
		model = seed.model;
		maxToolIterations = seed.maxToolIterations;
		reasoningEffort = seed.reasoningEffort;
		selectedTools = seed.selectedTools;
		selectedMcpServers = seed.selectedMcpServers;
		selectedSkills = seed.selectedSkills;
		selectedFolders = seed.selectedFolders;
		systemPrompt = seed.systemPrompt;
		kind = seed.kind;
		autoAnalyzeReports = seed.autoAnalyzeReports;
		// Reset validation state when agent changes
		errors = {};
		previousKind = seed.kind;
	});

	/**
	 * Previous value of `kind` so we can detect a user-driven transition to
	 * `'kanban'` (vs. the effect above resyncing from a loaded agent) and
	 * seed an editable default system prompt template when the field is
	 * blank. Never overwrites a non-empty prompt.
	 */
	let previousKind = $state<AgentKind | undefined>(undefined);

	const KANBAN_DEFAULT_SYSTEM_PROMPT = `You are the Kanban orchestrator. Your role has two modes:

# Compose mode
When asked to compose a kanban card, read the user demand carefully. Discover \
available worker agents (ListAgents) and reusable prompts (PromptManager) if \
available. Pick the most fitting target_agent_id and either reference a \
prompt_id or compose an inline_prompt. Call SubmitComposedCard exactly once \
with the final payload, then end with a 2-3 sentence rationale.

# Analyze mode
When asked to analyze a worker's report, compare it against the user's \
original demand. Pick a verdict: approve (report fulfils the demand), reject \
(report is wrong and unsalvageable) or needs_improvement (provide a full \
replacement prompt). Call SubmitAnalysis exactly once with your verdict and \
reasoning.

Be concise, factual, and conservative in your judgements.`;

	$effect(() => {
		// Inject the default template the first time the user flips `kind`
		// to 'kanban' on a brand-new agent (empty prompt). Editable afterwards.
		if (kind === 'kanban' && previousKind !== 'kanban' && systemPrompt.trim() === '') {
			systemPrompt = KANBAN_DEFAULT_SYSTEM_PROMPT;
		}
		previousKind = kind;
	});

	/** UI state */
	let saving = $state(false);
	let errors = $state<Record<string, string>>({});
	let loadWarnings = $state<string[]>([]);
	let mcpState = $state<MCPState>(createInitialMCPState());
	let llmState = $state<LLMState>(createInitialLLMState());
	let providerList = $state<ProviderInfo[]>([]);
	let availableSkillSummaries = $state<SkillSummary[]>([]);

	/** Available tools (from backend) - reactive to locale */
	const availableTools = $derived([
		{
			value: 'MemoryTool',
			label: $i18n('agents_tool_memory'),
			description: $i18n('agents_tool_memory_desc')
		},
		{
			value: 'TodoTool',
			label: $i18n('agents_tool_todo'),
			description: $i18n('agents_tool_todo_desc')
		},
		{
			value: 'UserQuestionTool',
			label: $i18n('agents_tool_user_question'),
			description: $i18n('agents_tool_user_question_desc')
		},
		{
			value: 'CalculatorTool',
			label: $i18n('agents_tool_calculator'),
			description: $i18n('agents_tool_calculator_desc')
		},
		{
			value: 'FileManagerTool',
			label: $i18n('agents_tool_file_manager'),
			description: $i18n('agents_tool_file_manager_desc')
		},
		{
			value: 'PromptManagerTool',
			label: $i18n('agents_tool_prompt_manager'),
			description: $i18n('agents_tool_prompt_manager_desc')
		},
		{
			value: 'SkillManagerTool',
			label: $i18n('agents_tool_skill_manager'),
			description: $i18n('agents_tool_skill_manager_desc')
		},
		{
			value: 'WorkflowManagerTool',
			label: $i18n('agents_tool_workflow_manager'),
			description: $i18n('agents_tool_workflow_manager_desc')
		}
	]);

	const KANBAN_ONLY_TOOLS = new Set([
		'PromptManagerTool',
		'SkillManagerTool',
		'WorkflowManagerTool'
	]);

	const visibleTools = $derived(
		kind === 'kanban'
			? availableTools
			: availableTools.filter((t) => !KANBAN_ONLY_TOOLS.has(t.value))
	);

	$effect(() => {
		if (kind !== 'kanban') {
			if (selectedTools.some((t) => KANBAN_ONLY_TOOLS.has(t))) {
				selectedTools = selectedTools.filter((t) => !KANBAN_ONLY_TOOLS.has(t));
			}
			if (autoAnalyzeReports) {
				autoAnalyzeReports = false;
			}
		}
		// Purge skills that no longer match the current agent kind (strict invariant).
		const validSkillNames = new Set(
			availableSkillSummaries.filter((s) => (s.kind ?? null) === (kind ?? null)).map((s) => s.name)
		);
		if (
			availableSkillSummaries.length > 0 &&
			selectedSkills.some((name) => !validSkillNames.has(name))
		) {
			selectedSkills = selectedSkills.filter((name) => validSkillNames.has(name));
		}
	});

	/** Lifecycle options with descriptions - reactive to locale */
	const lifecycleOptions = $derived([
		{
			value: 'permanent' as Lifecycle,
			label: $i18n('agents_lifecycle_permanent'),
			description: $i18n('agents_lifecycle_permanent_desc')
		},
		{
			value: 'temporary' as Lifecycle,
			label: $i18n('agents_lifecycle_temporary'),
			description: $i18n('agents_lifecycle_temporary_desc')
		}
	]);

	/**
	 * Help line under the lifecycle select: immutability notice in edit mode,
	 * description of the currently selected option in create mode.
	 */
	const lifecycleHelp = $derived(
		mode === 'edit'
			? $i18n('agents_lifecycle_readonly')
			: (lifecycleOptions.find((option) => option.value === lifecycle)?.description ?? '')
	);

	/** Agent kind select options - reactive to locale */
	const kindOptions = $derived([
		{ value: '', label: $i18n('agents_kind_none') },
		{ value: 'kanban', label: $i18n('agents_kind_kanban') }
	]);

	/** Provider options with details - reactive to locale, includes custom providers */
	const providerOptions = $derived.by(() => buildProviderOptions(providerList, $i18n));

	/** Provider select options: "Name (type)" per the settings mockups */
	const providerSelectOptions = $derived(
		providerOptions.map((option) => ({
			value: option.value,
			label: `${option.label} (${option.type})`
		}))
	);

	/** Reactive model list based on selected provider (full model objects) */
	const availableModels = $derived.by(() => {
		const providerType = toProviderType(provider);
		return getModelsByProvider(llmState, providerType);
	});

	/** Model select options: monospace api name, builtin marker appended */
	const modelSelectOptions = $derived(
		availableModels.map((m) => ({
			value: m.api_name,
			label: m.is_builtin ? `${m.api_name} — ${$i18n('agents_model_builtin')}` : m.api_name
		}))
	);

	/** Selected model object (for auto-populating temperature/maxTokens) */
	const selectedModel = $derived.by(() => {
		const providerType = toProviderType(provider);
		return getModelByApiName(llmState, model, providerType);
	});

	/**
	 * Reasoning-effort options for the Select. Mistral only exposes Off / High;
	 * other providers keep the full Off / Low / Medium / High range.
	 */
	const reasoningOptions = $derived(getReasoningOptions(provider, $i18n, selectedModel?.api_name));

	/** Help text below the reasoning-effort Select (provider-specific). */
	const reasoningHelp = $derived(getReasoningHelp(provider, $i18n));

	/**
	 * Keep `reasoningEffort` consistent with the available options when the
	 * provider switches to Mistral while a non-exposed value (low/medium) is
	 * selected, or when the user switches away from a DeepSeek model while
	 * `xhigh` is selected. Both cases collapse to "high" so the form state
	 * matches what the backend persists.
	 */
	$effect(() => {
		const normalized = normalizeReasoningEffortForProvider(
			provider,
			reasoningEffort,
			selectedModel?.api_name
		);
		if (normalized !== reasoningEffort) {
			reasoningEffort = normalized;
		}
	});

	/** Available skills (enabled only) */
	const availableSkills = $derived(buildAvailableSkills(availableSkillSummaries, kind));

	/** Available MCP servers from store */
	const availableMcpServers = $derived(
		buildAvailableMcpServers(mcpState.servers, $i18n('agents_mcp_no_description'))
	);

	/**
	 * Loads MCP servers, skills, and LLM models. Used both on mount and when a
	 * `settings:refresh` event fires (e.g. the user toggled `is_reasoning` on a
	 * model in Settings -> Models, which must be reflected here without forcing
	 * a remount of the form).
	 *
	 * Each refresh starts from a clean local `warnings` array (assigned once at
	 * the end) rather than appending to `loadWarnings`, so warnings resolved in
	 * a previous run do not stay pinned after a successful reload.
	 */
	async function loadAgentFormResources(): Promise<void> {
		const warnings: string[] = [];

		try {
			const servers = await loadServers();
			mcpState = setServers(mcpState, servers);
		} catch {
			warnings.push(t('agents_mcp_load_failed'));
		}

		try {
			availableSkillSummaries = await tauriInvoke<SkillSummary[]>('list_skills');
		} catch {
			warnings.push(t('agents_skills_load_failed'));
		}

		try {
			const data = await loadAllLLMData();
			providerList = data.providerList;
			let nextLlmState = createInitialLLMState();
			for (const [providerId, provSettings] of Object.entries(data.settings)) {
				nextLlmState = setProviderSettings(nextLlmState, providerId, provSettings);
			}
			nextLlmState = setModels(nextLlmState, data.models);
			llmState = nextLlmState;
		} catch {
			warnings.push(t('agents_llm_load_failed'));
		}

		loadWarnings = warnings;
	}

	onMount(() => {
		void loadAgentFormResources();
		// React to CRUD events from sibling Settings pages (Models, MCP, etc.)
		// so a freshly-toggled `is_reasoning` or a renamed/added model shows up
		// here immediately, rather than only after the next remount.
		return attachSettingsRefreshListener(loadAgentFormResources, { ignoreSource: 'agents' });
	});

	/**
	 * Updates model when provider changes (reset to first available if current invalid)
	 */
	$effect(() => {
		const first = availableModels[0];
		if (first) {
			const currentModelValid = availableModels.some((m) => m.api_name === model);
			if (!currentModelValid) {
				model = first.api_name;
			}
		}
	});

	/**
	 * Drop a stale reasoning_effort when the selected model is non-reasoning.
	 * Mirrors the backend normalization in `validate_agent_create`: a value
	 * carried over from a previous reasoning model would otherwise be sent on
	 * save and silently dropped at runtime — leaving the form's hidden state
	 * out of sync with what the backend persists.
	 *
	 * The strict `=== false` check is intentional: while the LLM list is still
	 * loading, `selectedModel` is `undefined` and we leave the user's choice
	 * untouched (avoids wiping a valid effort during the initial render).
	 */
	$effect(() => {
		if (selectedModel?.is_reasoning === false && reasoningEffort !== undefined) {
			reasoningEffort = undefined;
		}
	});

	/**
	 * Validates form fields
	 */
	function validate(): boolean {
		errors = validateAgentForm({
			name,
			agentId: agent?.id,
			existingAgents: $agents,
			availableModelsCount: availableModels.length,
			model,
			selectedModel,
			maxToolIterations,
			systemPrompt,
			translate: t
		});

		return Object.keys(errors).length === 0;
	}

	/**
	 * Handles form submission. The payload shape differs by mode to exploit
	 * the tri-state PATCH contract on update (see buildAgentUpdatePayload).
	 */
	async function handleSubmit(): Promise<void> {
		if (!validate()) return;
		if (!selectedModel) return;

		saving = true;

		const submitInput = {
			name,
			lifecycle,
			provider,
			model,
			selectedModel,
			tools: selectedTools,
			mcpServers: selectedMcpServers,
			skills: selectedSkills,
			folders: selectedFolders,
			systemPrompt,
			maxToolIterations,
			reasoningEffort,
			kind,
			autoAnalyzeReports
		};

		try {
			if (mode === 'create') {
				await agentStore.createAgent(buildAgentCreatePayload(submitInput));
			} else if (agent) {
				await agentStore.updateAgent(agent.id, buildAgentUpdatePayload(submitInput));
			}
			// Notify other Settings surfaces (workflow sidebar, sibling forms) so
			// they pick up the new agent set without waiting for the next mount.
			// The `source` tag lets the host `/settings/agents` page ignore its
			// own echo (the CRUD store already refreshed the list).
			dispatchSettingsRefresh({ source: 'agents' });
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
		selectedTools = toggleSelection(selectedTools, toolValue);
	}

	/**
	 * Toggles skill selection
	 */
	function toggleSkill(skillName: string): void {
		selectedSkills = toggleSelection(selectedSkills, skillName);
	}

	/**
	 * Toggles MCP server selection
	 */
	function toggleMcpServer(serverName: string): void {
		selectedMcpServers = toggleSelection(selectedMcpServers, serverName);
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
	{#snippet header()}
		<span class="card-title">
			{mode === 'create'
				? $i18n('agents_create_new')
				: `${$i18n('agents_edit')} — ${agent?.name ?? ''}`}
		</span>
	{/snippet}
	{#snippet body()}
		<form
			class="agent-form"
			onsubmit={(e) => {
				e.preventDefault();
				handleSubmit();
			}}
		>
			{#if loadWarnings.length > 0}
				<div class="load-warnings" role="status">
					{#each loadWarnings as warning (warning)}
						<p class="load-warning">{warning}</p>
					{/each}
				</div>
			{/if}

			<!-- Basic Information -->
			<section class="form-section">
				<h4 class="section-title">{$i18n('agents_basic_info')}</h4>

				<div class="field-grid cols-3">
					<Input
						label={$i18n('agents_name_label')}
						value={name}
						oninput={(e) => {
							name = e.currentTarget.value;
						}}
						placeholder={$i18n('agents_name_placeholder')}
						required
						help={errors.name || $i18n('agents_name_help')}
					/>

					<Select
						id="agent-lifecycle"
						label={$i18n('agents_lifecycle')}
						value={lifecycle}
						options={lifecycleOptions.map((option) => ({
							value: option.value,
							label: option.label
						}))}
						disabled={mode === 'edit'}
						onchange={(e) => {
							lifecycle = e.currentTarget.value as Lifecycle;
						}}
						help={lifecycleHelp}
					/>

					<Select
						id="agent-kind"
						label={$i18n('agents_kind')}
						value={kind ?? ''}
						options={kindOptions}
						onchange={(e) => {
							kind = e.currentTarget.value === 'kanban' ? 'kanban' : undefined;
						}}
						help={$i18n('agents_kind_help')}
					/>
				</div>

				{#if kind === 'kanban'}
					<div class="toggle-row">
						<span class="toggle-text">
							<strong id="agent-auto-analyze-label">{$i18n('agents_auto_analyze')}</strong>
							<span>{$i18n('agents_auto_analyze_desc')}</span>
						</span>
						<Switch
							checked={autoAnalyzeReports}
							onchange={(value) => (autoAnalyzeReports = value)}
							labelledBy="agent-auto-analyze-label"
						/>
					</div>
				{/if}
			</section>

			<!-- LLM Configuration -->
			<section class="form-section">
				<h4 class="section-title">{$i18n('agents_llm_config')}</h4>

				<div class="field-grid cols-2">
					<Select
						id="agent-provider"
						label={$i18n('agents_provider')}
						value={provider}
						options={providerSelectOptions}
						onchange={(e) => {
							provider = e.currentTarget.value;
						}}
					/>

					{#if availableModels.length === 0}
						<div class="no-models-message">
							<p>{$i18n('agents_no_models', { provider })}</p>
							<p>{$i18n('agents_no_models_hint')}</p>
						</div>
					{:else}
						<Select
							id="agent-model"
							label={$i18n('agents_model')}
							value={model}
							options={modelSelectOptions}
							onchange={(e) => {
								model = e.currentTarget.value;
							}}
							help={errors.model}
						/>
					{/if}

					{#if selectedModel?.is_reasoning}
						<Select
							id="reasoning-effort"
							label={$i18n('agents_reasoning_effort')}
							value={reasoningEffort ?? ''}
							options={reasoningOptions}
							onchange={(e) => {
								const v = e.currentTarget.value;
								reasoningEffort = v ? (v as ReasoningEffort) : undefined;
							}}
							help={reasoningHelp}
						/>
					{/if}

					<Input
						type="number"
						min={1}
						max={200}
						label={$i18n('agents_max_iterations_label')}
						value={String(maxToolIterations)}
						oninput={handleMaxToolIterationsInput}
						help={errors.maxToolIterations || $i18n('agents_max_iterations_help')}
					/>
				</div>
			</section>

			<!-- System Prompt -->
			<section class="form-section">
				<h4 class="section-title">{$i18n('agents_system_prompt')}</h4>

				<Textarea
					label={$i18n('agents_system_prompt_label')}
					value={systemPrompt}
					oninput={handleSystemPromptInput}
					rows={5}
					placeholder={$i18n('agents_system_prompt_placeholder')}
					required
					help={errors.systemPrompt ||
						$i18n('agents_system_prompt_chars', { count: systemPrompt.length })}
				/>
			</section>

			<!-- Tools -->
			<section class="form-section">
				<h4 class="section-title">{$i18n('agents_tools_section')}</h4>
				<p class="section-help">{$i18n('agents_tools_help')}</p>

				<div class="toggle-grid">
					{#each visibleTools as tool (tool.value)}
						<div class="toggle-row">
							<span class="toggle-text">
								<strong id="agent-tool-{tool.value}">{tool.label}</strong>
								<span>{tool.description}</span>
							</span>
							<Switch
								checked={selectedTools.includes(tool.value)}
								onchange={() => toggleTool(tool.value)}
								labelledBy="agent-tool-{tool.value}"
							/>
						</div>
					{/each}
				</div>
			</section>

			<!-- Folders -->
			<section class="form-section">
				<h4 class="section-title">{$i18n('agents_section_folders')}</h4>

				<AgentFolders
					folders={selectedFolders}
					onchange={(f) => {
						selectedFolders = f;
					}}
				/>
			</section>

			<!-- MCP Servers -->
			<section class="form-section">
				<h4 class="section-title">{$i18n('agents_mcp_section')}</h4>
				<p class="section-help">{$i18n('agents_mcp_help')}</p>

				{#if availableMcpServers.length === 0}
					<p class="no-servers">
						{$i18n('agents_mcp_none')}
					</p>
				{:else}
					<div class="toggle-list">
						{#each availableMcpServers as server, index (server.value)}
							<div class="toggle-row">
								<span class="toggle-text">
									<strong id="agent-mcp-{index}">{server.label}</strong>
									<span>{server.description}</span>
								</span>
								<Switch
									checked={selectedMcpServers.includes(server.value)}
									onchange={() => toggleMcpServer(server.value)}
									labelledBy="agent-mcp-{index}"
								/>
							</div>
						{/each}
					</div>
				{/if}
			</section>

			<!-- Skills -->
			<section class="form-section">
				<h4 class="section-title">{$i18n('agents_skills_section')}</h4>
				<p class="section-help">{$i18n('agents_skills_help')}</p>

				{#if availableSkills.length === 0}
					<p class="no-servers">
						{$i18n('agents_skills_none')}
					</p>
				{:else}
					<div class="toggle-list">
						{#each availableSkills as skill, index (skill.value)}
							<div class="toggle-row">
								<span class="toggle-text">
									<strong id="agent-skill-{index}">{skill.label}</strong>
									<span>{skill.description}</span>
								</span>
								<Switch
									checked={selectedSkills.includes(skill.value)}
									onchange={() => toggleSkill(skill.value)}
									labelledBy="agent-skill-{index}"
								/>
							</div>
						{/each}
					</div>
				{/if}
			</section>

			<div class="form-actions">
				{#if dirty && !saving}
					<span class="dirty-hint" role="status">
						<TriangleAlert size={14} aria-hidden="true" />
						{$i18n('settings_unsaved_changes')}
					</span>
				{/if}
				<Button variant="ghost" type="button" onclick={oncancel} disabled={saving}>
					{$i18n('common_cancel')}
				</Button>
				<Button variant="primary" type="submit" disabled={saving}>
					{saving
						? $i18n('agents_saving')
						: mode === 'create'
							? $i18n('agents_create')
							: $i18n('agents_save_changes')}
				</Button>
			</div>
		</form>
	{/snippet}
</Card>

<style>
	.agent-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xl);
	}

	.load-warnings {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.load-warning {
		margin: 0;
		padding: var(--spacing-xs) var(--spacing-sm);
		font-size: var(--font-size-sm);
		color: var(--color-warning);
		background: var(--color-warning-bg, rgba(234, 179, 8, 0.1));
		border-radius: var(--border-radius-sm);
	}

	.form-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.section-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-accent-deep);
		margin: 0;
	}

	.section-help {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin: 0;
	}

	.field-grid {
		display: grid;
		gap: var(--spacing-md);
		align-items: start;
	}

	.field-grid.cols-3 {
		grid-template-columns: repeat(3, 1fr);
	}

	.field-grid.cols-2 {
		grid-template-columns: repeat(2, 1fr);
	}

	/* Toggle rows (auto-analyze, tools, MCP servers, skills) */
	.toggle-row {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--spacing-lg);
		padding: var(--spacing-sm) 0;
	}

	.toggle-list .toggle-row + .toggle-row {
		border-top: 1px solid var(--color-border-light);
	}

	.toggle-text strong {
		display: block;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.toggle-text span {
		display: block;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin-top: 2px;
		max-width: 56ch;
	}

	.toggle-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		column-gap: var(--spacing-xl);
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

	/* Form Actions: sticky save bar pinned to the bottom of the scrolling
	   form. Opaque card surface, no backdrop blur: a blurred sticky bar
	   forces WebKitGTK to re-blur the content scrolling behind it on every
	   frame. */
	.form-actions {
		position: sticky;
		bottom: 0;
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		justify-content: flex-end;
		padding: var(--spacing-md) 0;
		border-top: 1px solid var(--color-border);
		background: var(--surface-1);
	}

	.dirty-hint {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-xs);
		margin-right: auto;
		font-size: var(--font-size-xs);
		color: var(--color-warning);
	}

	/* Responsive */
	@media (max-width: 900px) {
		.field-grid.cols-3 {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 768px) {
		.field-grid.cols-2,
		.toggle-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
