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

import { describe, it, expect } from 'vitest';
import { formatCost, formatCostOrPlaceholder } from '../currency';

describe('formatCost', () => {
	it('returns the default "Free" label when cost is exactly 0', () => {
		expect(formatCost(0)).toBe('Free');
	});

	it('honors a caller-provided free label', () => {
		expect(formatCost(0, 'Gratuit')).toBe('Gratuit');
	});

	it('returns "<$0.0001" for very small positive values', () => {
		expect(formatCost(0.00005)).toBe('<$0.0001');
		expect(formatCost(0.0000001)).toBe('<$0.0001');
	});

	it('formats sub-cent values with 4 decimals', () => {
		expect(formatCost(0.005)).toBe('$0.0050');
		expect(formatCost(0.0099)).toBe('$0.0099');
	});

	it('formats values >= 1c with 2 decimals', () => {
		expect(formatCost(0.01)).toBe('$0.01');
		expect(formatCost(1.234)).toBe('$1.23');
		expect(formatCost(12.5)).toBe('$12.50');
	});

	it('treats NaN/Infinity as the free label rather than printing "$NaN"', () => {
		expect(formatCost(Number.NaN)).toBe('Free');
		expect(formatCost(Number.POSITIVE_INFINITY)).toBe('Free');
	});
});

describe('formatCostOrPlaceholder', () => {
	it('returns em-dash placeholder for null', () => {
		expect(formatCostOrPlaceholder(null)).toBe('—');
	});

	it('returns em-dash placeholder for undefined', () => {
		expect(formatCostOrPlaceholder(undefined)).toBe('—');
	});

	it('delegates to formatCost when a number is provided', () => {
		expect(formatCostOrPlaceholder(1.5)).toBe('$1.50');
	});
});
