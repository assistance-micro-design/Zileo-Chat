import type {
	ConflictResolution,
	ImportConflict,
	ImportSelection,
	ImportValidation,
	ImportWarning,
	ImportWarningType,
	MCPAdditions
} from '$types/import-export';
import { SENSITIVE_ENV_PATTERNS } from '$types/import-export';

/**
 * i18n key for a warning's detail text, keyed purely on the warning type.
 *
 * Every {@link ImportWarningType} maps to exactly one key, so the UI never has
 * to inspect the (English) backend `detail` string to localize a warning.
 * Must stay in sync with the Rust `ImportWarningType` enum.
 */
const WARNING_DETAIL_KEYS: Record<ImportWarningType, string> = {
	missing_model: 'ie_warn_missing_model',
	missing_mcp_server: 'ie_warn_missing_mcp',
	missing_skill: 'ie_warn_missing_skill',
	missing_provider: 'ie_warn_missing_provider',
	machine_specific: 'ie_warn_folders',
	default_applied: 'ie_warn_defaults_applied',
	api_key_required: 'ie_warn_api_keys',
	builtin_model: 'ie_warn_builtin_model',
	mcp_secret_missing: 'ie_warn_mcp_secret_missing',
	mcp_allowlist_reset: 'ie_warn_mcp_allowlist_reset'
};

/** i18n key for a warning's recommended action, keyed on the warning type. */
const WARNING_ACTION_KEYS: Record<ImportWarningType, string> = {
	missing_model: 'ie_warn_missing_model_action',
	missing_mcp_server: 'ie_warn_missing_mcp_action',
	missing_skill: 'ie_warn_missing_skill_action',
	missing_provider: 'ie_warn_missing_provider_action',
	machine_specific: 'ie_warn_folders_action',
	default_applied: 'ie_warn_defaults_applied_action',
	api_key_required: 'ie_warn_api_keys_action',
	builtin_model: 'ie_warn_builtin_model_action',
	mcp_secret_missing: 'ie_warn_mcp_secret_missing_action',
	mcp_allowlist_reset: 'ie_warn_mcp_allowlist_reset_action'
};

/** Returns the i18n key for a warning's detail, or '' if the type is unknown. */
export function warningDetailKey(type: ImportWarningType): string {
	return WARNING_DETAIL_KEYS[type] ?? '';
}

/** Returns the i18n key for a warning's action, or '' if the type is unknown. */
export function warningActionKey(type: ImportWarningType): string {
	return WARNING_ACTION_KEYS[type] ?? '';
}

/**
 * Fills `{name}`/`{count}` placeholders in a translated warning template from
 * the backend `detail` string. The quoted entity name and the leading count are
 * language-independent (the backend always quotes the name and prefixes the
 * count), so this stays robust regardless of the UI locale.
 */
export function interpolateWarningTemplate(template: string, detail: string): string {
	let out = template;
	const nameMatch = detail.match(/'([^']+)'/);
	if (nameMatch?.[1]) out = out.replace('{name}', nameMatch[1]);
	const countMatch = detail.match(/^(\d+)/);
	if (countMatch?.[1]) out = out.replace('{count}', countMatch[1]);
	return out;
}

/**
 * Resolves the localized detail text for a warning.
 * Falls back to the raw backend `detail` when the type has no mapped key.
 *
 * @param warning - the structured import warning
 * @param translate - the i18n lookup (e.g. `$i18n`)
 */
export function resolveWarningDetail(
	warning: ImportWarning,
	translate: (key: string) => string
): string {
	const key = warningDetailKey(warning.warningType);
	if (!key) return warning.detail;
	return interpolateWarningTemplate(translate(key), warning.detail);
}

/**
 * Resolves the localized action text for a warning.
 * Falls back to the raw backend `action` when the type has no mapped key.
 */
export function resolveWarningAction(
	warning: ImportWarning,
	translate: (key: string) => string
): string {
	const key = warningActionKey(warning.warningType);
	if (!key) return warning.action;
	return interpolateWarningTemplate(translate(key), warning.detail);
}

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
