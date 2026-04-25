/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 *
 * Tests for isUuid utility.
 */

import { describe, it, expect } from 'vitest';
import { isUuid } from '../uuid';

describe('isUuid', () => {
	it('returns true for a canonical UUID v4', () => {
		expect(isUuid('123e4567-e89b-12d3-a456-426614174000')).toBe(true);
	});

	it('returns true for an upper-case UUID', () => {
		expect(isUuid('123E4567-E89B-12D3-A456-426614174000')).toBe(true);
	});

	it('returns false for a plain display name', () => {
		expect(isUuid('My Agent Name')).toBe(false);
	});

	it('returns false for a slugged name', () => {
		expect(isUuid('abc-123')).toBe(false);
	});

	it('returns false for an empty string', () => {
		expect(isUuid('')).toBe(false);
	});

	it('returns false for a UUID with extra whitespace', () => {
		expect(isUuid(' 123e4567-e89b-12d3-a456-426614174000 ')).toBe(false);
	});

	it('returns false for a UUID missing a segment', () => {
		expect(isUuid('123e4567-e89b-12d3-a456')).toBe(false);
	});
});
