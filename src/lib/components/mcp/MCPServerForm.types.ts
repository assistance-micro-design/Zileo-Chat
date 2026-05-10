import type { MCPAuthType, MCPDeploymentMethod } from '$types/mcp';

export interface KeyValueRow {
	key: string;
	value: string;
}

export interface MCPServerFormData {
	id: string;
	name: string;
	enabled: boolean;
	command: MCPDeploymentMethod;
	args: string;
	env: KeyValueRow[];
	description: string;
	authType: MCPAuthType;
	bearerToken: string;
	apiKeyHeaderName: string;
	apiKeyValue: string;
	basicUser: string;
	basicPass: string;
	extraHeaders: KeyValueRow[];
}

export interface MCPServerFormErrors {
	name?: string;
	args?: string;
	env?: string;
	authBearer?: string;
	authApiKeyHeader?: string;
	authApiKeyValue?: string;
	authBasic?: string;
	extraHeaders?: string;
}
