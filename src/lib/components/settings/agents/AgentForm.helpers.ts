import type {
	AgentConfig,
	AgentConfigCreate,
	AgentConfigUpdate,
	AgentKind,
	Lifecycle,
	ReasoningEffort
} from '$types/agent';
import type { ProviderInfo } from '$types/custom-provider';
import type { LLMModel, ProviderType } from '$types/llm';
import type { MCPServerConfig } from '$types/mcp';
import type { SkillSummary } from '$types/skill';

export interface AgentFormOption {
	value: string;
	label: string;
	description?: string;
}

export interface AgentProviderOption {
	value: string;
	label: string;
	type: string;
}

export interface AgentFormValidationInput {
	name: string;
	agentId?: string;
	existingAgents: Pick<AgentConfig, 'id' | 'name'>[];
	availableModelsCount: number;
	model: string;
	selectedModel: LLMModel | null | undefined;
	maxToolIterations: number;
	systemPrompt: string;
	translate: (key: string) => string;
}

export function toProviderType(providerName: string): ProviderType {
	return providerName.toLowerCase() as ProviderType;
}

export function formatContextWindow(tokens: number): string {
	if (tokens >= 1_000_000) {
		return `${(tokens / 1_000_000).toFixed(1)}M`;
	}
	if (tokens >= 1_000) {
		return `${Math.round(tokens / 1_000)}K`;
	}
	return tokens.toLocaleString();
}

export function toggleSelection(values: string[], value: string): string[] {
	if (values.includes(value)) {
		return values.filter((existing) => existing !== value);
	}
	return [...values, value];
}

export function buildProviderOptions(
	providerList: ProviderInfo[],
	translate: (key: string) => string
): AgentProviderOption[] {
	if (providerList.length > 0) {
		return providerList.map((provider) => ({
			value: provider.id,
			label: provider.displayName,
			type: provider.isCloud
				? translate('llm_provider_cloud_api')
				: translate('agents_provider_ollama_type')
		}));
	}

	return [
		{
			value: 'mistral',
			label: translate('agents_provider_mistral'),
			type: translate('agents_provider_mistral_type')
		},
		{
			value: 'ollama',
			label: translate('agents_provider_ollama'),
			type: translate('agents_provider_ollama_type')
		}
	];
}

/**
 * Builds the picker options for the agent's skill list.
 *
 * Strict kind filter: a standard agent (`agentKind = undefined`) only sees
 * skills with `kind` undefined; a Kanban agent (`agentKind = 'kanban'`) only
 * sees skills with `kind === 'kanban'`. This mirrors the backend invariant
 * `skill.kind == agent.kind` enforced by `SkillManagerTool::create_skill`.
 */
export function buildAvailableSkills(
	skillSummaries: SkillSummary[],
	agentKind?: 'kanban'
): AgentFormOption[] {
	return skillSummaries
		.filter((skill) => skill.enabled)
		.filter((skill) => (skill.kind ?? null) === (agentKind ?? null))
		.map((skill) => ({
			value: skill.name,
			label: skill.name,
			description: skill.description
		}));
}

export function buildAvailableMcpServers(
	servers: MCPServerConfig[],
	noDescriptionLabel: string
): AgentFormOption[] {
	return servers.map((server) => ({
		value: server.name,
		label: server.name,
		description: server.description || noDescriptionLabel
	}));
}

export function validateAgentForm(input: AgentFormValidationInput): Record<string, string> {
	const errors: Record<string, string> = {};
	const t = input.translate;

	if (!input.name.trim() || input.name.length < 1 || input.name.length > 64) {
		errors.name = t('agents_name_error');
	} else {
		const trimmedLower = input.name.trim().toLowerCase();
		const isDuplicate = input.existingAgents.some(
			(agent) => agent.name.toLowerCase() === trimmedLower && agent.id !== input.agentId
		);
		if (isDuplicate) {
			errors.name = t('agents_name_duplicate');
		}
	}

	if (input.availableModelsCount === 0) {
		errors.model = t('agents_no_models_error');
	} else if (!input.model) {
		errors.model = t('agents_model_required');
	} else if (!input.selectedModel) {
		errors.model = t('agents_model_not_found');
	}

	if (input.maxToolIterations < 1 || input.maxToolIterations > 200) {
		errors.maxToolIterations = t('agents_max_iterations_error');
	}

	if (!input.systemPrompt.trim()) {
		errors.systemPrompt = t('agents_system_prompt_required');
	} else if (input.systemPrompt.length > 10000) {
		errors.systemPrompt = t('agents_system_prompt_max');
	}

	return errors;
}

/**
 * Inputs accepted by {@link buildAgentSubmitPayload}. The form gathers these
 * from its own reactive state; the helper just shapes them into the right
 * IPC contract per mode.
 */
export interface AgentSubmitInput {
	/** Agent name (will be trimmed). */
	name: string;
	/** Lifecycle (only used in create mode; update cannot change it). */
	lifecycle: Lifecycle;
	/** Provider id (e.g. `mistral`, `ollama`, a custom provider id). */
	provider: string;
	/** Model api_name (matches the row picked from the model list). */
	model: string;
	/** Resolved model row picked in the UI (used for temperature/tokens/etc.). */
	selectedModel: Pick<
		LLMModel,
		'temperature_default' | 'max_output_tokens' | 'is_reasoning' | 'context_window'
	>;
	/** Selected tool keys. */
	tools: string[];
	/** Selected MCP server names. */
	mcpServers: string[];
	/** Selected skill names. */
	skills: string[];
	/** Authorized FileManagerTool folders. */
	folders: string[];
	/** System prompt (will be trimmed). */
	systemPrompt: string;
	/** Maximum tool execution iterations. */
	maxToolIterations: number;
	/** Selected reasoning effort, or undefined when "Off". */
	reasoningEffort: ReasoningEffort | undefined;
	/** Optional specialization (e.g. `kanban`). undefined = plain agent. */
	kind: AgentKind | undefined;
	/** When true, finished workflows are analyzed automatically by the agent. */
	autoAnalyzeReports: boolean;
}

/**
 * Builds the IPC payload for `create_agent` from the form state. The
 * `reasoning_effort` key is included as-is (undefined will be omitted by
 * JSON.stringify, which the backend reads as outer `None` — fine for a
 * brand-new row).
 */
export function buildAgentCreatePayload(input: AgentSubmitInput): AgentConfigCreate {
	return {
		name: input.name.trim(),
		lifecycle: input.lifecycle,
		llm: {
			provider: input.provider,
			model: input.model,
			temperature: input.selectedModel.temperature_default,
			max_tokens: input.selectedModel.max_output_tokens,
			is_reasoning: input.selectedModel.is_reasoning,
			context_window: input.selectedModel.context_window
		},
		tools: input.tools,
		mcp_servers: input.mcpServers,
		skills: input.skills,
		folders: input.folders,
		// require_file_confirmation is intentionally omitted: per-agent
		// authorizations live on Settings > Validation, and a new agent takes the
		// fail-safe backend default (true). See AgentAuthorizations.svelte.
		system_prompt: input.systemPrompt.trim(),
		max_tool_iterations: input.maxToolIterations,
		reasoning_effort: input.reasoningEffort,
		kind: input.kind,
		auto_analyze_reports: input.autoAnalyzeReports
	};
}

/**
 * Builds the IPC payload for `update_agent` from the form state. Unlike the
 * create payload, `reasoning_effort` is normalised to `null` when the form
 * value is undefined, so the backend deserialises into `Some(None)` and
 * clears the existing value (vs. "field absent" which the backend would
 * read as "keep existing"). This is the tri-state contract that makes the
 * "Off" dropdown actually clear the value on an existing agent.
 */
export function buildAgentUpdatePayload(input: AgentSubmitInput): AgentConfigUpdate {
	return {
		name: input.name.trim(),
		llm: {
			provider: input.provider,
			model: input.model,
			temperature: input.selectedModel.temperature_default,
			max_tokens: input.selectedModel.max_output_tokens,
			is_reasoning: input.selectedModel.is_reasoning,
			context_window: input.selectedModel.context_window
		},
		tools: input.tools,
		mcp_servers: input.mcpServers,
		skills: input.skills,
		folders: input.folders,
		// require_file_confirmation and mcp_tool_allowlist are intentionally
		// omitted (tri-state "keep existing"): per-agent authorizations are edited
		// on Settings > Validation, and AgentForm must never overwrite them.
		system_prompt: input.systemPrompt.trim(),
		max_tool_iterations: input.maxToolIterations,
		reasoning_effort: input.reasoningEffort ?? null,
		kind: input.kind ?? null,
		auto_analyze_reports: input.autoAnalyzeReports
	};
}
