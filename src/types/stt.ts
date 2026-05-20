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
 * @fileoverview Speech-to-text (Voxtral) settings and transcription types.
 *
 * Mirrors `src-tauri/src/models/stt.rs`. Update both sides together — Rust
 * `STTSettings` uses `#[serde(rename_all = "camelCase")]`.
 */

/** Persisted STT settings stored in `settings:stt.config`. */
export interface STTSettings {
	enabled: boolean;
	modelId: string;
	contextBias: string[];
	language: string | null;
	updatedAt: string;
}

/** Partial update payload. `language: null` clears the override. */
export interface UpdateSTTSettingsRequest {
	enabled?: boolean;
	modelId?: string;
	contextBias?: string[];
	language?: string | null;
}

/** Result returned from `transcribe_audio`. */
export interface TranscriptionResult {
	text: string;
	language: string | null;
	modelUsed: string;
}

/** Phase enum for the FAB state machine. */
export type STTPhase = 'idle' | 'armed' | 'recording' | 'transcribing' | 'error';

/** Supported language codes shown in the settings UI. `null` = auto-detect. */
export const STT_SUPPORTED_LANGUAGES = [
	'fr',
	'en',
	'es',
	'de',
	'it',
	'pt',
	'nl',
	'hi',
	'ar'
] as const;

export type STTLanguageCode = (typeof STT_SUPPORTED_LANGUAGES)[number];

/** Default Voxtral model (kept in sync with the Rust default). */
export const DEFAULT_VOXTRAL_MODEL_ID = 'voxtral-mini-transcribe-2507';
