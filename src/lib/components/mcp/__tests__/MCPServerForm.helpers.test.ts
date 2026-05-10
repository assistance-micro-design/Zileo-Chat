import { describe, expect, it } from 'vitest';
import {
	buildKeyValueMap,
	buildMCPAuthMetadata,
	buildMCPAuthSecret,
	getLegacyApiKeyValue,
	getLegacyHeaderEntries,
	isBasicAuthOverPlainHttp,
	parseArgs
} from '../MCPServerForm.helpers';

describe('MCPServerForm helpers', () => {
	it('parses args by trimming and dropping empty lines while preserving order', () => {
		expect(parseArgs('  first  \n\nsecond\n  third  ')).toEqual(['first', 'second', 'third']);
	});

	it('builds key/value maps by trimming keys only and keeping the last duplicate', () => {
		expect(
			buildKeyValueMap([
				{ key: ' TOKEN ', value: '  keep spaces  ' },
				{ key: '', value: 'ignored' },
				{ key: 'TOKEN', value: 'last wins' }
			])
		).toEqual({ TOKEN: 'last wins' });
	});

	it('builds auth metadata with the current defaults and trimming rules', () => {
		expect(
			buildMCPAuthMetadata({
				authType: 'apikey',
				apiKeyHeaderName: '   ',
				apiKeyValue: '',
				basicUser: '',
				basicPass: '',
				bearerToken: ''
			})
		).toEqual({ headerName: 'X-API-Key' });
		expect(
			buildMCPAuthMetadata({
				authType: 'basic',
				apiKeyHeaderName: '',
				apiKeyValue: '',
				basicUser: ' alice ',
				basicPass: '',
				bearerToken: ''
			})
		).toEqual({ username: 'alice' });
		expect(
			buildMCPAuthMetadata({
				authType: 'bearer',
				apiKeyHeaderName: '',
				apiKeyValue: '',
				basicUser: '',
				basicPass: '',
				bearerToken: ''
			})
		).toBeUndefined();
	});

	it('builds auth secrets while preserving blank-secret edit semantics', () => {
		expect(
			buildMCPAuthSecret({
				authType: 'bearer',
				bearerToken: 'token',
				apiKeyHeaderName: '',
				apiKeyValue: '',
				basicUser: '',
				basicPass: ''
			})
		).toEqual({ token: 'token' });
		expect(
			buildMCPAuthSecret({
				authType: 'apikey',
				bearerToken: '',
				apiKeyHeaderName: '',
				apiKeyValue: 'secret',
				basicUser: '',
				basicPass: ''
			})
		).toEqual({ value: 'secret' });
		expect(
			buildMCPAuthSecret({
				authType: 'basic',
				bearerToken: '',
				apiKeyHeaderName: '',
				apiKeyValue: '',
				basicUser: 'alice',
				basicPass: 'pass'
			})
		).toEqual({ password: 'pass' });
		expect(
			buildMCPAuthSecret({
				authType: 'basic',
				bearerToken: '',
				apiKeyHeaderName: '',
				apiKeyValue: '',
				basicUser: 'alice',
				basicPass: ''
			})
		).toBeUndefined();
	});

	it('detects legacy API_KEY only for HTTP servers without configured auth', () => {
		const env = [{ key: 'API_KEY', value: 'legacy' }];
		expect(getLegacyApiKeyValue(true, 'none', env)).toBe('legacy');
		expect(getLegacyApiKeyValue(false, 'none', env)).toBeNull();
		expect(getLegacyApiKeyValue(true, 'bearer', env)).toBeNull();
		expect(getLegacyApiKeyValue(true, 'none', [{ key: 'API_KEY', value: '' }])).toBeNull();
	});

	it('detects legacy HEADER_* entries only for HTTP servers without configured auth', () => {
		const env = [
			{ key: ' HEADER_X-Test ', value: '1' },
			{ key: 'HEADER_Empty', value: '' },
			{ key: 'OTHER', value: '2' }
		];
		expect(getLegacyHeaderEntries(true, 'none', env)).toEqual([
			{ key: ' HEADER_X-Test ', value: '1' }
		]);
		expect(getLegacyHeaderEntries(false, 'none', env)).toEqual([]);
		expect(getLegacyHeaderEntries(true, 'apikey', env)).toEqual([]);
	});

	it('detects Basic auth over plain HTTP using the first non-empty args line', () => {
		expect(isBasicAuthOverPlainHttp('basic', '\n http://example.test ')).toBe(true);
		expect(isBasicAuthOverPlainHttp('basic', 'https://example.test')).toBe(false);
		expect(isBasicAuthOverPlainHttp('bearer', 'http://example.test')).toBe(false);
	});
});
