// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

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
	type ChunkingStrategy,
	type MemoryStats,
	type ImportResult,
	type RegenerateResult,
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
			const mistralEmbed = EMBEDDING_MODELS.mistral.find(
				(m) => m.value === 'mistral-embed'
			);
			expect(mistralEmbed).toBeDefined();
			expect(mistralEmbed?.dimension).toBe(1024);
			expect(mistralEmbed?.label).toContain('1024D');
		});

		it('should have nomic-embed-text model with 768 dimensions', () => {
			const nomicEmbed = EMBEDDING_MODELS.ollama.find(
				(m) => m.value === 'nomic-embed-text'
			);
			expect(nomicEmbed).toBeDefined();
			expect(nomicEmbed?.dimension).toBe(768);
		});

		it('should have mxbai-embed-large model with 1024 dimensions', () => {
			const mxbaiEmbed = EMBEDDING_MODELS.ollama.find(
				(m) => m.value === 'mxbai-embed-large'
			);
			expect(mxbaiEmbed).toBeDefined();
			expect(mxbaiEmbed?.dimension).toBe(1024);
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

		it('should have 1024 as default dimension', () => {
			expect(DEFAULT_EMBEDDING_CONFIG.dimension).toBe(1024);
		});

		it('should have reasonable max_tokens value', () => {
			expect(DEFAULT_EMBEDDING_CONFIG.max_tokens).toBeGreaterThan(0);
			expect(DEFAULT_EMBEDDING_CONFIG.max_tokens).toBe(8192);
		});

		it('should have valid chunk_size between 100 and 2000', () => {
			expect(DEFAULT_EMBEDDING_CONFIG.chunk_size).toBeGreaterThanOrEqual(100);
			expect(DEFAULT_EMBEDDING_CONFIG.chunk_size).toBeLessThanOrEqual(2000);
		});

		it('should have valid chunk_overlap between 0 and 500', () => {
			expect(DEFAULT_EMBEDDING_CONFIG.chunk_overlap).toBeGreaterThanOrEqual(0);
			expect(DEFAULT_EMBEDDING_CONFIG.chunk_overlap).toBeLessThanOrEqual(500);
		});

		it('should have fixed as default strategy', () => {
			expect(DEFAULT_EMBEDDING_CONFIG.strategy).toBe('fixed');
		});

		it('should be a valid EmbeddingConfig type', () => {
			const config: EmbeddingConfig = DEFAULT_EMBEDDING_CONFIG;

			expect(config.provider).toBeDefined();
			expect(config.model).toBeDefined();
			expect(config.dimension).toBeDefined();
			expect(config.max_tokens).toBeDefined();
			expect(config.chunk_size).toBeDefined();
			expect(config.chunk_overlap).toBeDefined();
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

		it('should accept valid ChunkingStrategy values', () => {
			const strategies: ChunkingStrategy[] = ['fixed', 'semantic', 'recursive'];
			expect(strategies).toContain('fixed');
			expect(strategies).toContain('semantic');
			expect(strategies).toContain('recursive');
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

		it('should create valid RegenerateResult structure', () => {
			const result: RegenerateResult = {
				processed: 100,
				success: 98,
				failed: 2
			};

			expect(result.processed).toBe(100);
			expect(result.success + result.failed).toBe(result.processed);
		});
	});

	// =========================================================================
	// Configuration validation helpers
	// =========================================================================

	describe('Configuration Validation', () => {
		it('should validate chunk_overlap is less than chunk_size', () => {
			const config = DEFAULT_EMBEDDING_CONFIG;
			expect(config.chunk_overlap).toBeLessThan(config.chunk_size);
		});

		it('should ensure default model matches default provider', () => {
			const defaultModel = EMBEDDING_MODELS[DEFAULT_EMBEDDING_CONFIG.provider].find(
				(m) => m.value === DEFAULT_EMBEDDING_CONFIG.model
			);
			expect(defaultModel).toBeDefined();
		});

		it('should ensure default dimension matches default model', () => {
			const defaultModel = EMBEDDING_MODELS[DEFAULT_EMBEDDING_CONFIG.provider].find(
				(m) => m.value === DEFAULT_EMBEDDING_CONFIG.model
			);
			expect(defaultModel?.dimension).toBe(DEFAULT_EMBEDDING_CONFIG.dimension);
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
