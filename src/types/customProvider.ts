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
 * Custom provider types for OpenAI-compatible providers.
 *
 * Synchronized with src-tauri/src/models/custom_provider.rs
 * @module types/customProvider
 */

/**
 * Provider metadata returned by `list_providers`.
 *
 * Unified representation of both builtin (Mistral, Ollama) and
 * custom (RouterLab, OpenRouter, etc.) providers.
 */
export interface ProviderInfo {
	/** Provider identifier (e.g., "mistral", "ollama", "routerlab") */
	id: string;
	/** Human-readable display name */
	displayName: string;
	/** Whether this is a builtin provider */
	isBuiltin: boolean;
	/** Whether this provider requires internet (cloud API) */
	isCloud: boolean;
	/** Whether an API key is required */
	requiresApiKey: boolean;
	/** Whether the provider has a configurable base URL */
	hasBaseUrl: boolean;
	/** Base URL (for custom providers and Ollama) */
	baseUrl: string | null;
	/** Whether the provider is enabled */
	enabled: boolean;
}

/**
 * Request payload for creating a new custom provider.
 */
export interface CreateCustomProviderRequest {
	/** URL-safe identifier (lowercase alphanumeric + hyphens, 1-64 chars) */
	name: string;
	/** Human-readable display name (1-128 chars) */
	displayName: string;
	/** API base URL (e.g., "https://api.routerlab.ch/v1") */
	baseUrl: string;
	/** API key for authentication */
	apiKey: string;
}
