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
