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
 * @fileoverview Safe frontend wrappers around Tauri dialog APIs.
 *
 * @module lib/tauri/dialog
 */

import { createTauriUnavailableError, isTauriRuntime } from './environment';

type DialogModule = typeof import('@tauri-apps/plugin-dialog');

export type OpenDialogOptions = Parameters<DialogModule['open']>[0];
export type SaveDialogOptions = Parameters<DialogModule['save']>[0];
export type OpenDialogResult = Awaited<ReturnType<DialogModule['open']>>;
export type SaveDialogResult = Awaited<ReturnType<DialogModule['save']>>;

/**
 * Opens a native Tauri open-file dialog.
 */
export async function openDialog(options?: OpenDialogOptions): Promise<OpenDialogResult> {
	if (!isTauriRuntime()) {
		throw createTauriUnavailableError('Tauri open dialog');
	}

	const { open } = await import('@tauri-apps/plugin-dialog');
	return open(options);
}

/**
 * Opens a native Tauri save-file dialog.
 */
export async function saveDialog(options?: SaveDialogOptions): Promise<SaveDialogResult> {
	if (!isTauriRuntime()) {
		throw createTauriUnavailableError('Tauri save dialog');
	}

	const { save } = await import('@tauri-apps/plugin-dialog');
	return save(options);
}
