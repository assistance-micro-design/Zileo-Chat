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
 * @fileoverview Typed Tauri API client for the settings domain.
 *
 * Covers Kanban/MCP-network/STT settings, API-key storage, import/export,
 * the file-manager trash, and schema migrations.
 *
 * @module lib/api/settings
 */

import { invokeCommand } from './internal';
import type { KanbanSettings, UpdateKanbanSettingsRequest } from '$types/kanban-settings';
import type {
	McpNetworkSettings,
	UpdateMcpNetworkSettingsRequest
} from '$types/mcp-network';
import type { STTSettings, UpdateSTTSettingsRequest } from '$types/stt';
import type {
	ConfigImportResult,
	ConflictResolution,
	ExportOptions,
	ExportPreviewData,
	ExportSelection,
	ImportSelection,
	ImportValidation,
	MCPAdditions,
	MCPSanitizationConfig
} from '$types/import-export';

/** Result of a schema migration. Mirrors Rust `MigrationResult` (snake_case). */
export interface MigrationResult {
	success: boolean;
	message: string;
	records_affected: number;
}

/** Memory schema status. Mirrors Rust `MemorySchemaStatus` (snake_case). */
export interface MemorySchemaStatus {
	total_memories: number;
	with_embeddings: number;
	without_embeddings: number;
	with_workflow_id: number;
	hnsw_dimension: number;
}

/** A trash entry. Mirrors Rust `TrashEntry` (snake_case). */
export interface TrashEntry {
	trash_path: string;
	original_relative_path: string;
	deleted_at: string;
	size_bytes: number;
}

// ============================================================================
// Kanban settings
// ============================================================================

/** Get Kanban settings. */
export async function getKanbanSettings(): Promise<KanbanSettings> {
	return invokeCommand<KanbanSettings>('get_kanban_settings');
}

/** Update Kanban settings. */
export async function updateKanbanSettings(
	request: UpdateKanbanSettingsRequest
): Promise<KanbanSettings> {
	return invokeCommand<KanbanSettings>('update_kanban_settings', { request });
}

// ============================================================================
// MCP network settings
// ============================================================================

/** Get MCP network settings. */
export async function getMcpNetworkSettings(): Promise<McpNetworkSettings> {
	return invokeCommand<McpNetworkSettings>('get_mcp_network_settings');
}

/** Update MCP network settings. */
export async function updateMcpNetworkSettings(
	request: UpdateMcpNetworkSettingsRequest
): Promise<McpNetworkSettings> {
	return invokeCommand<McpNetworkSettings>('update_mcp_network_settings', { request });
}

// ============================================================================
// Speech-to-text settings
// ============================================================================

/** Get STT settings. */
export async function getSttSettings(): Promise<STTSettings> {
	return invokeCommand<STTSettings>('get_stt_settings');
}

/** Update STT settings. */
export async function updateSttSettings(
	config: UpdateSTTSettingsRequest
): Promise<STTSettings> {
	return invokeCommand<STTSettings>('update_stt_settings', { config });
}

/** Reset STT settings to defaults. */
export async function resetSttSettings(): Promise<STTSettings> {
	return invokeCommand<STTSettings>('reset_stt_settings');
}

// ============================================================================
// API keys (secure storage)
// ============================================================================

/** Securely store a provider API key. */
export async function saveApiKey(provider: string, apiKey: string): Promise<void> {
	return invokeCommand<void>('save_api_key', { provider, apiKey });
}

/** Remove a stored provider API key. */
export async function deleteApiKey(provider: string): Promise<void> {
	return invokeCommand<void>('delete_api_key', { provider });
}

/** Check whether a provider key exists (without exposing it). */
export async function hasApiKey(provider: string): Promise<boolean> {
	return invokeCommand<boolean>('has_api_key', { provider });
}

/** List providers with a stored API key. */
export async function listApiKeyProviders(): Promise<string[]> {
	return invokeCommand<string[]>('list_api_key_providers');
}

// ============================================================================
// Import / export
// ============================================================================

/** Validate an import payload. */
export async function validateImport(data: string): Promise<ImportValidation> {
	return invokeCommand<ImportValidation>('validate_import', { data });
}

/** Input for `executeImport`. */
export interface ExecuteImportInput {
	data: string;
	selection: ImportSelection;
	resolutions: Record<string, ConflictResolution>;
	mcpAdditions: Record<string, MCPAdditions>;
}

/** Execute an import. */
export async function executeImport(input: ExecuteImportInput): Promise<ConfigImportResult> {
	return invokeCommand<ConfigImportResult>('execute_import', {
		data: input.data,
		selection: input.selection,
		resolutions: input.resolutions,
		mcpAdditions: input.mcpAdditions
	});
}

/** Preview an export package. */
export async function prepareExportPreview(
	selection: ExportSelection
): Promise<ExportPreviewData> {
	return invokeCommand<ExportPreviewData>('prepare_export_preview', { selection });
}

/** Generate an export file, returning its content. */
export async function generateExportFile(
	selection: ExportSelection,
	options: ExportOptions,
	sanitization: Record<string, MCPSanitizationConfig>
): Promise<string> {
	return invokeCommand<string>('generate_export_file', { selection, options, sanitization });
}

/** Write export content to a file. Returns bytes written. */
export async function saveExportToFile(path: string, content: string): Promise<number> {
	return invokeCommand<number>('save_export_to_file', { path, content });
}

// ============================================================================
// Trash (file manager)
// ============================================================================

/** List trash entries of a folder. */
export async function listTrash(folderPath: string): Promise<TrashEntry[]> {
	return invokeCommand<TrashEntry[]>('list_trash', { folderPath });
}

/** Restore a file from trash, returning its restored path. */
export async function restoreFromTrash(trashPath: string, folderPath: string): Promise<string> {
	return invokeCommand<string>('restore_from_trash_cmd', { trashPath, folderPath });
}

// ============================================================================
// Migrations
// ============================================================================

/** Get the memory schema status. */
export async function getMemorySchemaStatus(): Promise<MemorySchemaStatus> {
	return invokeCommand<MemorySchemaStatus>('get_memory_schema_status');
}

/** Migrate the MCP auth schema. */
export async function migrateMcpAuthV1(): Promise<MigrationResult> {
	return invokeCommand<MigrationResult>('migrate_mcp_auth_v1');
}

/** Migrate the MCP HTTP schema. */
export async function migrateMcpHttpSchema(): Promise<MigrationResult> {
	return invokeCommand<MigrationResult>('migrate_mcp_http_schema');
}

/** Migrate the memory v2 schema. */
export async function migrateMemoryV2Schema(): Promise<MigrationResult> {
	return invokeCommand<MigrationResult>('migrate_memory_v2_schema');
}

/** Migrate reasoning-effort fields. */
export async function migrateReasoningEffort(): Promise<MigrationResult> {
	return invokeCommand<MigrationResult>('migrate_reasoning_effort');
}

/** Migrate sidebar features. */
export async function migrateSidebarFeatures(): Promise<MigrationResult> {
	return invokeCommand<MigrationResult>('migrate_sidebar_features');
}

/** Migrate token-cost accuracy fields. */
export async function migrateTokenCostAccuracyV1(): Promise<MigrationResult> {
	return invokeCommand<MigrationResult>('migrate_token_cost_accuracy_v1');
}
