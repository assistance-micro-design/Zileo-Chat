// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Vitest test setup file.
 * Mocks Tauri APIs for unit testing.
 */

import { vi } from 'vitest';

// Mock @tauri-apps/api/event
vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn().mockResolvedValue(() => {}),
	emit: vi.fn().mockResolvedValue(undefined)
}));

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn().mockResolvedValue(undefined)
}));
