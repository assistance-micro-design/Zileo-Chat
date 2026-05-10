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
 * @fileoverview Safe frontend wrapper around Tauri window APIs.
 *
 * @module lib/tauri/window
 */

import { isTauriRuntime } from './environment';

export type TauriWindowTheme = 'light' | 'dark' | null;

/**
 * Applies a native Tauri window theme when Tauri is available.
 *
 * Outside Tauri this is intentionally a no-op because the DOM theme is handled
 * separately by the frontend theme store.
 *
 * @param value - Native window theme, or null to follow the OS theme
 */
export async function setTauriWindowTheme(value: TauriWindowTheme): Promise<void> {
	if (!isTauriRuntime()) {
		return;
	}

	const { getCurrentWindow } = await import('@tauri-apps/api/window');
	await getCurrentWindow().setTheme(value);
}
