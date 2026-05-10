import type {
	ConflictResolution,
	ImportConflict,
	ImportSelection,
	ImportValidation,
	MCPAdditions
} from '$types/import-export';
import { SENSITIVE_ENV_PATTERNS } from '$types/import-export';

export function createEmptyImportSelection(): ImportSelection {
	return {
		agents: [],
		mcpServers: [],
		models: [],
		prompts: [],
		skills: [],
		customProviders: []
	};
}

export function createSelectionFromValidation(validation: ImportValidation): ImportSelection {
	return {
		agents: validation.entities.agents.map((agent) => agent.name),
		mcpServers: validation.entities.mcpServers.map((server) => server.name),
		models: validation.entities.models.map((model) => model.name),
		prompts: validation.entities.prompts.map((prompt) => prompt.name),
		skills: (validation.entities.skills || []).map((skill) => skill.name),
		customProviders: (validation.entities.customProviders || []).map((provider) => provider.name)
	};
}

export function createMcpAdditionsMap(
	missingMcpEnv: Record<string, string[]>
): Record<string, MCPAdditions> {
	const additionsMap: Record<string, MCPAdditions> = {};
	for (const [serverName, missingKeys] of Object.entries(missingMcpEnv)) {
		if (missingKeys.length > 0) {
			additionsMap[serverName] = {
				addEnv: {},
				addArgs: []
			};
		}
	}
	return additionsMap;
}

export function filterConflictsForSelection(
	conflicts: ImportConflict[],
	selection: ImportSelection
): ImportConflict[] {
	return conflicts.filter((conflict) => {
		switch (conflict.entityType) {
			case 'agent':
				return selection.agents.includes(conflict.entityName);
			case 'mcp':
				return selection.mcpServers.includes(conflict.entityName);
			case 'model':
				return selection.models.includes(conflict.entityName);
			case 'prompt':
				return selection.prompts.includes(conflict.entityName);
			case 'skill':
				return selection.skills.includes(conflict.entityName);
			case 'custom_provider':
				return selection.customProviders.includes(conflict.entityName);
			default:
				return false;
		}
	});
}

export function filterMissingMcpEnvForSelection(
	missingMcpEnv: Record<string, string[]>,
	selection: ImportSelection
): Record<string, string[]> {
	const filtered: Record<string, string[]> = {};
	for (const [serverName, keys] of Object.entries(missingMcpEnv)) {
		if (selection.mcpServers.includes(serverName)) {
			filtered[serverName] = keys;
		}
	}
	return filtered;
}

export function getConflictKey(conflict: ImportConflict): string {
	return `${conflict.entityType}:${conflict.entityName}`;
}

export function hasImportSelection(selection: ImportSelection): boolean {
	return (
		selection.agents.length +
			selection.mcpServers.length +
			selection.models.length +
			selection.prompts.length +
			selection.skills.length +
			selection.customProviders.length >
		0
	);
}

export function isSensitiveEnvKey(key: string): boolean {
	const normalized = key.toUpperCase();
	return SENSITIVE_ENV_PATTERNS.some((pattern) => normalized.includes(pattern));
}

export function areRequiredMcpEnvVarsFilled(
	missingEnv: Record<string, string[]>,
	mcpAdditionsMap: Record<string, MCPAdditions>
): boolean {
	return Object.entries(missingEnv).every(([serverName, keys]) => {
		const additions = mcpAdditionsMap[serverName];
		if (!additions) return false;

		const sensitiveKeys = keys.filter(isSensitiveEnvKey);
		return sensitiveKeys.every((key) => additions.addEnv[key]?.trim());
	});
}

export function areConflictsResolved(
	conflicts: ImportConflict[],
	resolutions: Record<string, ConflictResolution>
): boolean {
	return conflicts.every((conflict) => resolutions[getConflictKey(conflict)]);
}
