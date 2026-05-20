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

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { readable } from 'svelte/store';
import type { STTSettings } from '$types/stt';
import type { CapturedField } from '$lib/utils/dom-insert';
import type {
	MediaRecorderSession,
	RecordedAudio,
	SupportedAudioMime
} from '$lib/utils/audio-capture';
import { AudioCaptureError } from '$lib/utils/audio-capture';

// ---------------------------------------------------------------------------
// Mocks — declared BEFORE the store import so the module sees the spies.
// ---------------------------------------------------------------------------

const toastAdd = vi.fn();
vi.mock('$lib/stores/toast', () => ({
	toastStore: { add: toastAdd }
}));

const settingsState = {
	settings: {
		enabled: true,
		modelId: 'voxtral-mini-2509',
		contextBias: [],
		language: null,
		updatedAt: new Date().toISOString()
	} as STTSettings | null
};
vi.mock('$lib/stores/sttSettings', () => ({
	sttSettingsStore: {
		getState: () => ({
			settings: settingsState.settings,
			loading: false,
			saving: false,
			error: null
		})
	}
}));

vi.mock('$lib/i18n', () => ({
	i18n: readable((key: string) => key),
	localeStore: readable('en')
}));

const transcribeMock = vi.fn();
vi.mock('$lib/services/sttService', () => ({
	transcribe: (...args: unknown[]) => transcribeMock(...args)
}));

const startRecordingMock = vi.fn();
const stopRecordingMock = vi.fn();
const cancelRecordingMock = vi.fn();
const blobToBase64Mock = vi.fn();
vi.mock('$lib/utils/audio-capture', async () => {
	const actual = await vi.importActual<typeof import('$lib/utils/audio-capture')>(
		'$lib/utils/audio-capture'
	);
	return {
		...actual,
		startRecording: (...args: unknown[]) => startRecordingMock(...args),
		stopRecording: (...args: unknown[]) => stopRecordingMock(...args),
		cancelRecording: (...args: unknown[]) => cancelRecordingMock(...args),
		blobToBase64: (...args: unknown[]) => blobToBase64Mock(...args)
	};
});

const captureActiveFieldMock = vi.fn();
const insertTextIntoFieldMock = vi.fn();
vi.mock('$lib/utils/dom-insert', async () => {
	const actual =
		await vi.importActual<typeof import('$lib/utils/dom-insert')>('$lib/utils/dom-insert');
	return {
		...actual,
		captureActiveField: (...args: unknown[]) => captureActiveFieldMock(...args),
		insertTextIntoField: (...args: unknown[]) => insertTextIntoFieldMock(...args)
	};
});

// Now we can pull the store. The singleton-state lives at module scope, so
// each test must reset it explicitly via `.cancel()` in `beforeEach`.
const { sttStore } = await import('../sttStore.svelte');

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeField(): CapturedField {
	const el = document.createElement('textarea');
	el.value = 'hello';
	document.body.appendChild(el);
	return { el, selectionStart: 0, selectionEnd: 0 };
}

function fakeSession(): MediaRecorderSession {
	return {
		recorder: {} as unknown as MediaRecorder,
		stream: {} as unknown as MediaStream,
		mimeType: 'audio/webm' as SupportedAudioMime,
		recorderMimeType: 'audio/webm;codecs=opus',
		chunks: [],
		startedAt: Date.now()
	};
}

/** Returns the payload of the first toast that was pushed since the last reset. */
function firstToast(): { type: string; message: string; title: string } {
	const call = toastAdd.mock.calls[0];
	if (!call) throw new Error('expected toastStore.add to have been called');
	return call[0] as { type: string; message: string; title: string };
}

function fakeAudio(): RecordedAudio {
	return {
		blob: new Blob(['audio'], { type: 'audio/webm' }),
		mimeType: 'audio/webm',
		durationMs: 1000
	};
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('sttStore — initial state', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		sttStore.cancel();
		settingsState.settings = {
			enabled: true,
			modelId: 'voxtral-mini-2509',
			contextBias: [],
			language: null,
			updatedAt: new Date().toISOString()
		};
	});

	afterEach(() => {
		sttStore.cancel();
		document.body.innerHTML = '';
	});

	it('starts in idle phase with no captured field', () => {
		expect(sttStore.phase).toBe('idle');
		expect(sttStore.hasArmedField).toBe(false);
		expect(sttStore.isActive).toBe(false);
	});

	it('reports isActive only for recording and transcribing phases', () => {
		// idle and armed are not active; verified via state machine below.
		expect(sttStore.isActive).toBe(false);
	});
});

describe('sttStore.attachToActive', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		sttStore.cancel();
	});
	afterEach(() => sttStore.cancel());

	it('returns false and stays idle when no editable field is focused', () => {
		captureActiveFieldMock.mockReturnValue(null);
		expect(sttStore.attachToActive()).toBe(false);
		expect(sttStore.phase).toBe('idle');
		expect(sttStore.hasArmedField).toBe(false);
	});

	it('captures the field and moves to armed when one is focused', () => {
		captureActiveFieldMock.mockReturnValue(makeField());
		expect(sttStore.attachToActive()).toBe(true);
		expect(sttStore.phase).toBe('armed');
		expect(sttStore.hasArmedField).toBe(true);
	});
});

describe('sttStore.detach', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		sttStore.cancel();
	});
	afterEach(() => sttStore.cancel());

	it('releases the captured field and drops back to idle from armed', () => {
		captureActiveFieldMock.mockReturnValue(makeField());
		sttStore.attachToActive();
		expect(sttStore.phase).toBe('armed');
		sttStore.detach();
		expect(sttStore.phase).toBe('idle');
		expect(sttStore.hasArmedField).toBe(false);
	});
});

describe('sttStore.startRecording', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		sttStore.cancel();
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
		sttStore.cancel();
	});

	it('fails fast with an error toast when no field has been captured', async () => {
		await sttStore.startRecording();
		expect(startRecordingMock).not.toHaveBeenCalled();
		expect(toastAdd).toHaveBeenCalledTimes(1);
		expect(firstToast()).toMatchObject({ type: 'error' });
		expect(sttStore.phase).toBe('error');
	});

	it('transitions to recording when the capture succeeds', async () => {
		captureActiveFieldMock.mockReturnValue(makeField());
		startRecordingMock.mockResolvedValue(fakeSession());
		sttStore.attachToActive();

		await sttStore.startRecording();
		expect(sttStore.phase).toBe('recording');
		expect(sttStore.isActive).toBe(true);
	});

	it('maps an AudioCaptureError to its i18n key', async () => {
		captureActiveFieldMock.mockReturnValue(makeField());
		startRecordingMock.mockRejectedValue(new AudioCaptureError('permission-denied', 'blocked'));
		sttStore.attachToActive();

		await sttStore.startRecording();
		expect(toastAdd).toHaveBeenCalledTimes(1);
		const call = firstToast();
		expect(call.type).toBe('error');
		expect(call.message).toContain('errors.stt.permission_denied');
		expect(sttStore.phase).toBe('error');
	});

	it('falls back to a generic recorder_failed key for non-AudioCaptureError', async () => {
		captureActiveFieldMock.mockReturnValue(makeField());
		startRecordingMock.mockRejectedValue(new Error('weird'));
		sttStore.attachToActive();

		await sttStore.startRecording();
		const call = firstToast();
		expect(call.message).toContain('errors.stt.recorder_failed');
	});

	it('is idempotent while already recording', async () => {
		captureActiveFieldMock.mockReturnValue(makeField());
		startRecordingMock.mockResolvedValue(fakeSession());
		sttStore.attachToActive();
		await sttStore.startRecording();
		startRecordingMock.mockClear();

		await sttStore.startRecording();
		expect(startRecordingMock).not.toHaveBeenCalled();
	});
});

describe('sttStore.stopAndTranscribe', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		sttStore.cancel();
		settingsState.settings = {
			enabled: true,
			modelId: 'voxtral-mini-2509',
			contextBias: [],
			language: null,
			updatedAt: new Date().toISOString()
		};
	});
	afterEach(() => sttStore.cancel());

	async function getIntoRecording() {
		captureActiveFieldMock.mockReturnValue(makeField());
		startRecordingMock.mockResolvedValue(fakeSession());
		sttStore.attachToActive();
		await sttStore.startRecording();
	}

	it('is a no-op when called outside the recording phase', async () => {
		await sttStore.stopAndTranscribe();
		expect(stopRecordingMock).not.toHaveBeenCalled();
	});

	it('runs the happy path: stop -> base64 -> transcribe -> insert -> idle', async () => {
		await getIntoRecording();
		stopRecordingMock.mockResolvedValue(fakeAudio());
		blobToBase64Mock.mockResolvedValue('YWJj');
		transcribeMock.mockResolvedValue({ text: 'bonjour' });

		await sttStore.stopAndTranscribe();

		expect(stopRecordingMock).toHaveBeenCalledTimes(1);
		expect(blobToBase64Mock).toHaveBeenCalledTimes(1);
		expect(transcribeMock).toHaveBeenCalledWith({
			audioBase64: 'YWJj',
			mimeType: 'audio/webm',
			contextBias: [],
			languageOverride: 'en',
			modelId: 'voxtral-mini-2509'
		});
		expect(insertTextIntoFieldMock).toHaveBeenCalledTimes(1);
		expect(insertTextIntoFieldMock.mock.calls[0]?.[3]).toBe('bonjour');
		expect(sttStore.phase).toBe('idle');
		expect(sttStore.hasArmedField).toBe(false);
	});

	it('forwards the pinned language override when set in settings', async () => {
		await getIntoRecording();
		settingsState.settings = {
			enabled: true,
			modelId: 'voxtral-mini-2509',
			contextBias: ['CamelCaseTerm'],
			language: 'fr',
			updatedAt: new Date().toISOString()
		};
		stopRecordingMock.mockResolvedValue(fakeAudio());
		blobToBase64Mock.mockResolvedValue('YWJj');
		transcribeMock.mockResolvedValue({ text: 'salut' });

		await sttStore.stopAndTranscribe();
		expect(transcribeMock.mock.calls[0]?.[0]).toMatchObject({
			languageOverride: 'fr',
			contextBias: ['CamelCaseTerm']
		});
	});

	it('cancels the session and toasts when STT was disabled mid-recording', async () => {
		await getIntoRecording();
		settingsState.settings = { ...settingsState.settings!, enabled: false };

		await sttStore.stopAndTranscribe();
		expect(cancelRecordingMock).toHaveBeenCalledTimes(1);
		expect(transcribeMock).not.toHaveBeenCalled();
		expect(firstToast().message).toContain('errors.stt.disabled');
	});

	it('toasts and recovers when stopRecording throws an AudioCaptureError', async () => {
		await getIntoRecording();
		stopRecordingMock.mockRejectedValue(new AudioCaptureError('too-short', 'fast'));

		await sttStore.stopAndTranscribe();
		expect(transcribeMock).not.toHaveBeenCalled();
		expect(insertTextIntoFieldMock).not.toHaveBeenCalled();
		expect(firstToast().message).toContain('errors.stt.recording_too_short');
	});

	it('toasts a generic recorder_failed when stopRecording throws an unknown error', async () => {
		await getIntoRecording();
		stopRecordingMock.mockRejectedValue(new Error('broken'));

		await sttStore.stopAndTranscribe();
		expect(firstToast().message).toContain('errors.stt.recorder_failed');
	});

	it('toasts transcription_failed when the network call rejects', async () => {
		await getIntoRecording();
		stopRecordingMock.mockResolvedValue(fakeAudio());
		blobToBase64Mock.mockResolvedValue('YWJj');
		transcribeMock.mockRejectedValue(new Error('429 rate limited'));

		await sttStore.stopAndTranscribe();
		expect(insertTextIntoFieldMock).not.toHaveBeenCalled();
		const msg = firstToast().message;
		expect(msg).toContain('errors.stt.transcription_failed');
		expect(msg).toContain('429 rate limited');
	});
});

describe('sttStore.cancel', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		sttStore.cancel();
	});

	it('releases the active session and returns to idle', async () => {
		captureActiveFieldMock.mockReturnValue(makeField());
		startRecordingMock.mockResolvedValue(fakeSession());
		sttStore.attachToActive();
		await sttStore.startRecording();
		expect(sttStore.phase).toBe('recording');

		sttStore.cancel();
		expect(cancelRecordingMock).toHaveBeenCalledTimes(1);
		expect(sttStore.phase).toBe('idle');
		expect(sttStore.hasArmedField).toBe(false);
	});

	it('is safe to call when nothing is in flight', () => {
		expect(() => sttStore.cancel()).not.toThrow();
		expect(cancelRecordingMock).not.toHaveBeenCalled();
	});
});
