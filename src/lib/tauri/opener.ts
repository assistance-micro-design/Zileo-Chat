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
 * @fileoverview Safe frontend wrapper around Tauri opener APIs.
 *
 * @module lib/tauri/opener
 */

import { createTauriUnavailableError, isTauriRuntime } from './environment';

/**
 * Opens an external URL through Tauri when available.
 *
 * @param url - URL to open externally
 */
export async function openExternalUrl(url: string): Promise<void> {
	if (!isTauriRuntime()) {
		throw createTauriUnavailableError('Tauri external URL opener');
	}

	const { openUrl } = await import('@tauri-apps/plugin-opener');
	await openUrl(url);
}
