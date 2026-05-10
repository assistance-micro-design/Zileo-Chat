/**
 * Date grouping labels for temporal classification.
 */
export type DateGroupLabel = 'today' | 'yesterday' | 'last_7_days' | 'older';

/**
 * A group of items sharing the same temporal label.
 */
export interface DateGroup<T> {
	label: DateGroupLabel;
	items: T[];
}

/**
 * Groups items by date proximity relative to a reference date.
 * Items within each group are sorted most recent first.
 * Empty groups are omitted.
 *
 * @param items - Array of items to group
 * @param dateField - Key of the item containing an ISO date string
 * @param now - Reference date (defaults to current time)
 * @returns Array of non-empty date groups ordered: today, yesterday, last_7_days, older
 */
export function groupByDate<T>(items: T[], dateField: keyof T, now?: Date): DateGroup<T>[] {
	if (items.length === 0) return [];

	const ref = now ?? new Date();
	const todayStart = new Date(ref);
	todayStart.setUTCHours(0, 0, 0, 0);

	const yesterdayStart = new Date(todayStart);
	yesterdayStart.setUTCDate(yesterdayStart.getUTCDate() - 1);

	const weekStart = new Date(todayStart);
	weekStart.setUTCDate(weekStart.getUTCDate() - 7);

	const buckets: Record<DateGroupLabel, T[]> = {
		today: [],
		yesterday: [],
		last_7_days: [],
		older: []
	};

	for (const item of items) {
		const raw = item[dateField];
		const date = raw instanceof Date ? raw : new Date(raw as string);

		if (date >= todayStart) {
			buckets.today.push(item);
		} else if (date >= yesterdayStart) {
			buckets.yesterday.push(item);
		} else if (date >= weekStart) {
			buckets.last_7_days.push(item);
		} else {
			buckets.older.push(item);
		}
	}

	// Sort each bucket by date descending (most recent first)
	const toTime = (val: T[keyof T]) =>
		(val instanceof Date ? val : new Date(val as string)).getTime();
	const sortDesc = (a: T, b: T) => toTime(b[dateField]) - toTime(a[dateField]);

	const labels: DateGroupLabel[] = ['today', 'yesterday', 'last_7_days', 'older'];
	const result: DateGroup<T>[] = [];

	for (const label of labels) {
		const group = buckets[label];
		if (group.length > 0) {
			group.sort(sortDesc);
			result.push({ label, items: group });
		}
	}

	return result;
}
