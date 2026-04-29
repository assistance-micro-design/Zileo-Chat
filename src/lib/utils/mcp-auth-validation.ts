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
 * MCP HTTP authentication validation helpers (v1.2).
 *
 * Pure functions, symmetric with the Rust backend in
 * `src-tauri/src/commands/mcp/validation.rs`. Error messages are i18n
 * KEYS — callers resolve them via `$i18n(key)` for display.
 *
 * @module utils/mcp-auth-validation
 */

/** Result of a validation check. The error is an i18n key, not a message. */
export interface ValidationResult {
	valid: boolean;
	/** i18n key (e.g. `mcp_auth_invalid_bearer`). Only set when `valid === false`. */
	error?: string;
}

/** Maximum length of a Bearer token (after trim). */
export const MAX_BEARER_TOKEN_LEN = 4096;
/** Maximum length of an API-key value. */
export const MAX_API_KEY_VALUE_LEN = 1024;
/** Maximum length of a Basic auth username. */
export const MAX_BASIC_USERNAME_LEN = 256;
/** Maximum length of a Basic auth password. */
export const MAX_BASIC_PASSWORD_LEN = 1024;
/** Maximum length of an HTTP header name (auth or extra). */
export const MAX_HEADER_NAME_LEN = 64;
/** Maximum length of an HTTP header value (extra headers). */
export const MAX_HEADER_VALUE_LEN = 1024;
/** Maximum number of extra headers per server. */
export const MAX_EXTRA_HEADERS = 20;

const VALID_HEADER_NAME_RE = /^[A-Za-z0-9_-]+$/;
const ok = (): ValidationResult => ({ valid: true });
const err = (error: string): ValidationResult => ({ valid: false, error });

/**
 * Returns true when `name` is a syntactically valid HTTP header name for
 * our purposes (1..=64 chars, only `[A-Za-z0-9_-]`).
 */
export function validateHeaderName(name: string): boolean {
	return (
		typeof name === 'string' &&
		name.length >= 1 &&
		name.length <= MAX_HEADER_NAME_LEN &&
		VALID_HEADER_NAME_RE.test(name)
	);
}

/**
 * Returns true when `value` is a safe HTTP header value (no `\r` / `\n`,
 * 1..=1024 chars).
 */
export function validateHeaderValue(value: string): boolean {
	return (
		typeof value === 'string' &&
		value.length >= 1 &&
		value.length <= MAX_HEADER_VALUE_LEN &&
		!value.includes('\r') &&
		!value.includes('\n')
	);
}

/**
 * Validates a Bearer token. Trims first; rejects empty / too long / newlines.
 *
 * @returns ValidationResult with i18n key on failure.
 */
export function validateBearerToken(token: string): ValidationResult {
	const trimmed = token.trim();
	if (!trimmed) {
		return err('mcp_auth_invalid_bearer');
	}
	if (trimmed.length > MAX_BEARER_TOKEN_LEN) {
		return err('mcp_auth_invalid_bearer');
	}
	if (trimmed.includes('\r') || trimmed.includes('\n')) {
		return err('mcp_auth_invalid_bearer');
	}
	return ok();
}

/**
 * Validates the API-key header NAME. Defaults to `X-API-Key` when caller
 * passes empty / undefined.
 */
export function validateApiKeyHeaderName(name: string | undefined): ValidationResult {
	const trimmed = (name ?? '').trim() || 'X-API-Key';
	if (!validateHeaderName(trimmed)) {
		return err('mcp_auth_invalid_header_name');
	}
	return ok();
}

/**
 * Validates the API-key VALUE (the secret).
 */
export function validateApiKeyValue(value: string): ValidationResult {
	if (!value) {
		return err('mcp_auth_invalid_header_value');
	}
	if (value.length > MAX_API_KEY_VALUE_LEN) {
		return err('mcp_auth_invalid_header_value');
	}
	if (value.includes('\r') || value.includes('\n')) {
		return err('mcp_auth_invalid_header_value');
	}
	return ok();
}

/**
 * Validates Basic auth credentials.
 *
 * Username: 1..=256 chars, no `\r` / `\n`, no `:`.
 * Password: 1..=1024 chars, no `\r` / `\n`.
 */
export function validateBasicAuth(username: string, password: string): ValidationResult {
	if (
		!username ||
		username.length > MAX_BASIC_USERNAME_LEN ||
		username.includes('\r') ||
		username.includes('\n') ||
		username.includes(':')
	) {
		return err('mcp_auth_invalid_basic');
	}
	if (
		!password ||
		password.length > MAX_BASIC_PASSWORD_LEN ||
		password.includes('\r') ||
		password.includes('\n')
	) {
		return err('mcp_auth_invalid_basic');
	}
	return ok();
}

/**
 * Validates the entire `extraHeaders` map. When `authTypeSet` is true, the
 * `Authorization` key is rejected (it would conflict with the main auth).
 */
export function validateExtraHeaders(
	headers: Record<string, string>,
	authTypeSet: boolean
): ValidationResult {
	const entries = Object.entries(headers);
	if (entries.length > MAX_EXTRA_HEADERS) {
		return err('mcp_auth_invalid_header_value');
	}
	for (const [name, value] of entries) {
		if (!validateHeaderName(name)) {
			return err('mcp_auth_invalid_header_name');
		}
		if (authTypeSet && name.toLowerCase() === 'authorization') {
			return err('mcp_auth_extra_headers_authz_warning');
		}
		if (!validateHeaderValue(value)) {
			return err('mcp_auth_invalid_header_value');
		}
	}
	return ok();
}
