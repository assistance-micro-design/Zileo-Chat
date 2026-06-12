/**
 * Dirty-state helpers for long settings forms.
 *
 * A form is "dirty" when its current values no longer match the values it
 * was seeded with. Both sides are reduced to a stable serialized snapshot so
 * the comparison is insensitive to object key order, while array order stays
 * significant (lists like tool selections are ordered data).
 */

/**
 * Serializes a value into a stable string: object keys are sorted
 * recursively, arrays keep their order. `undefined` members are dropped,
 * matching JSON semantics.
 *
 * @param value - Any JSON-serializable value
 * @returns Deterministic string representation of the value
 */
export function stableSnapshot(value: unknown): string {
	return JSON.stringify(sortKeysDeep(value));
}

/**
 * Recursively rebuilds plain objects with their keys sorted so that two
 * structurally equal values serialize identically. Arrays are mapped in
 * place (order preserved); primitives pass through.
 */
function sortKeysDeep(value: unknown): unknown {
	if (Array.isArray(value)) {
		return value.map(sortKeysDeep);
	}
	if (value !== null && typeof value === 'object') {
		const source = value as Record<string, unknown>;
		const sorted: Record<string, unknown> = {};
		for (const key of Object.keys(source).sort()) {
			sorted[key] = sortKeysDeep(source[key]);
		}
		return sorted;
	}
	return value;
}

/**
 * Returns true when the current form values diverge from the initial ones.
 *
 * @param initial - Values the form was seeded with
 * @param current - Values the form holds now
 * @returns Whether the form has unsaved changes
 */
export function isDirty(initial: unknown, current: unknown): boolean {
	return stableSnapshot(initial) !== stableSnapshot(current);
}
