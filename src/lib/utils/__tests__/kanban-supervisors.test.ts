/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 */

import { describe, it, expect } from 'vitest';
import { supervisorRoleState } from '$lib/utils/kanban-supervisors';

describe('supervisorRoleState', () => {
	const live = new Set(['a', 'b']);

	it('returns "unset" when no id is configured', () => {
		expect(supervisorRoleState(undefined, live)).toBe('unset');
		expect(supervisorRoleState(null, live)).toBe('unset');
		expect(supervisorRoleState('', live)).toBe('unset');
		expect(supervisorRoleState('   ', live)).toBe('unset');
	});

	it('returns "ok" when the configured id is a live Kanban agent', () => {
		expect(supervisorRoleState('a', live)).toBe('ok');
		// Surrounding whitespace is trimmed before the lookup.
		expect(supervisorRoleState('  b  ', live)).toBe('ok');
	});

	it('returns "dangling" when the configured id is not a live Kanban agent', () => {
		expect(supervisorRoleState('ghost', live)).toBe('dangling');
		// A previously valid id whose agent was deleted (empty live set).
		expect(supervisorRoleState('a', new Set())).toBe('dangling');
	});
});
