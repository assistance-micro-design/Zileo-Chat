import type { MCPAuthMetadata, MCPAuthSecret, MCPAuthType } from '$types/mcp';
import type { KeyValueRow } from './MCPServerForm.types';

export interface MCPServerFormAuthFields {
	authType: MCPAuthType;
	apiKeyHeaderName: string;
	apiKeyValue: string;
	basicUser: string;
	basicPass: string;
	bearerToken: string;
}

export function generateMCPServerId(): string {
	return `mcp-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
}

export function parseArgs(argsText: string): string[] {
	return argsText
		.split('\n')
		.map((arg) => arg.trim())
		.filter((arg) => arg.length > 0);
}

export function buildKeyValueMap(rows: KeyValueRow[]): Record<string, string> {
	const map: Record<string, string> = {};
	for (const row of rows) {
		const key = row.key.trim();
		if (!key) continue;
		map[key] = row.value;
	}
	return map;
}

export function buildMCPAuthMetadata(fields: MCPServerFormAuthFields): MCPAuthMetadata | undefined {
	switch (fields.authType) {
		case 'apikey':
			return { headerName: fields.apiKeyHeaderName.trim() || 'X-API-Key' };
		case 'basic':
			return { username: fields.basicUser.trim() };
		default:
			return undefined;
	}
}

export function buildMCPAuthSecret(fields: MCPServerFormAuthFields): MCPAuthSecret | undefined {
	switch (fields.authType) {
		case 'bearer':
			return fields.bearerToken ? { token: fields.bearerToken } : undefined;
		case 'apikey':
			return fields.apiKeyValue ? { value: fields.apiKeyValue } : undefined;
		case 'basic':
			return fields.basicPass ? { password: fields.basicPass } : undefined;
		default:
			return undefined;
	}
}

export function getLegacyApiKeyValue(
	isHttp: boolean,
	authType: MCPAuthType,
	env: KeyValueRow[]
): string | null {
	if (!isHttp) return null;
	if (authType !== 'none') return null;
	const entry = env.find((e) => e.key.trim() === 'API_KEY');
	return entry && entry.value ? entry.value : null;
}

export function getLegacyHeaderEntries(
	isHttp: boolean,
	authType: MCPAuthType,
	env: KeyValueRow[]
): KeyValueRow[] {
	if (!isHttp) return [];
	if (authType !== 'none') return [];
	return env.filter((e) => e.key.trim().startsWith('HEADER_') && e.value.length > 0);
}

export function isBasicAuthOverPlainHttp(authType: MCPAuthType, argsText: string): boolean {
	if (authType !== 'basic') return false;
	const url = argsText.split('\n').find((line) => line.trim().length > 0);
	return Boolean(url && /^http:\/\//i.test(url.trim()));
}
