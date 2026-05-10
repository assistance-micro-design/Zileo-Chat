import { describe, expect, it } from 'vitest';
import type { ImportConflict, ImportSelection, ImportValidation } from '$types/import-export';
import {
	areConflictsResolved,
	areRequiredMcpEnvVarsFilled,
	createEmptyImportSelection,
	createMcpAdditionsMap,
	createSelectionFromValidation,
	filterConflictsForSelection,
	filterMissingMcpEnvForSelection,
	getConflictKey,
	hasImportSelection,
	isSensitiveEnvKey
} from '../ImportPanel.helpers';

function selection(overrides: Partial<ImportSelection> = {}): ImportSelection {
	return { ...createEmptyImportSelection(), ...overrides };
}

function validation(): ImportValidation {
	return {
		valid: true,
		schemaVersion: '1.2',
		errors: [],
		warnings: [],
		conflicts: [],
		missingMcpEnv: {},
		entities: {
			agents: [{ name: 'agent-a' } as ImportValidation['entities']['agents'][number]],
			mcpServers: [{ name: 'mcp-a' } as ImportValidation['entities']['mcpServers'][number]],
			models: [{ name: 'model-a' } as ImportValidation['entities']['models'][number]],
			prompts: [{ name: 'prompt-a' } as ImportValidation['entities']['prompts'][number]],
			skills: [{ name: 'skill-a' } as ImportValidation['entities']['skills'][number]],
			customProviders: [
				{ name: 'provider-a' } as ImportValidation['entities']['customProviders'][number]
			]
		}
	};
}

describe('ImportPanel helpers', () => {
	it('creates an empty import selection with all entity categories', () => {
		expect(createEmptyImportSelection()).toEqual({
			agents: [],
			mcpServers: [],
			models: [],
			prompts: [],
			skills: [],
			customProviders: []
		});
	});

	it('creates a full selection from validation entities', () => {
		expect(createSelectionFromValidation(validation())).toEqual({
			agents: ['agent-a'],
			mcpServers: ['mcp-a'],
			models: ['model-a'],
			prompts: ['prompt-a'],
			skills: ['skill-a'],
			customProviders: ['provider-a']
		});
	});

	it('creates MCP additions entries only for servers with missing keys', () => {
		expect(createMcpAdditionsMap({ mcpA: ['API_KEY'], mcpB: [] })).toEqual({
			mcpA: { addEnv: {}, addArgs: [] }
		});
	});

	it('filters conflicts using the selected entity names and category mapping', () => {
		const conflicts: ImportConflict[] = [
			{ entityType: 'agent', entityName: 'agent-a', existingId: '1' },
			{ entityType: 'mcp', entityName: 'mcp-a', existingId: '2' },
			{ entityType: 'model', entityName: 'model-b', existingId: '3' },
			{ entityType: 'custom_provider', entityName: 'provider-a', existingId: '4' }
		];

		expect(
			filterConflictsForSelection(
				conflicts,
				selection({
					agents: ['agent-a'],
					mcpServers: ['mcp-a'],
					customProviders: ['provider-a']
				})
			)
		).toEqual([conflicts[0], conflicts[1], conflicts[3]]);
	});

	it('filters missing MCP env by selected server names', () => {
		expect(
			filterMissingMcpEnvForSelection(
				{ mcpA: ['API_KEY'], mcpB: ['TOKEN'] },
				selection({
					mcpServers: ['mcpB']
				})
			)
		).toEqual({ mcpB: ['TOKEN'] });
	});

	it('generates stable conflict keys and checks conflict resolutions', () => {
		const conflicts: ImportConflict[] = [
			{ entityType: 'agent', entityName: 'same-name', existingId: '1' },
			{ entityType: 'model', entityName: 'same-name', existingId: '2' }
		];

		expect(getConflictKey(conflicts[0]!)).toBe('agent:same-name');
		expect(getConflictKey(conflicts[1]!)).toBe('model:same-name');
		expect(areConflictsResolved(conflicts, { 'agent:same-name': 'skip' })).toBe(false);
		expect(
			areConflictsResolved(conflicts, {
				'agent:same-name': 'skip',
				'model:same-name': 'overwrite'
			})
		).toBe(true);
	});

	it('detects whether any import entity is selected', () => {
		expect(hasImportSelection(createEmptyImportSelection())).toBe(false);
		expect(hasImportSelection(selection({ skills: ['skill-a'] }))).toBe(true);
		expect(hasImportSelection(selection({ customProviders: ['provider-a'] }))).toBe(true);
	});

	it('requires filled values only for sensitive missing MCP env vars', () => {
		expect(isSensitiveEnvKey('openai_api_key')).toBe(true);
		expect(isSensitiveEnvKey('PRIVATE_KEY')).toBe(true);
		expect(isSensitiveEnvKey('plain_config')).toBe(false);

		expect(
			areRequiredMcpEnvVarsFilled(
				{ mcpA: ['API_KEY', 'plain_config'] },
				{ mcpA: { addEnv: { API_KEY: '   ' }, addArgs: [] } }
			)
		).toBe(false);
		expect(
			areRequiredMcpEnvVarsFilled(
				{ mcpA: ['API_KEY', 'plain_config'] },
				{ mcpA: { addEnv: { API_KEY: 'secret' }, addArgs: [] } }
			)
		).toBe(true);
	});
});
