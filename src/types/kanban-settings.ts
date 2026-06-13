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
 * @fileoverview Kanban settings.
 *
 * Mirrors `src-tauri/src/commands/settings_kanban.rs` (`KanbanSettings`,
 * `#[serde(rename_all = "camelCase")]`). Update both sides together.
 */

/** Lower bound for the configurable compose timeout (seconds). */
export const COMPOSE_TIMEOUT_MIN_SECS = 60;

/** Upper bound for the configurable compose timeout (seconds). */
export const COMPOSE_TIMEOUT_MAX_SECS = 1800;

/**
 * Default compose timeout (seconds). Mirror of the Rust
 * `COMPOSE_TIMEOUT_DEFAULT_SECS` so the UI seeds the same value the backend
 * returns when no row is stored (closes the TS-Rust micro-drift).
 */
export const COMPOSE_TIMEOUT_DEFAULT_SECS = 600;

/** Persisted Kanban settings stored in `settings:kanban.config`. */
export interface KanbanSettings {
	/**
	 * Hard wall-clock ceiling (seconds) for a single detached card compose run.
	 * Clamped to [COMPOSE_TIMEOUT_MIN_SECS, COMPOSE_TIMEOUT_MAX_SECS].
	 */
	composeTimeoutSecs: number;
	/**
	 * Global supervisor agent for the compose flow. Optional (Rust
	 * `Option<String>` + skip-when-none); absent/undefined falls back to the
	 * agent passed by the card creator.
	 */
	composeAgentId?: string;
	/**
	 * Global supervisor agent for the analyze flow. Optional; absent/undefined
	 * falls back to the card's own `kanban_agent_id`.
	 */
	analyzeAgentId?: string;
}

/**
 * Partial update payload — only provided fields are applied.
 *
 * The agent ids are tri-state: omit a key to leave it unchanged, send `null`
 * (or an empty string) to clear it back to the fallback, or send a value to set
 * it.
 */
export interface UpdateKanbanSettingsRequest {
	composeTimeoutSecs?: number;
	composeAgentId?: string | null;
	analyzeAgentId?: string | null;
}
