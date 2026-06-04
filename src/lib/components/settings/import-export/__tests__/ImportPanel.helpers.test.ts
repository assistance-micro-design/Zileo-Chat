import { describe, expect, it } from 'vitest';
import type {
	ImportConflict,
	ImportSelection,
	ImportValidation,
	ImportWarning,
	ImportWarningType
} from '$types/import-export';
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
	interpolateWarningTemplate,
	isSensitiveEnvKey,
	resolveWarningAction,
	resolveWarningDetail,
	warningActionKey,
	warningDetailKey
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

	describe('warning i18n resolution', () => {
		function warning(
			warningType: ImportWarningType,
			detail = '',
			action = 'raw action'
		): ImportWarning {
			return { warningType, severity: 'high', entity: 'e', detail, action };
		}

		it('maps every warning type to a dedicated detail + action key', () => {
			const types: ImportWarningType[] = [
				'missing_model',
				'missing_mcp_server',
				'missing_skill',
				'missing_provider',
				'machine_specific',
				'default_applied',
				'api_key_required',
				'builtin_model',
				'mcp_secret_missing',
				'mcp_allowlist_reset'
			];
			for (const type of types) {
				expect(warningDetailKey(type), `detail key for ${type}`).not.toBe('');
				expect(warningActionKey(type), `action key for ${type}`).not.toBe('');
			}
		});

		it('wires the mcp_secret_missing warning (regression: was English-only)', () => {
			expect(warningDetailKey('mcp_secret_missing')).toBe('ie_warn_mcp_secret_missing');
			expect(warningActionKey('mcp_secret_missing')).toBe('ie_warn_mcp_secret_missing_action');
		});

		it('interpolates {name} and {count} from the backend detail, language-independent', () => {
			expect(
				interpolateWarningTemplate("Modele '{name}' introuvable", "model 'gpt' not found")
			).toBe("Modele 'gpt' introuvable");
			expect(
				interpolateWarningTemplate('{count} dossiers', '3 folder path(s) are machine-specific')
			).toBe('3 dossiers');
		});

		it('resolves localized detail/action via the translate function, not the English text', () => {
			const translate = (key: string) => `T:${key}`;
			// A French-locale detail must still resolve by type, not by substring.
			const w = warning('missing_model', "Modele 'gpt-5' introuvable", 'Ajoutez le modele');
			expect(resolveWarningDetail(w, translate)).toBe('T:ie_warn_missing_model');
			expect(resolveWarningAction(w, translate)).toBe('T:ie_warn_missing_model_action');
		});

		it('falls back to raw backend strings for an unknown warning type', () => {
			const w = warning('totally_unknown' as ImportWarningType, 'raw detail', 'raw action');
			const translate = (key: string) => `T:${key}`;
			expect(resolveWarningDetail(w, translate)).toBe('raw detail');
			expect(resolveWarningAction(w, translate)).toBe('raw action');
		});
	});
});
