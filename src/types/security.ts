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
 * Security-related types for API key management.
 *
 * These types correspond to the Tauri security commands for storing
 * and retrieving API keys securely using OS keychain + AES-256 encryption.
 */

/**
 * Supported LLM provider names.
 *
 * These are the providers that can have API keys stored.
 */
export type LLMProvider =
	| 'Mistral'
	| 'Ollama'
	| 'OpenAI'
	| 'Anthropic'
	| 'Google'
	| 'Cohere'
	| 'HuggingFace';

/**
 * Result of checking if an API key exists.
 */
export interface ApiKeyStatus {
	/** The provider name */
	provider: string;
	/** Whether the provider has an API key stored */
	exists: boolean;
}

/**
 * Security settings for the application.
 */
export interface SecuritySettings {
	/** List of providers with stored API keys */
	configuredProviders: string[];
}
