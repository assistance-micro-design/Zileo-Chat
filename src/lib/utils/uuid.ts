/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 *
 * UUID validation utilities.
 */

const UUID_REGEX = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Returns true if the value matches the canonical 8-4-4-4-12 UUID hex format.
 * Case-insensitive, no whitespace tolerance.
 *
 * @param value - String to test
 * @returns Whether the value is a valid UUID format
 */
export function isUuid(value: string): boolean {
	return UUID_REGEX.test(value);
}
