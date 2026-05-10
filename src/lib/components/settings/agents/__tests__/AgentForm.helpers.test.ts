import { describe, expect, it } from 'vitest';
import type { AgentConfig } from '$types/agent';
import type { ProviderInfo } from '$types/custom-provider';
import type { LLMModel } from '$types/llm';
import type { MCPServerConfig } from '$types/mcp';
import type { SkillSummary } from '$types/skill';
import {
	buildAvailableMcpServers,
	buildAvailableSkills,
	buildProviderOptions,
	formatContextWindow,
	toProviderType,
	toggleSelection,
	validateAgentForm
} from '../AgentForm.helpers';

const t = (key: string) => key;

const model = {
	id: 'model-1',
	provider: 'mistral',
	name: 'Mistral Large',
	api_name: 'mistral-large-latest',
	context_window: 128000,
	max_output_tokens: 8192,
	temperature_default: 0.7,
	is_builtin: true,
	is_reasoning: false,
	input_price_per_mtok: 0,
	output_price_per_mtok: 0,
	cache_read_price_per_mtok: 0,
	cache_write_price_per_mtok: 0,
	created_at: '2025-01-01T00:00:00Z',
	updated_at: '2025-01-01T00:00:00Z'
} satisfies LLMModel;

function validationInput(overrides = {}) {
	return {
		name: 'AgentOne',
		agentId: undefined,
		existingAgents: [] as Pick<AgentConfig, 'id' | 'name'>[],
		availableModelsCount: 1,
		model: model.api_name,
		selectedModel: model,
		maxToolIterations: 50,
		systemPrompt: 'You are helpful.',
		translate: t,
		...overrides
	};
}

describe('AgentForm helpers', () => {
	it('normalizes provider names to provider types', () => {
		expect(toProviderType('MISTRAL')).toBe('mistral');
		expect(toProviderType('Custom-Provider')).toBe('custom-provider');
	});

	it('formats context windows with current K/M display rules', () => {
		expect(formatContextWindow(999)).toBe('999');
		expect(formatContextWindow(1000)).toBe('1K');
		expect(formatContextWindow(128000)).toBe('128K');
		expect(formatContextWindow(1000000)).toBe('1.0M');
		expect(formatContextWindow(1500000)).toBe('1.5M');
	});

	it('toggles selections immutably while preserving order', () => {
		const original = ['MemoryTool', 'TodoTool'];
		expect(toggleSelection(original, 'CalculatorTool')).toEqual([
			'MemoryTool',
			'TodoTool',
			'CalculatorTool'
		]);
		expect(toggleSelection(original, 'MemoryTool')).toEqual(['TodoTool']);
		expect(original).toEqual(['MemoryTool', 'TodoTool']);
	});

	it('builds provider fallback options and loaded provider options', () => {
		expect(buildProviderOptions([], t)).toEqual([
			{ value: 'mistral', label: 'agents_provider_mistral', type: 'agents_provider_mistral_type' },
			{ value: 'ollama', label: 'agents_provider_ollama', type: 'agents_provider_ollama_type' }
		]);

		const providers = [
			{ id: 'mistral', displayName: 'Mistral', isCloud: true },
			{ id: 'ollama', displayName: 'Ollama', isCloud: false }
		] as ProviderInfo[];
		expect(buildProviderOptions(providers, t)).toEqual([
			{ value: 'mistral', label: 'Mistral', type: 'llm_provider_cloud_api' },
			{ value: 'ollama', label: 'Ollama', type: 'agents_provider_ollama_type' }
		]);
	});

	it('builds available skills from enabled skills only', () => {
		const skills = [
			{ name: 'enabled', description: 'Enabled skill', enabled: true },
			{ name: 'disabled', description: 'Disabled skill', enabled: false }
		] as SkillSummary[];

		expect(buildAvailableSkills(skills)).toEqual([
			{ value: 'enabled', label: 'enabled', description: 'Enabled skill' }
		]);
	});

	it('builds MCP server options with description fallback', () => {
		const servers = [
			{ name: 'filesystem', description: 'Files' },
			{ name: 'github', description: '' }
		] as MCPServerConfig[];

		expect(buildAvailableMcpServers(servers, 'No description')).toEqual([
			{ value: 'filesystem', label: 'filesystem', description: 'Files' },
			{ value: 'github', label: 'github', description: 'No description' }
		]);
	});

	it('validates name constraints and duplicate names case-insensitively', () => {
		expect(validateAgentForm(validationInput({ name: '   ' })).name).toBe('agents_name_error');
		expect(validateAgentForm(validationInput({ name: 'a'.repeat(65) })).name).toBe(
			'agents_name_error'
		);
		expect(
			validateAgentForm(
				validationInput({
					name: 'AgentOne',
					existingAgents: [{ id: 'other', name: 'agentone' }]
				})
			).name
		).toBe('agents_name_duplicate');
		expect(
			validateAgentForm(
				validationInput({
					name: 'AgentOne',
					agentId: 'same',
					existingAgents: [{ id: 'same', name: 'agentone' }]
				})
			).name
		).toBeUndefined();
	});

	it('validates model priority, max iterations and system prompt constraints', () => {
		expect(
			validateAgentForm(
				validationInput({ availableModelsCount: 0, model: '', selectedModel: null })
			).model
		).toBe('agents_no_models_error');
		expect(validateAgentForm(validationInput({ model: '', selectedModel: null })).model).toBe(
			'agents_model_required'
		);
		expect(validateAgentForm(validationInput({ selectedModel: null })).model).toBe(
			'agents_model_not_found'
		);
		expect(validateAgentForm(validationInput({ maxToolIterations: 0 })).maxToolIterations).toBe(
			'agents_max_iterations_error'
		);
		expect(validateAgentForm(validationInput({ maxToolIterations: 201 })).maxToolIterations).toBe(
			'agents_max_iterations_error'
		);
		expect(validateAgentForm(validationInput({ systemPrompt: '   ' })).systemPrompt).toBe(
			'agents_system_prompt_required'
		);
		expect(
			validateAgentForm(validationInput({ systemPrompt: 'a'.repeat(10001) })).systemPrompt
		).toBe('agents_system_prompt_max');
		expect(validateAgentForm(validationInput())).toEqual({});
	});
});
