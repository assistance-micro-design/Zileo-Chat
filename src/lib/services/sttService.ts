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
 * @fileoverview Thin service layer in front of the `transcribe_audio` IPC call.
 */

import { tauriInvoke as invoke } from '$lib/tauri';
import type { TranscriptionResult } from '$types/stt';

export interface TranscribeParams {
	audioBase64: string;
	mimeType: string;
	contextBias: string[];
	languageOverride: string | null;
	modelId: string;
}

/**
 * Sends a recorded blob to Mistral Voxtral via the Rust command and
 * returns the transcription. Errors propagate as plain `Error` for the
 * caller to translate into a toast.
 */
export async function transcribe(params: TranscribeParams): Promise<TranscriptionResult> {
	return invoke<TranscriptionResult>('transcribe_audio', {
		audioBase64: params.audioBase64,
		mimeType: params.mimeType,
		contextBias: params.contextBias,
		languageOverride: params.languageOverride,
		modelId: params.modelId
	});
}
