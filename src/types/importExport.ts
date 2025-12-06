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

// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Import/Export Settings Types
 *
 * Types for exporting and importing configuration entities (Agents, MCP Servers, Models, Prompts).
 * Synchronized with src-tauri/src/models/import_export.rs
 *
 * @module types/importExport
 */

import type { AgentSummary, LLMConfig } from './agent';
import type { PromptSummary } from './prompt';

// ============ EXPORT TYPES ============

/**
 * Selection of entities to export.
 * At least one entity must be selected.
 */
export interface ExportSelection {
	/** Agent IDs to export */
	agents: string[];
	/** MCP Server IDs to export */
	mcpServers: string[];
	/** Model IDs to export (custom only recommended) */
	models: string[];
	/** Prompt IDs to export */
	prompts: string[];
}

/**
 * Export configuration options.
 */
export interface ExportOptions {
	/** Export format (JSON only in Phase 1) */
	format: 'json';
	/** Whether to include created_at/updated_at timestamps */
	includeTimestamps: boolean;
	/** Whether to enable MCP env var sanitization UI */
	sanitizeMcp: boolean;
}

/**
 * MCP server sanitization configuration for export.
 * Allows clearing or modifying sensitive environment variables before export.
 */
export interface MCPSanitizationConfig {
	/** Env var keys to clear (set to empty string) */
	clearEnvKeys: string[];
	/** Env var values to modify/override */
	modifyEnv: Record<string, string>;
	/** Modified command args (optional) */
	modifyArgs: string[];
	/** If true, skip this server entirely from export */
	excludeFromExport: boolean;
}

/**
 * Export manifest with metadata about the export.
 */
export interface ExportManifest {
	/** Schema version for compatibility checking */
	version: string;
	/** Application version that created the export */
	appVersion: string;
	/** ISO 8601 timestamp of export */
	exportedAt: string;
	/** Optional identifier of who exported */
	exportedBy?: string;
	/** Optional user description of the export */
	description?: string;
	/** Entity counts in the export */
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
}

/**
 * Complete export package containing manifest and all entities.
 * This is the structure of the exported JSON file.
 */
export interface ExportPackage {
	/** Export metadata */
	manifest: ExportManifest;
	/** Exported agent configurations */
	agents: AgentExportData[];
	/** Exported MCP server configurations */
	mcpServers: MCPServerExportData[];
	/** Exported LLM model definitions */
	models: LLMModelExportData[];
	/** Exported prompt templates */
	prompts: PromptExportData[];
}

/**
 * Agent data for export (includes timestamps if enabled).
 * Note: IDs are NOT exported - entities are identified by NAME.
 * A new UUID is generated on import.
 */
export interface AgentExportData {
	/** Agent name - used as unique identifier for import conflict detection */
	name: string;
	lifecycle: 'permanent' | 'temporary';
	llm: LLMConfig;
	tools: string[];
	mcpServers: string[];
	systemPrompt: string;
	maxToolIterations: number;
	createdAt?: string;
	updatedAt?: string;
}

/**
 * MCP Server data for export.
 * Note: IDs are NOT exported - entities are identified by NAME.
 * A new UUID is generated on import.
 */
export interface MCPServerExportData {
	/** Server name - used as unique identifier for import conflict detection */
	name: string;
	enabled: boolean;
	command: 'docker' | 'npx' | 'uvx' | 'http';
	args: string[];
	env: Record<string, string>;
	description?: string;
	createdAt?: string;
	updatedAt?: string;
}

/**
 * LLM Model data for export.
 * Note: IDs are NOT exported - entities are identified by NAME.
 * A new UUID is generated on import.
 */
export interface LLMModelExportData {
	provider: 'mistral' | 'ollama';
	/** Model name - used as unique identifier for import conflict detection */
	name: string;
	apiName: string;
	contextWindow: number;
	maxOutputTokens: number;
	temperatureDefault: number;
	isBuiltin: boolean;
	isReasoning: boolean;
	inputPricePerMtok: number;
	outputPricePerMtok: number;
	createdAt?: string;
	updatedAt?: string;
}

/**
 * Prompt data for export.
 * Note: IDs are NOT exported - entities are identified by NAME.
 * A new UUID is generated on import.
 */
export interface PromptExportData {
	/** Prompt name - used as unique identifier for import conflict detection */
	name: string;
	description: string;
	category: 'system' | 'user' | 'analysis' | 'generation' | 'coding' | 'custom';
	content: string;
	createdAt?: string;
	updatedAt?: string;
}

/**
 * Preview data returned before finalizing export.
 * Used to show the user what will be exported and allow MCP sanitization.
 */
export interface ExportPreviewData {
	/** Agent summaries */
	agents: AgentSummary[];
	/** MCP server summaries */
	mcpServers: MCPServerSummary[];
	/** Model summaries */
	models: LLMModelSummary[];
	/** Prompt summaries */
	prompts: PromptSummary[];
	/** Map of server_id -> env var key names (for sanitization UI) */
	mcpEnvKeys: Record<string, string[]>;
}

/**
 * MCP server summary for preview.
 * ID is present for export preview (from DB), absent for import preview.
 */
export interface MCPServerSummary {
	/** ID is present for export preview, absent for import preview */
	id?: string;
	/** Name is the unique identifier for import */
	name: string;
	enabled: boolean;
	command: string;
	toolsCount: number;
}

/**
 * LLM model summary for preview.
 * ID is present for export preview (from DB), absent for import preview.
 */
export interface LLMModelSummary {
	/** ID is present for export preview, absent for import preview */
	id?: string;
	/** Name is the unique identifier for import */
	name: string;
	provider: string;
	apiName: string;
	isBuiltin: boolean;
}

/**
 * Export operation result.
 */
export interface ExportResult {
	/** Whether export succeeded */
	success: boolean;
	/** Suggested filename */
	filename: string;
	/** File size in bytes */
	sizeBytes: number;
	/** Entity counts exported */
	counts: ExportCounts;
}

// ============ IMPORT TYPES ============

/**
 * Selection of entities to import from the package.
 * Note: These are entity NAMES, not IDs (IDs are not in the export file).
 */
export interface ImportSelection {
	/** Agent names to import */
	agents: string[];
	/** MCP Server names to import */
	mcpServers: string[];
	/** Model names to import */
	models: string[];
	/** Prompt names to import */
	prompts: string[];
}

/**
 * Import conflict information.
 * Conflicts are detected by NAME only (IDs are not exported).
 */
export interface ImportConflict {
	/** Type of entity with conflict */
	entityType: 'agent' | 'mcp' | 'model' | 'prompt';
	/** Name of the entity being imported - used as unique identifier */
	entityName: string;
	/** ID of the existing entity in the database */
	existingId: string;
}

/**
 * How to resolve an import conflict.
 */
export type ConflictResolution = 'skip' | 'overwrite' | 'rename';

/**
 * Additional env vars/args to add when importing an MCP server.
 * Used when the import is missing required environment variables.
 */
export interface MCPAdditions {
	/** Additional environment variables to set */
	addEnv: Record<string, string>;
	/** Additional command arguments */
	addArgs: string[];
}

/**
 * Import validation result.
 * Returned after parsing the import file to show preview and conflicts.
 */
export interface ImportValidation {
	/** Whether the import file is valid */
	valid: boolean;
	/** Schema version of the import file */
	schemaVersion: string;
	/** Validation errors (blocking) */
	errors: string[];
	/** Validation warnings (non-blocking) */
	warnings: string[];
	/** Entities found in the import file */
	entities: ImportEntities;
	/** Detected conflicts with existing entities */
	conflicts: ImportConflict[];
	/** Map of server_id -> missing required env var keys */
	missingMcpEnv: Record<string, string[]>;
}

/**
 * Entity summaries from import file.
 */
export interface ImportEntities {
	agents: AgentSummary[];
	mcpServers: MCPServerSummary[];
	models: LLMModelSummary[];
	prompts: PromptSummary[];
}

/**
 * Import operation result for settings import.
 * Named ConfigImportResult to avoid conflict with embedding.ts ImportResult.
 */
export interface ConfigImportResult {
	/** Whether import completed (may have partial failures) */
	success: boolean;
	/** Number of entities successfully imported per type */
	imported: ImportCounts;
	/** Number of entities skipped per type */
	skipped: ImportCounts;
	/** Import errors for individual entities */
	errors: ImportError[];
}

/**
 * Entity import counts.
 */
export interface ImportCounts {
	agents: number;
	mcpServers: number;
	models: number;
	prompts: number;
}

/**
 * Individual entity import error.
 */
export interface ImportError {
	entityType: 'agent' | 'mcp' | 'model' | 'prompt';
	entityId: string;
	error: string;
}

// ============ CONSTANTS ============

/** Current schema version for export packages */
export const EXPORT_SCHEMA_VERSION = '1.0';

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
