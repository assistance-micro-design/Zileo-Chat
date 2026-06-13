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
 * @fileoverview Safe frontend wrapper around Tauri webview APIs.
 *
 * @module lib/tauri/webview
 */

import { isTauriRuntime } from './environment';

/**
 * Sets the native webview zoom level when Tauri is available.
 *
 * The native zoom scales the whole rendered surface (text, layout and images),
 * unlike a CSS-only rescale. It does not persist across app restarts, so the
 * caller is responsible for storing the factor and re-applying it on startup.
 *
 * Outside Tauri this is intentionally a no-op (tests, SSR), and any backend
 * failure is swallowed so a zoom request never breaks the calling flow.
 *
 * @param factor - Zoom scale factor (1 = 100%)
 */
export async function setTauriWebviewZoom(factor: number): Promise<void> {
	if (!isTauriRuntime()) {
		return;
	}

	const { getCurrentWebview } = await import('@tauri-apps/api/webview');
	await getCurrentWebview().setZoom(factor);
}
