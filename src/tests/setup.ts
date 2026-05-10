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

/**
 * Vitest test setup file.
 * Mocks the frontend Tauri adapter boundary for unit testing.
 */

import { vi } from 'vitest';

vi.mock('$lib/tauri', () => ({
	createTauriUnavailableError: (apiName: string) =>
		new Error(`${apiName} is only available in the Tauri runtime`),
	isBrowserRuntime: vi.fn(() => typeof window !== 'undefined'),
	isTauriRuntime: vi.fn(() => false),
	tauriInvoke: vi.fn().mockResolvedValue(undefined),
	tauriListen: vi.fn().mockResolvedValue(() => {}),
	setTauriWindowTheme: vi.fn().mockResolvedValue(undefined),
	openDialog: vi.fn().mockResolvedValue(null),
	saveDialog: vi.fn().mockResolvedValue(null),
	openExternalUrl: vi.fn().mockResolvedValue(undefined)
}));
