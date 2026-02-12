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
 * Builtin provider identifiers.
 */
export type BuiltinProvider = 'mistral' | 'ollama';

/**
 * LLM provider types supported by the application.
 *
 * Extends beyond builtins to support custom OpenAI-compatible providers
 * (e.g., "routerlab", "openrouter", "together-ai").
 */
export type ProviderType = BuiltinProvider | string;

/**
 * LLM response from a completion request
 */
export interface LLMResponse {
  /** Generated text content */
  content: string;
  /** Number of input tokens (prompt) */
  tokens_input: number;
  /** Number of output tokens (completion) */
  tokens_output: number;
  /** Model used for generation */
  model: string;
  /** Provider used */
  provider: ProviderType;
  /** Finish reason (if available) */
  finish_reason: string | null;
}

/**
 * Provider status information
 */
export interface ProviderStatus {
  /** Provider type */
  provider: string;
  /** Whether the provider is configured */
  configured: boolean;
  /** Current default model */
  default_model: string;
  /** Available models */
  available_models: string[];
}

/**
 * Complete LLM configuration response
 */
export interface LLMConfigResponse {
  /** Active provider */
  active_provider: string;
  /** Mistral configuration status */
  mistral: ProviderStatus;
  /** Ollama configuration status */
  ollama: ProviderStatus;
  /** Ollama server URL */
  ollama_url: string;
}

/**
 * Default Ollama server URL
 */
export const DEFAULT_OLLAMA_URL = 'http://localhost:11434';

// ============================================================================
// LLM Model Types (CRUD)
// ============================================================================

/**
 * LLM model definition (builtin or custom).
 *
 * Models can be either builtin (shipped with the application and immutable)
 * or custom (user-created and fully editable).
 */
export interface LLMModel {
  /** Unique identifier (UUID for custom, api_name for builtin) */
  id: string;
  /** Provider this model belongs to */
  provider: ProviderType;
  /** Human-readable display name */
  name: string;
  /** Model identifier used in API calls (e.g., "mistral-large-latest") */
  api_name: string;
  /** Maximum context length in tokens (1024 - 2,000,000) */
  context_window: number;
  /** Maximum generation length in tokens (256 - 128,000) */
  max_output_tokens: number;
  /** Default sampling temperature (0.0 - 2.0) */
  temperature_default: number;
  /** Whether this is a builtin model (cannot be deleted) */
  is_builtin: boolean;
  /** Whether this is a reasoning/thinking model (Magistral, DeepSeek-R1, etc.) */
  is_reasoning: boolean;
  /** Price per million input tokens (USD) - user configurable */
  input_price_per_mtok: number;
  /** Price per million output tokens (USD) - user configurable */
  output_price_per_mtok: number;
  /** ISO 8601 creation timestamp */
  created_at: string;
  /** ISO 8601 last update timestamp */
  updated_at: string;
}

/**
 * Request payload for creating a new custom model.
 *
 * All fields except temperature_default and is_reasoning are required.
 */
export interface CreateModelRequest {
  /** Provider this model belongs to */
  provider: ProviderType;
  /** Human-readable display name (1-64 characters) */
  name: string;
  /** Model identifier used in API calls (must be unique per provider) */
  api_name: string;
  /** Maximum context length in tokens (1024 - 2,000,000) */
  context_window: number;
  /** Maximum generation length in tokens (256 - 128,000) */
  max_output_tokens: number;
  /** Default sampling temperature (0.0 - 2.0, defaults to 0.7) */
  temperature_default?: number;
  /** Whether this is a reasoning/thinking model (defaults to false) */
  is_reasoning?: boolean;
  /** Price per million input tokens (USD, default 0.0) */
  input_price_per_mtok?: number;
  /** Price per million output tokens (USD, default 0.0) */
  output_price_per_mtok?: number;
}

/**
 * Request payload for updating an existing model.
 *
 * All fields are optional. Only provided fields will be updated.
 * For builtin models, only temperature_default and is_reasoning can be modified.
 */
export interface UpdateModelRequest {
  /** New display name (1-64 characters) */
  name?: string;
  /** New API name (must be unique per provider) */
  api_name?: string;
  /** New context window size (1024 - 2,000,000) */
  context_window?: number;
  /** New max output tokens (256 - 128,000) */
  max_output_tokens?: number;
  /** New default temperature (0.0 - 2.0) */
  temperature_default?: number;
  /** Whether this is a reasoning/thinking model */
  is_reasoning?: boolean;
  /** New price per million input tokens (USD) */
  input_price_per_mtok?: number;
  /** New price per million output tokens (USD) */
  output_price_per_mtok?: number;
}

/**
 * Configuration settings for a provider.
 *
 * Stores per-provider settings including enabled state, default model,
 * and optional base URL (primarily for Ollama).
 */
export interface ProviderSettings {
  /** Provider type */
  provider: ProviderType;
  /** Whether this provider is enabled */
  enabled: boolean;
  /** ID of the default model for this provider */
  default_model_id: string | null;
  /** Whether an API key is configured (for Mistral) */
  api_key_configured: boolean;
  /** Custom base URL (primarily for Ollama) */
  base_url: string | null;
  /** ISO 8601 last update timestamp */
  updated_at: string;
}

/**
 * Result of a provider connection test.
 *
 * Contains success status, latency measurement, and any error details.
 */
export interface ConnectionTestResult {
  /** Provider that was tested */
  provider: ProviderType;
  /** Whether the connection was successful */
  success: boolean;
  /** Round-trip latency in milliseconds (if successful) */
  latency_ms: number | null;
  /** Error message (if failed) */
  error_message: string | null;
  /** Model used for the test (if applicable) */
  model_tested: string | null;
}

/**
 * State structure for the LLM store.
 *
 * Centralizes all LLM-related state for the settings UI.
 */
export interface LLMState {
  /** Provider settings indexed by provider id */
  providers: Record<string, ProviderSettings | null>;
  /** All loaded models (builtin + custom) */
  models: LLMModel[];
  /** Currently active provider */
  activeProvider: ProviderType | null;
  /** Whether data is being loaded */
  loading: boolean;
  /** Error message if an operation failed */
  error: string | null;
  /** Provider currently being tested for connection */
  testingProvider: ProviderType | null;
}
