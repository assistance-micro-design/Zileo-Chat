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
 * Import/Export Settings Types (Schema v1.1)
 *
 * Types for exporting and importing configuration entities:
 * Agents, MCP Servers, Models, Prompts, Skills, Custom Providers.
 * Synchronized with src-tauri/src/models/import_export.rs
 *
 * @module types/import-export
 */

import type { ReasoningEffort } from './agent';

// ============ EXPORT TYPES ============

/**
 * Selection of entities to export.
 * At least one entity must be selected.
 */
export interface ExportSelection {
	agents: string[];
	mcpServers: string[];
	models: string[];
	prompts: string[];
	skills: string[];
	customProviders: string[];
}

/**
 * Export configuration options.
 */
export interface ExportOptions {
	format: 'json';
	includeTimestamps: boolean;
	sanitizeMcp: boolean;
}

/**
 * MCP server sanitization configuration for export.
 */
export interface MCPSanitizationConfig {
	clearEnvKeys: string[];
	modifyEnv: Record<string, string>;
	modifyArgs: string[];
	excludeFromExport: boolean;
}

/**
 * Export manifest with metadata about the export.
 */
export interface ExportManifest {
	version: string;
	appVersion: string;
	exportedAt: string;
	exportedBy?: string;
	description?: string;
	counts: ExportCounts;
}

/**
 * Entity counts in an export package.
 */
export interface ExportCounts {
	agents: number;
	mcpServers: number;
	models: number;
	prompts: number;
	skills: number;
	customProviders: number;
}

/**
 * Complete export package (schema v1.1).
 */
export interface ExportPackage {
	manifest: ExportManifest;
	agents: AgentExportData[];
	mcpServers: MCPServerExportData[];
	models: LLMModelExportData[];
	prompts: PromptExportData[];
	skills: SkillExportData[];
	customProviders: CustomProviderExportData[];
}

// ============ ENTITY EXPORT DATA ============

/**
 * LLM config for export (v1.1: added isReasoning, contextWindow).
 */
export interface LLMConfigExport {
	provider: string;
	model: string;
	temperature: number;
	maxTokens: number;
	/** Whether the model supports reasoning/thinking (v1.1) */
	isReasoning: boolean;
	/** Context window size override (v1.1) */
	contextWindow?: number;
}

/**
 * Agent data for export (v1.1: added folders, requireFileConfirmation).
 */
export interface AgentExportData {
	name: string;
	lifecycle: 'permanent' | 'temporary';
	llm: LLMConfigExport;
	tools: string[];
	mcpServers: string[];
	skills: string[];
	systemPrompt: string;
	maxToolIterations: number;
	reasoningEffort: ReasoningEffort | null;
	/** Authorized folder paths (v1.1, machine-specific) */
	folders: string[];
	/** Whether file operations require user confirmation (v1.1) */
	requireFileConfirmation: boolean;
	createdAt?: string;
	updatedAt?: string;
}

/**
 * MCP Server data for export.
 */
export interface MCPServerExportData {
	name: string;
	enabled: boolean;
	command: string;
	args: string[];
	env: Record<string, string>;
	description?: string;
	createdAt?: string;
	updatedAt?: string;
}

/**
 * LLM Model data for export.
 */
export interface LLMModelExportData {
	provider: string;
	name: string;
	apiName: string;
	contextWindow: number;
	maxOutputTokens: number;
	temperatureDefault: number;
	isBuiltin: boolean;
	isReasoning: boolean;
	inputPricePerMtok: number;
	outputPricePerMtok: number;
	cacheReadPricePerMtok: number;
	cacheWritePricePerMtok: number;
	createdAt?: string;
	updatedAt?: string;
}

/**
 * Prompt data for export.
 */
export interface PromptExportData {
	name: string;
	description: string;
	category: 'system' | 'user' | 'analysis' | 'generation' | 'coding' | 'custom';
	content: string;
	createdAt?: string;
	updatedAt?: string;
}

/**
 * Skill data for export (v1.1).
 */
export interface SkillExportData {
	name: string;
	description: string;
	category: 'system' | 'coding' | 'workflow' | 'analysis' | 'custom';
	content: string;
	enabled: boolean;
	createdAt?: string;
	updatedAt?: string;
}

/**
 * Custom provider data for export (v1.1).
 */
export interface CustomProviderExportData {
	name: string;
	displayName: string;
	baseUrl: string;
	enabled: boolean;
	createdAt?: string;
}

// ============ PREVIEW SUMMARIES ============

/**
 * Preview data returned before finalizing export.
 */
export interface ExportPreviewData {
	agents: AgentExportSummary[];
	mcpServers: MCPServerExportSummary[];
	models: LLMModelExportSummary[];
	prompts: PromptExportSummary[];
	skills: SkillExportSummary[];
	customProviders: CustomProviderExportSummary[];
	mcpEnvKeys: Record<string, string[]>;
}

/** Agent summary for preview. */
export interface AgentExportSummary {
	id?: string;
	name: string;
	lifecycle: string;
	provider: string;
	model: string;
	toolsCount: number;
	mcpServersCount: number;
	skillsCount: number;
	foldersCount: number;
}

/** MCP server summary for preview. */
export interface MCPServerExportSummary {
	id?: string;
	name: string;
	enabled: boolean;
	command: string;
	toolsCount: number;
}

/** LLM model summary for preview. */
export interface LLMModelExportSummary {
	id?: string;
	name: string;
	provider: string;
	apiName: string;
	isBuiltin: boolean;
}

/** Prompt summary for preview. */
export interface PromptExportSummary {
	id?: string;
	name: string;
	description: string;
	category: string;
	variablesCount: number;
}

/** Skill summary for preview (v1.1). */
export interface SkillExportSummary {
	id?: string;
	name: string;
	category: string;
	enabled: boolean;
	contentLength: number;
}

/** Custom provider summary for preview (v1.1). */
export interface CustomProviderExportSummary {
	id?: string;
	name: string;
	displayName: string;
	baseUrl: string;
}

// ============ IMPORT TYPES ============

/**
 * Selection of entities to import from the package.
 * Note: These are entity NAMES, not IDs.
 */
export interface ImportSelection {
	agents: string[];
	mcpServers: string[];
	models: string[];
	prompts: string[];
	skills: string[];
	customProviders: string[];
}

/**
 * Import conflict information.
 */
export interface ImportConflict {
	entityType: 'agent' | 'mcp' | 'model' | 'prompt' | 'skill' | 'custom_provider';
	entityName: string;
	existingId: string;
}

/** How to resolve an import conflict. */
export type ConflictResolution = 'skip' | 'overwrite' | 'rename';

/** Additional env vars/args for MCP import. */
export interface MCPAdditions {
	addEnv: Record<string, string>;
	addArgs: string[];
}

// ============ IMPORT WARNING (v1.1) ============

/** Category of import warning. */
export type ImportWarningType =
	| 'missing_dependency'
	| 'machine_specific'
	| 'default_applied'
	| 'builtin_model';

/**
 * Structured import warning with actionable context (v1.1).
 */
export interface ImportWarning {
	warningType: ImportWarningType;
	/** "info" | "medium" | "high" */
	severity: string;
	/** Which entity is affected */
	entity: string;
	/** What the problem is */
	detail: string;
	/** What the user should do */
	action: string;
}

// ============ IMPORT VALIDATION & RESULT ============

/**
 * Import validation result with structured warnings (v1.1).
 */
export interface ImportValidation {
	valid: boolean;
	schemaVersion: string;
	errors: string[];
	warnings: ImportWarning[];
	entities: ImportEntities;
	conflicts: ImportConflict[];
	missingMcpEnv: Record<string, string[]>;
}

/** Entity summaries from import file. */
export interface ImportEntities {
	agents: AgentExportSummary[];
	mcpServers: MCPServerExportSummary[];
	models: LLMModelExportSummary[];
	prompts: PromptExportSummary[];
	skills: SkillExportSummary[];
	customProviders: CustomProviderExportSummary[];
}

/**
 * Import operation result (v1.1: added postImportActions).
 */
export interface ConfigImportResult {
	success: boolean;
	imported: ImportCounts;
	skipped: ImportCounts;
	errors: ImportError[];
	/** Actionable items for the user to check after import */
	postImportActions: string[];
}

/** Entity import counts. */
export interface ImportCounts {
	agents: number;
	mcpServers: number;
	models: number;
	prompts: number;
	skills: number;
	customProviders: number;
}

/** Individual entity import error. */
export interface ImportError {
	entityType: 'agent' | 'mcp' | 'model' | 'prompt' | 'skill' | 'custom_provider';
	entityId: string;
	error: string;
}

// ============ CONSTANTS ============

/** Current schema version for export packages */
export const EXPORT_SCHEMA_VERSION = '1.1';

/** Maximum import file size in bytes (10MB) */
export const MAX_IMPORT_FILE_SIZE = 10 * 1024 * 1024;

/** Sensitive env var key patterns to warn about */
export const SENSITIVE_ENV_PATTERNS = [
	'API_KEY',
	'SECRET',
	'TOKEN',
	'PASSWORD',
	'CREDENTIAL',
	'PRIVATE_KEY'
];
