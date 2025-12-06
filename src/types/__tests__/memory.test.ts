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
 * @fileoverview Tests for memory types.
 *
 * Tests verify:
 * - Memory type structure and compatibility
 * - Type guards and validation patterns
 * - Backend type synchronization
 */

import { describe, it, expect } from 'vitest';
import type {
	MemoryType,
	Memory,
	CreateMemoryParams,
	SearchMemoryParams,
	MemorySearchResult
} from '../memory';

describe('Memory Types', () => {
	// =========================================================================
	// MemoryType validation
	// =========================================================================

	describe('MemoryType', () => {
		it('should accept valid memory types', () => {
			const validTypes: MemoryType[] = ['user_pref', 'context', 'knowledge', 'decision'];

			expect(validTypes).toHaveLength(4);
			expect(validTypes).toContain('user_pref');
			expect(validTypes).toContain('context');
			expect(validTypes).toContain('knowledge');
			expect(validTypes).toContain('decision');
		});

		it('should categorize memory types correctly', () => {
			// User preferences - user-specific settings
			const userPref: MemoryType = 'user_pref';
			expect(userPref).toBe('user_pref');

			// Context - conversation-specific information
			const context: MemoryType = 'context';
			expect(context).toBe('context');

			// Knowledge - factual information
			const knowledge: MemoryType = 'knowledge';
			expect(knowledge).toBe('knowledge');

			// Decision - recorded choices and rationale
			const decision: MemoryType = 'decision';
			expect(decision).toBe('decision');
		});
	});

	// =========================================================================
	// Memory structure tests
	// =========================================================================

	describe('Memory', () => {
		it('should create valid Memory object', () => {
			const memory: Memory = {
				id: 'mem_abc123',
				type: 'knowledge',
				content: 'SurrealDB supports HNSW vector indexing',
				metadata: { priority: 0.8, tags: ['database', 'vectors'] },
				created_at: '2025-11-26T10:30:00Z'
			};

			expect(memory.id).toBe('mem_abc123');
			expect(memory.type).toBe('knowledge');
			expect(memory.content).toContain('SurrealDB');
			expect(memory.metadata).toBeDefined();
			expect(memory.created_at).toBeDefined();
		});

		it('should handle all memory types', () => {
			const types: MemoryType[] = ['user_pref', 'context', 'knowledge', 'decision'];

			for (const type of types) {
				const memory: Memory = {
					id: `mem_${type}_001`,
					type,
					content: `Content for ${type}`,
					metadata: {},
					created_at: new Date().toISOString()
				};

				expect(memory.type).toBe(type);
				expect(memory.id).toContain(type);
			}
		});

		it('should handle metadata with various fields', () => {
			const memory: Memory = {
				id: 'mem_001',
				type: 'decision',
				content: 'Chose Tauri over Electron for desktop app',
				metadata: {
					agent_source: 'architecture_agent',
					priority: 0.95,
					tags: ['architecture', 'desktop', 'framework'],
					workflow_id: 'wf_tech_decisions',
					related_ids: ['mem_002', 'mem_003'],
					custom_field: 'any value'
				},
				created_at: '2025-11-26T10:30:00Z'
			};

			expect(memory.metadata.agent_source).toBe('architecture_agent');
			expect(memory.metadata.priority).toBe(0.95);
			expect((memory.metadata.tags as string[]).length).toBe(3);
			expect(memory.metadata.workflow_id).toBe('wf_tech_decisions');
		});
	});

	// =========================================================================
	// CreateMemoryParams tests
	// =========================================================================

	describe('CreateMemoryParams', () => {
		it('should create minimal params with required fields', () => {
			const params: CreateMemoryParams = {
				memoryType: 'knowledge',
				content: 'Rust is a systems programming language'
			};

			expect(params.memoryType).toBe('knowledge');
			expect(params.content).toBeDefined();
			expect(params.metadata).toBeUndefined();
		});

		it('should create params with optional metadata', () => {
			const params: CreateMemoryParams = {
				memoryType: 'user_pref',
				content: 'User prefers dark mode',
				metadata: { priority: 0.5, tags: ['ui', 'theme'] }
			};

			expect(params.metadata).toBeDefined();
			expect(params.metadata?.priority).toBe(0.5);
		});

		it('should create params with workflowId', () => {
			const params: CreateMemoryParams = {
				memoryType: 'context',
				content: 'Workflow-specific context',
				workflowId: 'wf_001'
			};

			expect(params.workflowId).toBe('wf_001');
		});

		it('should accept all memory types', () => {
			const types: MemoryType[] = ['user_pref', 'context', 'knowledge', 'decision'];

			for (const type of types) {
				const params: CreateMemoryParams = {
					memoryType: type,
					content: `Content for ${type}`
				};

				expect(params.memoryType).toBe(type);
			}
		});
	});

	// =========================================================================
	// SearchMemoryParams tests
	// =========================================================================

	describe('SearchMemoryParams', () => {
		it('should create minimal search params', () => {
			const params: SearchMemoryParams = {
				query: 'vector database'
			};

			expect(params.query).toBe('vector database');
			expect(params.limit).toBeUndefined();
			expect(params.typeFilter).toBeUndefined();
		});

		it('should create params with all options', () => {
			const params: SearchMemoryParams = {
				query: 'embedding generation',
				limit: 5,
				typeFilter: 'knowledge',
				workflowId: 'wf_001',
				threshold: 0.8
			};

			expect(params.query).toBe('embedding generation');
			expect(params.limit).toBe(5);
			expect(params.typeFilter).toBe('knowledge');
			expect(params.workflowId).toBe('wf_001');
			expect(params.threshold).toBe(0.8);
		});

		it('should accept all memory types as filter', () => {
			const types: MemoryType[] = ['user_pref', 'context', 'knowledge', 'decision'];

			for (const type of types) {
				const params: SearchMemoryParams = {
					query: 'test',
					typeFilter: type
				};

				expect(params.typeFilter).toBe(type);
			}
		});
	});

	// =========================================================================
	// MemorySearchResult tests
	// =========================================================================

	describe('MemorySearchResult', () => {
		it('should create valid search result', () => {
			const result: MemorySearchResult = {
				memory: {
					id: 'mem_001',
					type: 'knowledge',
					content: 'SurrealDB is a multi-model database',
					metadata: {},
					created_at: '2025-11-26T10:30:00Z'
				},
				score: 0.92
			};

			expect(result.memory).toBeDefined();
			expect(result.score).toBe(0.92);
			expect(result.score).toBeGreaterThan(0);
			expect(result.score).toBeLessThanOrEqual(1);
		});

		it('should handle score range 0-1', () => {
			const highScore: MemorySearchResult = {
				memory: {
					id: 'mem_001',
					type: 'knowledge',
					content: 'Exact match',
					metadata: {},
					created_at: '2025-11-26T10:30:00Z'
				},
				score: 1.0
			};

			const lowScore: MemorySearchResult = {
				memory: {
					id: 'mem_002',
					type: 'knowledge',
					content: 'Barely relevant',
					metadata: {},
					created_at: '2025-11-26T10:30:00Z'
				},
				score: 0.7
			};

			expect(highScore.score).toBe(1.0);
			expect(lowScore.score).toBe(0.7);
			expect(highScore.score).toBeGreaterThan(lowScore.score);
		});

		it('should maintain memory structure in results', () => {
			const result: MemorySearchResult = {
				memory: {
					id: 'mem_full',
					type: 'decision',
					content: 'Architecture decision with full details',
					metadata: {
						agent_source: 'architect_agent',
						priority: 0.9,
						tags: ['architecture', 'design']
					},
					created_at: '2025-11-26T10:30:00Z'
				},
				score: 0.85
			};

			expect(result.memory.id).toBe('mem_full');
			expect(result.memory.type).toBe('decision');
			expect(result.memory.metadata.agent_source).toBe('architect_agent');
			expect(result.score).toBe(0.85);
		});
	});

	// =========================================================================
	// Integration with embedding types
	// =========================================================================

	describe('Integration Patterns', () => {
		it('should support memory with embedding reference', () => {
			// Embedding field is optional - stored in DB but not always returned
			const memoryWithEmbeddingRef: Memory & { embedding_id?: string } = {
				id: 'mem_001',
				type: 'knowledge',
				content: 'Content with embedding',
				metadata: { has_embedding: true },
				created_at: '2025-11-26T10:30:00Z',
				embedding_id: 'emb_vec_001'
			};

			expect(memoryWithEmbeddingRef.embedding_id).toBeDefined();
			expect(memoryWithEmbeddingRef.metadata.has_embedding).toBe(true);
		});

		it('should support workflow-scoped memory via metadata', () => {
			const workflowMemory: Memory = {
				id: 'mem_wf_001',
				type: 'context',
				content: 'Workflow-specific context',
				metadata: {
					workflow_id: 'wf_code_review',
					scope: 'workflow'
				},
				created_at: '2025-11-26T10:30:00Z'
			};

			expect(workflowMemory.metadata.workflow_id).toBe('wf_code_review');
			expect(workflowMemory.metadata.scope).toBe('workflow');
		});
	});
});
