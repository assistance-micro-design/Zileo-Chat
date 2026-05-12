import { describe, expect, it } from 'vitest';
import type { MemoryType } from '$types/memory';
import {
	buildMemoryTypeOptions,
	formatImportFailureMessage,
	formatScope,
	getExportMetadata,
	getTypeVariant,
	truncate
} from '../MemoryList.helpers';

describe('MemoryList helpers', () => {
	it('truncates text only when it exceeds the maximum length', () => {
		expect(truncate('short', 10)).toBe('short');
		expect(truncate('exact', 5)).toBe('exact');
		expect(truncate('longer than max', 6)).toBe('longer...');
		expect(truncate('abc', 0)).toBe('...');
	});

	it('formats memory scope using the injected general label', () => {
		expect(formatScope(undefined, 'General')).toBe('General');
		expect(formatScope(null, 'General')).toBe('General');
		expect(formatScope('', 'General')).toBe('General');
		expect(formatScope('abcdefghijkl', 'General')).toBe('abcdefghijkl');
		expect(formatScope('abcdefghijklm', 'General')).toBe('abcdefghijkl...');
	});

	it('maps memory types to badge variants with a primary fallback', () => {
		expect(getTypeVariant('user_pref')).toBe('primary');
		expect(getTypeVariant('context')).toBe('success');
		expect(getTypeVariant('knowledge')).toBe('warning');
		expect(getTypeVariant('decision')).toBe('error');
		expect(getTypeVariant('unknown' as MemoryType)).toBe('primary');
	});

	it('builds memory type options in the existing order', () => {
		expect(
			buildMemoryTypeOptions({
				all: 'All',
				userPref: 'User preferences',
				context: 'Context',
				knowledge: 'Knowledge',
				decision: 'Decision'
			})
		).toEqual([
			{ value: '', label: 'All' },
			{ value: 'user_pref', label: 'User preferences' },
			{ value: 'context', label: 'Context' },
			{ value: 'knowledge', label: 'Knowledge' },
			{ value: 'decision', label: 'Decision' }
		]);
	});

	it('returns stable JSON export metadata', () => {
		expect(getExportMetadata('json', new Date('2025-05-09T12:34:56.000Z'))).toEqual({
			exportFormat: 'json',
			extension: 'json',
			mimeType: 'application/json',
			filterName: 'JSON',
			defaultFilename: 'memories-2025-05-09.json'
		});
	});

	it('returns stable CSV export metadata', () => {
		expect(getExportMetadata('csv', new Date('2025-05-09T12:34:56.000Z'))).toEqual({
			exportFormat: 'csv',
			extension: 'csv',
			mimeType: 'text/csv',
			filterName: 'CSV',
			defaultFilename: 'memories-2025-05-09.csv'
		});
	});

	it('formats import failure messages using the first three errors', () => {
		expect(formatImportFailureMessage('{count} failed: {errors}', 4, ['a', 'b', 'c', 'd'])).toBe(
			'4 failed: a, b, c'
		);
	});
});
