// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * LLM provider types supported by the application
 */
export type ProviderType = 'mistral' | 'ollama';

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
 * Available Mistral models
 */
export const MISTRAL_MODELS = [
  'mistral-large-latest',
  'mistral-medium-latest',
  'mistral-small-latest',
  'open-mistral-7b',
  'open-mixtral-8x7b',
  'open-mixtral-8x22b',
  'codestral-latest',
] as const;

/**
 * Available Ollama models (common defaults)
 */
export const OLLAMA_MODELS = [
  'llama3.2',
  'llama3.1',
  'llama3',
  'mistral',
  'mixtral',
  'codellama',
  'phi3',
  'gemma2',
  'qwen2.5',
] as const;

/**
 * Default models per provider
 */
export const DEFAULT_MODELS = {
  mistral: 'mistral-large-latest',
  ollama: 'llama3.2',
} as const;

/**
 * Default Ollama server URL
 */
export const DEFAULT_OLLAMA_URL = 'http://localhost:11434';
