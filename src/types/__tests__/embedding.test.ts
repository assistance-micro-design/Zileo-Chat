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
 * @fileoverview Tests for embedding types and constants.
 *
 * Tests verify:
 * - EMBEDDING_MODELS constant structure and values
 * - DEFAULT_EMBEDDING_CONFIG validation
 * - Type compatibility with Rust backend
 */

import { describe, it, expect } from 'vitest';
import {
	EMBEDDING_MODELS,
	DEFAULT_EMBEDDING_CONFIG,
	type EmbeddingConfig,
	type EmbeddingProviderType,
	type MemoryStats,
	type ImportResult,
	type ExportFormat
} from '../embedding';

describe('Embedding Types', () => {
	// =========================================================================
	// EMBEDDING_MODELS constant tests
	// =========================================================================

	describe('EMBEDDING_MODELS', () => {
		it('should have mistral provider with models', () => {
			expect(EMBEDDING_MODELS.mistral).toBeDefined();
			expect(EMBEDDING_MODELS.mistral.length).toBeGreaterThan(0);
		});

		it('should have ollama provider with models', () => {
			expect(EMBEDDING_MODELS.ollama).toBeDefined();
			expect(EMBEDDING_MODELS.ollama.length).toBeGreaterThan(0);
		});

		it('should have mistral-embed model with 1024 dimensions', () => {
			const mistralEmbed = EMBEDDING_MODELS.mistral.find((m) => m.value === 'mistral-embed');
			expect(mistralEmbed).toBeDefined();
			expect(mistralEmbed?.dimension).toBe(1024);
			expect(mistralEmbed?.label).toContain('1024D');
		});

		it('should have mxbai-embed-large model with 1024 dimensions', () => {
			const mxbaiEmbed = EMBEDDING_MODELS.ollama.find((m) => m.value === 'mxbai-embed-large');
			expect(mxbaiEmbed).toBeDefined();
			expect(mxbaiEmbed?.dimension).toBe(1024);
		});

		it('should exclude nomic-embed-text (incompatible with HNSW 1024D index)', () => {
			const nomic = EMBEDDING_MODELS.ollama.find((m) => m.value === 'nomic-embed-text');
			expect(nomic).toBeUndefined();
		});

		it('should only list models compatible with HNSW 1024D index', () => {
			const allModels = [...EMBEDDING_MODELS.mistral, ...EMBEDDING_MODELS.ollama];
			for (const model of allModels) {
				expect(model.dimension).toBe(1024);
			}
		});

		it('should have valid model structure for all providers', () => {
			const providers: EmbeddingProviderType[] = ['mistral', 'ollama'];

			for (const provider of providers) {
				const models = EMBEDDING_MODELS[provider];
				expect(Array.isArray(models)).toBe(true);

				for (const model of models) {
					expect(typeof model.value).toBe('string');
					expect(typeof model.label).toBe('string');
					expect(typeof model.dimension).toBe('number');
					expect(model.dimension).toBeGreaterThan(0);
					expect(model.value.length).toBeGreaterThan(0);
					expect(model.label.length).toBeGreaterThan(0);
				}
			}
		});
	});

	// =========================================================================
	// DEFAULT_EMBEDDING_CONFIG tests
	// =========================================================================

	describe('DEFAULT_EMBEDDING_CONFIG', () => {
		it('should use mistral as default provider', () => {
			expect(DEFAULT_EMBEDDING_CONFIG.provider).toBe('mistral');
		});

		it('should use mistral-embed as default model', () => {
			expect(DEFAULT_EMBEDDING_CONFIG.model).toBe('mistral-embed');
		});

		it('should be a valid EmbeddingConfig type', () => {
			const config: EmbeddingConfig = DEFAULT_EMBEDDING_CONFIG;

			expect(config.provider).toBeDefined();
			expect(config.model).toBeDefined();
		});
	});

	// =========================================================================
	// Type compatibility tests
	// =========================================================================

	describe('Type Compatibility', () => {
		it('should accept valid EmbeddingProviderType values', () => {
			const providers: EmbeddingProviderType[] = ['mistral', 'ollama'];
			expect(providers).toContain('mistral');
			expect(providers).toContain('ollama');
		});

		it('should accept valid ExportFormat values', () => {
			const formats: ExportFormat[] = ['json', 'csv'];
			expect(formats).toContain('json');
			expect(formats).toContain('csv');
		});

		it('should create valid MemoryStats structure', () => {
			const stats: MemoryStats = {
				total: 100,
				with_embeddings: 80,
				without_embeddings: 20,
				by_type: { knowledge: 50, decision: 30, context: 15, user_pref: 5 },
				by_agent: { main_agent: 60, helper_agent: 40 }
			};

			expect(stats.total).toBe(100);
			expect(stats.with_embeddings + stats.without_embeddings).toBe(stats.total);
			expect(Object.values(stats.by_type).reduce((a, b) => a + b, 0)).toBe(stats.total);
		});

		it('should create valid ImportResult structure', () => {
			const result: ImportResult = {
				imported: 95,
				failed: 5,
				errors: ['Invalid format on line 42', 'Missing content field on line 67']
			};

			expect(result.imported).toBe(95);
			expect(result.failed).toBe(5);
			expect(result.errors.length).toBe(2);
		});
	});

	// =========================================================================
	// Configuration validation helpers
	// =========================================================================

	describe('Configuration Validation', () => {
		it('should ensure default model matches default provider', () => {
			const defaultModel = EMBEDDING_MODELS[DEFAULT_EMBEDDING_CONFIG.provider].find(
				(m) => m.value === DEFAULT_EMBEDDING_CONFIG.model
			);
			expect(defaultModel).toBeDefined();
		});

		it('should have all embedding models with positive dimensions', () => {
			const allModels = [...EMBEDDING_MODELS.mistral, ...EMBEDDING_MODELS.ollama];

			for (const model of allModels) {
				expect(model.dimension).toBeGreaterThan(0);
				// Typical embedding dimensions are 768, 1024, 1536, 3072
				expect(model.dimension).toBeGreaterThanOrEqual(256);
				expect(model.dimension).toBeLessThanOrEqual(4096);
			}
		});
	});
});
