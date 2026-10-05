/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/**
 * @fileoverview Typed Tauri API client for the agents domain.
 *
 * Covers agent CRUD, LLM models, provider settings, custom providers,
 * prompts, skills, speech-to-text transcription and agent tooling helpers.
 * Each function maps 1:1 to a Rust command; argument keys are camelCase
 * (the `tauri::command` macro converts them to the snake_case Rust params).
 *
 * @module lib/api/agents
 */

import { invokeCommand } from './internal';
import type {
	AgentConfig,
	AgentConfigCreate,
	AgentConfigUpdate,
	AgentSummary
} from '$types/agent';
import type {
	ConnectionTestResult,
	CreateModelRequest,
	LLMModel,
	ProviderSettings,
	ProviderType,
	UpdateModelRequest
} from '$types/llm';
import type {
	CreateCustomProviderRequest,
	CustomProviderResponse,
	ProviderInfo
} from '$types/custom-provider';
import type { Prompt, PromptCreate, PromptSummary, PromptUpdate } from '$types/prompt';
import type { PromptVersion, PromptVersionSummary } from '$types/prompt_version';
import type { Skill, SkillCreate, SkillSummary, SkillUpdate } from '$types/skill';
import type { SkillVersion, SkillVersionSummary } from '$types/skill_version';
import type { AvailableToolInfo } from '$types/validation';
import type { TranscriptionResult } from '$types/stt';

// ============================================================================
// Agent CRUD
// ============================================================================

/** List all registered agents (summaries). */
export async function listAgents(): Promise<AgentSummary[]> {
	return invokeCommand<AgentSummary[]>('list_agents');
}

/** Get the full configuration of an agent. */
export async function getAgentConfig(agentId: string): Promise<AgentConfig> {
	return invokeCommand<AgentConfig>('get_agent_config', { agentId });
}

/** Create an agent, returning its ID. */
export async function createAgent(config: AgentConfigCreate): Promise<string> {
	return invokeCommand<string>('create_agent', { config });
}

/** Update an agent, returning the updated configuration. */
export async function updateAgent(
	agentId: string,
	config: AgentConfigUpdate
): Promise<AgentConfig> {
	return invokeCommand<AgentConfig>('update_agent', { agentId, config });
}

/** Delete an agent. */
export async function deleteAgent(agentId: string): Promise<void> {
	return invokeCommand<void>('delete_agent', { agentId });
}

// ============================================================================
// LLM models
// ============================================================================

/** List LLM models (builtin + custom), optionally filtered by provider. */
export async function listModels(provider?: ProviderType | string | null): Promise<LLMModel[]> {
	return invokeCommand<LLMModel[]>('list_models', { provider: provider });
}

/** Get a single model by ID. */
export async function getModel(id: string): Promise<LLMModel> {
	return invokeCommand<LLMModel>('get_model', { id });
}

/** Get a model by API name and provider. */
export async function getModelByApiName(
	apiName: string,
	provider: ProviderType | string
): Promise<LLMModel> {
	return invokeCommand<LLMModel>('get_model_by_api_name', { apiName, provider });
}

/** Create a custom model. */
export async function createModel(data: CreateModelRequest): Promise<LLMModel> {
	return invokeCommand<LLMModel>('create_model', { data });
}

/** Update an existing model. */
export async function updateModel(id: string, data: UpdateModelRequest): Promise<LLMModel> {
	return invokeCommand<LLMModel>('update_model', { id, data });
}

/** Delete a custom model. */
export async function deleteModel(id: string): Promise<boolean> {
	return invokeCommand<boolean>('delete_model', { id });
}

// ============================================================================
// Provider settings + connection
// ============================================================================

/** Get provider settings. */
export async function getProviderSettings(
	provider: ProviderType | string
): Promise<ProviderSettings> {
	return invokeCommand<ProviderSettings>('get_provider_settings', { provider });
}

/** Update provider settings. */
export async function updateProviderSettings(
	provider: ProviderType | string,
	enabled?: boolean | null,
	baseUrl?: string | null
): Promise<ProviderSettings> {
	return invokeCommand<ProviderSettings>('update_provider_settings', {
		provider,
		enabled: enabled,
		baseUrl: baseUrl
	});
}

/** Test a provider connection. */
export async function testProviderConnection(
	provider: ProviderType | string
): Promise<ConnectionTestResult> {
	return invokeCommand<ConnectionTestResult>('test_provider_connection', { provider });
}

/** Seed the database with builtin models. Returns the number seeded. */
export async function seedBuiltinModels(): Promise<number> {
	return invokeCommand<number>('seed_builtin_models');
}

// ============================================================================
// Custom providers
// ============================================================================

/** List all providers (builtin + custom). */
export async function listProviders(): Promise<ProviderInfo[]> {
	return invokeCommand<ProviderInfo[]>('list_providers');
}

/** Create a custom provider. */
export async function createCustomProvider(
	request: CreateCustomProviderRequest
): Promise<CustomProviderResponse> {
	return invokeCommand<CustomProviderResponse>('create_custom_provider', {
		name: request.name,
		displayName: request.displayName,
		baseUrl: request.baseUrl,
		apiKey: request.apiKey,
		supportsCacheControl: request.supportsCacheControl,
		supportsReasoningParam: request.supportsReasoningParam
	});
}

/** Update inputs for `updateCustomProvider` (all fields optional). */
export interface UpdateCustomProviderInput {
	displayName?: string | null;
	baseUrl?: string | null;
	apiKey?: string | null;
	enabled?: boolean | null;
	supportsCacheControl?: boolean | null;
	supportsReasoningParam?: boolean | null;
}

/** Update an existing custom provider. */
export async function updateCustomProvider(
	name: string,
	input: UpdateCustomProviderInput
): Promise<CustomProviderResponse> {
	return invokeCommand<CustomProviderResponse>('update_custom_provider', {
		name,
		displayName: input.displayName,
		baseUrl: input.baseUrl,
		apiKey: input.apiKey,
		enabled: input.enabled,
		supportsCacheControl: input.supportsCacheControl,
		supportsReasoningParam: input.supportsReasoningParam
	});
}

/** Delete a custom provider. */
export async function deleteCustomProvider(name: string): Promise<void> {
	return invokeCommand<void>('delete_custom_provider', { name });
}

// ============================================================================
// Prompts
// ============================================================================

/** List all prompt templates (summaries). */
export async function listPrompts(): Promise<PromptSummary[]> {
	return invokeCommand<PromptSummary[]>('list_prompts');
}

/** Get a single prompt template. */
export async function getPrompt(promptId: string): Promise<Prompt> {
	return invokeCommand<Prompt>('get_prompt', { promptId });
}

/** Create a prompt template, returning its ID. */
export async function createPrompt(config: PromptCreate): Promise<string> {
	return invokeCommand<string>('create_prompt', { config });
}

/** Update a prompt template. */
export async function updatePrompt(
	promptId: string,
	config: PromptUpdate,
	editedBy?: string | null,
	editSummary?: string | null
): Promise<Prompt> {
	return invokeCommand<Prompt>('update_prompt', {
		promptId,
		config,
		editedBy: editedBy,
		editSummary: editSummary
	});
}

/** Delete a prompt template. */
export async function deletePrompt(promptId: string): Promise<void> {
	return invokeCommand<void>('delete_prompt', { promptId });
}

/** Search prompts by query and/or category. */
export async function searchPrompts(
	query?: string | null,
	category?: string | null
): Promise<PromptSummary[]> {
	return invokeCommand<PromptSummary[]>('search_prompts', {
		query: query,
		category: category
	});
}

/** List version history of a prompt. */
export async function listPromptVersions(promptId: string): Promise<PromptVersionSummary[]> {
	return invokeCommand<PromptVersionSummary[]>('list_prompt_versions', { promptId });
}

/** Get a single prompt version. */
export async function getPromptVersion(versionId: string): Promise<PromptVersion> {
	return invokeCommand<PromptVersion>('get_prompt_version', { versionId });
}

/** Restore a prompt version. */
export async function restorePromptVersion(
	promptId: string,
	versionId: string,
	editedBy?: string | null
): Promise<void> {
	return invokeCommand<void>('restore_prompt_version', {
		promptId,
		versionId,
		editedBy: editedBy
	});
}

/** Delete a prompt version. */
export async function deletePromptVersion(versionId: string): Promise<void> {
	return invokeCommand<void>('delete_prompt_version', { versionId });
}

// ============================================================================
// Skills
// ============================================================================

/** List skills (summaries), optionally filtered by kind. */
export async function listSkills(kind?: string | null): Promise<SkillSummary[]> {
	return invokeCommand<SkillSummary[]>('list_skills', { kind: kind });
}

/** Get a single skill. */
export async function getSkill(skillId: string): Promise<Skill> {
	return invokeCommand<Skill>('get_skill', { skillId });
}

/** Create a skill, returning its ID. */
export async function createSkill(config: SkillCreate): Promise<string> {
	return invokeCommand<string>('create_skill', { config });
}

/** Update a skill. */
export async function updateSkill(
	skillId: string,
	config: SkillUpdate,
	editedBy?: string | null,
	editSummary?: string | null
): Promise<Skill> {
	return invokeCommand<Skill>('update_skill', {
		skillId,
		config,
		editedBy: editedBy,
		editSummary: editSummary
	});
}

/** Delete a skill. */
export async function deleteSkill(skillId: string): Promise<void> {
	return invokeCommand<void>('delete_skill', { skillId });
}

/** List version history of a skill. */
export async function listSkillVersions(skillId: string): Promise<SkillVersionSummary[]> {
	return invokeCommand<SkillVersionSummary[]>('list_skill_versions', { skillId });
}

/** Get a single skill version. */
export async function getSkillVersion(versionId: string): Promise<SkillVersion> {
	return invokeCommand<SkillVersion>('get_skill_version', { versionId });
}

/** Restore a skill version. */
export async function restoreSkillVersion(
	skillId: string,
	versionId: string,
	editedBy?: string | null
): Promise<void> {
	return invokeCommand<void>('restore_skill_version', {
		skillId,
		versionId,
		editedBy: editedBy
	});
}

/** Delete a skill version. */
export async function deleteSkillVersion(versionId: string): Promise<void> {
	return invokeCommand<void>('delete_skill_version', { versionId });
}

// ============================================================================
// Tooling helpers
// ============================================================================

/** List tools available for validation-settings gating. */
export async function listAvailableTools(): Promise<AvailableToolInfo[]> {
	return invokeCommand<AvailableToolInfo[]>('list_available_tools');
}

/** Validate an agent folder path, returning its canonical path. */
export async function validateAgentFolder(path: string): Promise<string> {
	return invokeCommand<string>('validate_agent_folder', { path });
}

/** Preview the Kanban role prompt for an agent and mode. */
export async function previewKanbanRolePrompt(agentId: string, mode: string): Promise<string> {
	return invokeCommand<string>('preview_kanban_role_prompt', { agentId, mode });
}

// ============================================================================
// Speech-to-text
// ============================================================================

/** Input for `transcribeAudio`. */
export interface TranscribeAudioInput {
	audioBase64: string;
	mimeType: string;
	contextBias: string[];
	languageOverride?: string | null;
	modelId: string;
}

/** Transcribe audio to text. */
export async function transcribeAudio(input: TranscribeAudioInput): Promise<TranscriptionResult> {
	return invokeCommand<TranscriptionResult>('transcribe_audio', {
		audioBase64: input.audioBase64,
		mimeType: input.mimeType,
		contextBias: input.contextBias,
		languageOverride: input.languageOverride,
		modelId: input.modelId
	});
}
