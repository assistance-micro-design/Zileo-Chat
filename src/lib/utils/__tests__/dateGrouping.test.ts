import { describe, it, expect } from 'vitest';
import { groupByDate } from '../dateGrouping';
import type { DateGroupLabel } from '../dateGrouping';

interface TestItem {
	id: string;
	updated_at: string;
}

describe('groupByDate', () => {
	const now = new Date('2026-02-21T14:00:00Z');

	it('should return empty array for empty input', () => {
		expect(groupByDate<TestItem>([], 'updated_at', now)).toEqual([]);
	});

	it('should group today items', () => {
		const items: TestItem[] = [{ id: '1', updated_at: '2026-02-21T10:00:00Z' }];
		const groups = groupByDate(items, 'updated_at', now);
		expect(groups).toHaveLength(1);
		expect(groups[0].label).toBe('today' satisfies DateGroupLabel);
		expect(groups[0].items).toHaveLength(1);
	});

	it('should group yesterday items', () => {
		const items: TestItem[] = [{ id: '1', updated_at: '2026-02-20T10:00:00Z' }];
		const groups = groupByDate(items, 'updated_at', now);
		expect(groups).toHaveLength(1);
		expect(groups[0].label).toBe('yesterday' satisfies DateGroupLabel);
	});

	it('should group last 7 days items', () => {
		const items: TestItem[] = [{ id: '1', updated_at: '2026-02-17T10:00:00Z' }];
		const groups = groupByDate(items, 'updated_at', now);
		expect(groups).toHaveLength(1);
		expect(groups[0].label).toBe('last_7_days' satisfies DateGroupLabel);
	});

	it('should group older items', () => {
		const items: TestItem[] = [{ id: '1', updated_at: '2026-01-01T10:00:00Z' }];
		const groups = groupByDate(items, 'updated_at', now);
		expect(groups).toHaveLength(1);
		expect(groups[0].label).toBe('older' satisfies DateGroupLabel);
	});

	it('should handle mixed dates and sort within groups (most recent first)', () => {
		const items: TestItem[] = [
			{ id: '1', updated_at: '2026-02-21T08:00:00Z' },
			{ id: '2', updated_at: '2026-02-21T12:00:00Z' },
			{ id: '3', updated_at: '2026-02-20T10:00:00Z' },
			{ id: '4', updated_at: '2026-01-15T10:00:00Z' }
		];
		const groups = groupByDate(items, 'updated_at', now);
		expect(groups).toHaveLength(3); // today, yesterday, older
		expect(groups[0].label).toBe('today');
		expect(groups[0].items).toHaveLength(2);
		expect(groups[0].items[0].id).toBe('2'); // most recent first
		expect(groups[0].items[1].id).toBe('1');
		expect(groups[1].label).toBe('yesterday');
		expect(groups[2].label).toBe('older');
	});

	it('should skip empty groups', () => {
		const items: TestItem[] = [
			{ id: '1', updated_at: '2026-02-21T08:00:00Z' },
			{ id: '2', updated_at: '2026-01-01T10:00:00Z' }
		];
		const groups = groupByDate(items, 'updated_at', now);
		expect(groups).toHaveLength(2); // today and older, no yesterday/last_7_days
		expect(groups[0].label).toBe('today');
		expect(groups[1].label).toBe('older');
	});

	it('should handle items at midnight boundary', () => {
		// Feb 21 at 00:00:00 UTC should still be "today" relative to Feb 21 14:00 UTC
		const items: TestItem[] = [{ id: '1', updated_at: '2026-02-21T00:00:00Z' }];
		const groups = groupByDate(items, 'updated_at', now);
		expect(groups[0].label).toBe('today');
	});

	it('should handle items exactly 7 days ago', () => {
		// Feb 14 is exactly 7 days before Feb 21 - should be in last_7_days
		const items: TestItem[] = [{ id: '1', updated_at: '2026-02-14T14:00:00Z' }];
		const groups = groupByDate(items, 'updated_at', now);
		expect(groups[0].label).toBe('last_7_days');
	});

	it('should put items 8+ days ago in older', () => {
		const items: TestItem[] = [{ id: '1', updated_at: '2026-02-13T14:00:00Z' }];
		const groups = groupByDate(items, 'updated_at', now);
		expect(groups[0].label).toBe('older');
	});
});
