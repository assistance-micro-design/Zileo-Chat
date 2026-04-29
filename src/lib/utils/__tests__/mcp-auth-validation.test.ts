/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 *
 * Tests for the MCP HTTP auth validation helpers (v1.2).
 */

import { describe, it, expect } from 'vitest';
import {
	validateBearerToken,
	validateApiKeyHeaderName,
	validateApiKeyValue,
	validateBasicAuth,
	validateExtraHeaders,
	validateHeaderName,
	validateHeaderValue,
	MAX_BEARER_TOKEN_LEN,
	MAX_HEADER_NAME_LEN,
	MAX_EXTRA_HEADERS
} from '../mcp-auth-validation';

describe('validateHeaderName', () => {
	it('accepts ASCII alnum, underscore and hyphen', () => {
		expect(validateHeaderName('X-API-Key')).toBe(true);
		expect(validateHeaderName('Authorization')).toBe(true);
		expect(validateHeaderName('X_Tenant_42')).toBe(true);
	});

	it('rejects empty, too long, and non-ASCII names', () => {
		expect(validateHeaderName('')).toBe(false);
		expect(validateHeaderName('a'.repeat(MAX_HEADER_NAME_LEN + 1))).toBe(false);
		expect(validateHeaderName('éclair')).toBe(false);
		expect(validateHeaderName('X Tenant')).toBe(false);
		expect(validateHeaderName('X:Tenant')).toBe(false);
	});
});

describe('validateHeaderValue', () => {
	it('accepts simple ASCII values', () => {
		expect(validateHeaderValue('42')).toBe(true);
		expect(validateHeaderValue('Bearer xxx')).toBe(true);
	});

	it('rejects empty and newlines', () => {
		expect(validateHeaderValue('')).toBe(false);
		expect(validateHeaderValue('with\nnewline')).toBe(false);
		expect(validateHeaderValue('with\rcr')).toBe(false);
	});
});

describe('validateBearerToken', () => {
	it('returns ok for a normal token', () => {
		const r = validateBearerToken('sk-abc1234567890');
		expect(r.valid).toBe(true);
		expect(r.error).toBeUndefined();
	});

	it('trims surrounding whitespace before validating', () => {
		expect(validateBearerToken('  sk-token  ').valid).toBe(true);
	});

	it('rejects empty token (after trim)', () => {
		const r = validateBearerToken('   ');
		expect(r.valid).toBe(false);
		expect(r.error).toBe('mcp_auth_invalid_bearer');
	});

	it('rejects too-long token', () => {
		const r = validateBearerToken('a'.repeat(MAX_BEARER_TOKEN_LEN + 1));
		expect(r.valid).toBe(false);
	});

	it('rejects newlines in token', () => {
		expect(validateBearerToken('sk-abc\nbad').valid).toBe(false);
		expect(validateBearerToken('sk-abc\rbad').valid).toBe(false);
	});
});

describe('validateApiKeyHeaderName', () => {
	it('accepts undefined (defaults to X-API-Key)', () => {
		expect(validateApiKeyHeaderName(undefined).valid).toBe(true);
		expect(validateApiKeyHeaderName('').valid).toBe(true);
	});

	it('accepts custom alphanumeric header name', () => {
		expect(validateApiKeyHeaderName('X-Custom-Key').valid).toBe(true);
	});

	it('rejects names with spaces or non-ASCII', () => {
		expect(validateApiKeyHeaderName('Bad Header').valid).toBe(false);
		expect(validateApiKeyHeaderName('éclair').valid).toBe(false);
	});
});

describe('validateApiKeyValue', () => {
	it('accepts a normal value', () => {
		expect(validateApiKeyValue('abc-123').valid).toBe(true);
	});

	it('rejects empty / newlines', () => {
		expect(validateApiKeyValue('').valid).toBe(false);
		expect(validateApiKeyValue('a\nb').valid).toBe(false);
	});
});

describe('validateBasicAuth', () => {
	it('accepts ascii credentials', () => {
		expect(validateBasicAuth('alice', 'p@ss').valid).toBe(true);
	});

	it('rejects username with colon', () => {
		const r = validateBasicAuth('ali:ce', 'p@ss');
		expect(r.valid).toBe(false);
		expect(r.error).toBe('mcp_auth_invalid_basic');
	});

	it('rejects empty credentials', () => {
		expect(validateBasicAuth('', 'p').valid).toBe(false);
		expect(validateBasicAuth('alice', '').valid).toBe(false);
	});

	it('rejects newlines', () => {
		expect(validateBasicAuth('alice\n', 'p').valid).toBe(false);
		expect(validateBasicAuth('alice', 'p\rass').valid).toBe(false);
	});
});

describe('validateExtraHeaders', () => {
	it('accepts a small map', () => {
		const r = validateExtraHeaders({ 'X-Tenant': '42', 'X-Trace': 'abc' }, false);
		expect(r.valid).toBe(true);
	});

	it('rejects too many entries', () => {
		const headers: Record<string, string> = {};
		for (let i = 0; i < MAX_EXTRA_HEADERS + 1; i++) {
			headers[`X-Header-${i}`] = 'v';
		}
		expect(validateExtraHeaders(headers, false).valid).toBe(false);
	});

	it('rejects Authorization when auth is configured (case-insensitive)', () => {
		expect(validateExtraHeaders({ Authorization: 'Bearer xxx' }, true).valid).toBe(false);
		expect(validateExtraHeaders({ authorization: 'Bearer xxx' }, true).valid).toBe(false);
	});

	it('allows Authorization when no main auth is set', () => {
		expect(validateExtraHeaders({ Authorization: 'Bearer xxx' }, false).valid).toBe(true);
	});

	it('rejects invalid header name', () => {
		expect(validateExtraHeaders({ 'X Bad': 'v' }, false).valid).toBe(false);
	});

	it('rejects newlines in value', () => {
		expect(validateExtraHeaders({ 'X-Tenant': 'a\nb' }, false).valid).toBe(false);
	});

	it('rejects empty value', () => {
		expect(validateExtraHeaders({ 'X-Tenant': '' }, false).valid).toBe(false);
	});
});
