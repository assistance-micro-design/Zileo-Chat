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
 * @fileoverview Push-to-talk state machine orchestrating audio capture,
 * Voxtral transcription and DOM insertion.
 *
 * Lifecycle:
 *   idle -> armed (capturedField captured) -> recording (mic active) ->
 *   transcribing (HTTP call) -> idle (text inserted)
 * On any failure: -> error -> idle (after the toast is pushed).
 */

import type { STTPhase } from '$types/stt';
import {
	AudioCaptureError,
	blobToBase64,
	cancelRecording,
	startRecording,
	stopRecording,
	type MediaRecorderSession
} from '$lib/utils/audio-capture';
import { captureActiveField, insertTextIntoField, type CapturedField } from '$lib/utils/dom-insert';
import { transcribe } from '$lib/services/sttService';
import { sttSettingsStore } from '$lib/stores/sttSettings';
import { toastStore } from '$lib/stores/toast';
import { getErrorMessage } from '$lib/utils/error';
import { i18n, localeStore } from '$lib/i18n';
import { get } from 'svelte/store';

interface STTState {
	phase: STTPhase;
	captured: CapturedField | null;
	session: MediaRecorderSession | null;
}

const state = $state<STTState>({
	phase: 'idle',
	captured: null,
	session: null
});

function t(key: string): string {
	return get(i18n)(key);
}

function fail(messageKey: string, detail?: string): void {
	toastStore.add({
		type: 'error',
		title: t('stt_toast_error_title'),
		message: detail ? `${t(messageKey)} — ${detail}` : t(messageKey),
		persistent: false,
		duration: 5000
	});
	state.phase = 'error';
	state.captured = null;
	state.session = null;
	// Drop back to idle on the next tick so the FAB can be re-armed.
	setTimeout(() => {
		if (state.phase === 'error') state.phase = 'idle';
	}, 600);
}

function mapCaptureError(err: AudioCaptureError): string {
	switch (err.kind) {
		case 'permission-denied':
			return 'errors.stt.permission_denied';
		case 'no-codec':
			return 'errors.stt.no_codec';
		case 'no-device':
			return 'errors.stt.no_device';
		case 'too-short':
			return 'errors.stt.recording_too_short';
		case 'too-large':
			return 'errors.stt.recording_too_large';
		case 'empty':
			return 'errors.stt.recording_empty';
		default:
			return 'errors.stt.recorder_failed';
	}
}

export const sttStore = {
	get phase(): STTPhase {
		return state.phase;
	},

	get isActive(): boolean {
		return state.phase === 'recording' || state.phase === 'transcribing';
	},

	get hasArmedField(): boolean {
		return state.captured !== null;
	},

	/** Captures the currently focused field. Call right before recording. */
	attachToActive(): boolean {
		const captured = captureActiveField();
		if (!captured) {
			return false;
		}
		state.captured = captured;
		if (state.phase === 'idle') {
			state.phase = 'armed';
		}
		return true;
	},

	/** Releases any captured field without recording. */
	detach(): void {
		if (state.phase === 'armed') {
			state.phase = 'idle';
		}
		state.captured = null;
	},

	/** Begins capture. Requires {@link attachToActive} beforehand. */
	async startRecording(): Promise<void> {
		if (state.phase === 'recording' || state.phase === 'transcribing') return;
		if (!state.captured) {
			fail('errors.stt.no_focused_field');
			return;
		}
		try {
			state.session = await startRecording();
			state.phase = 'recording';
		} catch (err) {
			if (err instanceof AudioCaptureError) {
				fail(mapCaptureError(err));
			} else {
				fail('errors.stt.recorder_failed', getErrorMessage(err));
			}
		}
	},

	/** Stops capture, sends the blob to Voxtral, inserts the text. */
	async stopAndTranscribe(): Promise<void> {
		if (state.phase !== 'recording' || !state.session || !state.captured) {
			return;
		}
		const session = state.session;
		const captured = state.captured;
		state.session = null;
		state.phase = 'transcribing';

		const settingsState = sttSettingsStore.getState();
		const settings = settingsState.settings;
		if (!settings || !settings.enabled) {
			cancelRecording(session);
			fail('errors.stt.disabled');
			return;
		}

		let audio;
		try {
			audio = await stopRecording(session);
		} catch (err) {
			if (err instanceof AudioCaptureError) {
				fail(mapCaptureError(err));
			} else {
				fail('errors.stt.recorder_failed', getErrorMessage(err));
			}
			return;
		}

		try {
			const audioBase64 = await blobToBase64(audio.blob);
			// Fallback hint: when the user has not pinned a language, send the
			// app's current UI locale. Voxtral handles `null`/missing fine, but
			// passing a hint improves accuracy and latency on short clips.
			const languageHint = settings.language ?? get(localeStore);
			const result = await transcribe({
				audioBase64,
				mimeType: audio.mimeType,
				contextBias: settings.contextBias,
				languageOverride: languageHint,
				modelId: settings.modelId
			});
			insertTextIntoField(captured.el, captured.selectionStart, captured.selectionEnd, result.text);
			state.phase = 'idle';
			state.captured = null;
		} catch (err) {
			fail('errors.stt.transcription_failed', getErrorMessage(err));
		}
	},

	/** Aborts the recording session without transcribing. */
	cancel(): void {
		if (state.session) {
			cancelRecording(state.session);
			state.session = null;
		}
		state.phase = 'idle';
		state.captured = null;
	}
};
