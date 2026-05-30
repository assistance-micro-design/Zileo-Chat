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
import {
	AudioCaptureError,
	blobToBase64,
	cancelRecording,
	MAX_BLOB_BYTES,
	MIN_RECORDING_MS,
	pickSupportedMime,
	startRecording,
	stopRecording,
	SUPPORTED_AUDIO_MIMES,
	type MediaRecorderSession
} from '../audio-capture';

// jsdom's Blob lacks .arrayBuffer() / .stream() in some environments. The pure
// encoder is browser-compatible; tests use the runtime Blob as provided.

// ---------------------------------------------------------------------------
// pickSupportedMime — pure
// ---------------------------------------------------------------------------

describe('pickSupportedMime', () => {
	it('returns null when no MediaRecorder class is available', () => {
		expect(pickSupportedMime(null)).toBeNull();
	});

	it('returns null when nothing in the probe list is supported', () => {
		const recorderClass = { isTypeSupported: () => false };
		expect(pickSupportedMime(recorderClass)).toBeNull();
	});

	it('returns the FIRST probe that the runtime accepts (opus webm preferred)', () => {
		const recorderClass = {
			isTypeSupported: (mime: string) => mime === 'audio/webm;codecs=opus'
		};
		const result = pickSupportedMime(recorderClass);
		expect(result).not.toBeNull();
		expect(result!.full).toBe('audio/webm;codecs=opus');
		expect(result!.base).toBe('audio/webm');
	});

	it('falls back to mp4 when only mp4 is supported (Safari path)', () => {
		const recorderClass = {
			isTypeSupported: (mime: string) => mime === 'audio/mp4'
		};
		const result = pickSupportedMime(recorderClass);
		expect(result!.base).toBe('audio/mp4');
	});

	it('every probe maps to a whitelisted base MIME', () => {
		const recorderClass = { isTypeSupported: () => true };
		const result = pickSupportedMime(recorderClass);
		expect(SUPPORTED_AUDIO_MIMES).toContain(result!.base);
	});
});

// ---------------------------------------------------------------------------
// AudioCaptureError — type discriminator
// ---------------------------------------------------------------------------

describe('AudioCaptureError', () => {
	it('exposes the kind and is an Error subclass', () => {
		const e = new AudioCaptureError('permission-denied', 'nope');
		expect(e).toBeInstanceOf(Error);
		expect(e).toBeInstanceOf(AudioCaptureError);
		expect(e.kind).toBe('permission-denied');
		expect(e.message).toBe('nope');
		expect(e.name).toBe('AudioCaptureError');
	});
});

// ---------------------------------------------------------------------------
// blobToBase64 — pure
// ---------------------------------------------------------------------------

describe('blobToBase64', () => {
	it('encodes a small blob without the data: prefix', async () => {
		const blob = new Blob([new Uint8Array([72, 101, 108, 108, 111])], { type: 'audio/webm' });
		const encoded = await blobToBase64(blob);
		expect(encoded).toBe('SGVsbG8='); // "Hello"
		expect(encoded.startsWith('data:')).toBe(false);
	});

	it('round-trips arbitrary binary payloads', async () => {
		const bytes = new Uint8Array(1024);
		for (let i = 0; i < bytes.length; i++) bytes[i] = i % 256;
		const encoded = await blobToBase64(new Blob([bytes]));
		const decoded = Uint8Array.from(atob(encoded), (c) => c.charCodeAt(0));
		expect(decoded).toEqual(bytes);
	});

	it('handles blobs larger than the 0x8000-byte chunk window', async () => {
		// 100 KB triggers >3 chunks in the encoder loop.
		const bytes = new Uint8Array(100 * 1024);
		for (let i = 0; i < bytes.length; i++) bytes[i] = (i * 31) % 256;
		const encoded = await blobToBase64(new Blob([bytes]));
		const decoded = Uint8Array.from(atob(encoded), (c) => c.charCodeAt(0));
		expect(decoded.length).toBe(bytes.length);
		expect(decoded[0]).toBe(bytes[0]);
		expect(decoded[decoded.length - 1]).toBe(bytes[bytes.length - 1]);
	});

	it('returns an empty string on an empty blob', async () => {
		const encoded = await blobToBase64(new Blob([]));
		expect(encoded).toBe('');
	});
});

// ---------------------------------------------------------------------------
// startRecording / stopRecording / cancelRecording — heavy mocks
// ---------------------------------------------------------------------------

interface FakeTrack {
	stop: ReturnType<typeof vi.fn>;
}

interface FakeStream {
	getTracks: () => FakeTrack[];
}

class FakeMediaRecorder {
	static isTypeSupported: (mime: string) => boolean = vi.fn(
		(mime: string) => mime === 'audio/webm;codecs=opus'
	);
	state: 'inactive' | 'recording' | 'paused' = 'inactive';
	listeners: Record<string, ((ev: { data?: Blob }) => void)[]> = {};

	constructor(
		public stream: FakeStream,
		public options: { mimeType: string }
	) {}

	addEventListener(name: string, cb: (ev: { data?: Blob }) => void): void {
		(this.listeners[name] ||= []).push(cb);
	}
	removeEventListener(name: string, cb: (ev: { data?: Blob }) => void): void {
		const arr = this.listeners[name] || [];
		const idx = arr.indexOf(cb);
		if (idx >= 0) arr.splice(idx, 1);
	}
	start(): void {
		this.state = 'recording';
	}
	stop(): void {
		this.state = 'inactive';
		// Drain dataavailable then stop (in real life: spec-ordered).
		queueMicrotask(() => {
			this.emit('dataavailable', { data: new Blob(['audio'], { type: 'audio/webm' }) });
			this.emit('stop', {});
		});
	}
	emit(name: string, ev: { data?: Blob }): void {
		for (const cb of this.listeners[name] || []) cb(ev);
	}
}

function installMediaRecorderMock(): void {
	(globalThis as unknown as { MediaRecorder: unknown }).MediaRecorder = FakeMediaRecorder;
}

function installGetUserMedia(behaviour: () => Promise<FakeStream>): FakeStream {
	const track: FakeTrack = { stop: vi.fn() };
	const stream: FakeStream = { getTracks: () => [track] };
	Object.defineProperty(globalThis.navigator, 'mediaDevices', {
		value: { getUserMedia: behaviour ?? (() => Promise.resolve(stream)) },
		configurable: true
	});
	return stream;
}

describe('startRecording', () => {
	beforeEach(() => {
		installMediaRecorderMock();
	});

	afterEach(() => {
		// Reset MediaRecorder isTypeSupported to default for the next test.
		FakeMediaRecorder.isTypeSupported = vi.fn((mime: string) => mime === 'audio/webm;codecs=opus');
	});

	it('throws no-device when getUserMedia is unavailable', async () => {
		Object.defineProperty(globalThis.navigator, 'mediaDevices', {
			value: undefined,
			configurable: true
		});
		await expect(startRecording()).rejects.toBeInstanceOf(AudioCaptureError);
		await expect(startRecording()).rejects.toMatchObject({ kind: 'no-device' });
	});

	it('throws no-codec when MediaRecorder rejects every probe', async () => {
		FakeMediaRecorder.isTypeSupported = vi.fn(() => false);
		installGetUserMedia(() => Promise.resolve({ getTracks: () => [] }));
		await expect(startRecording()).rejects.toMatchObject({ kind: 'no-codec' });
	});

	it('throws permission-denied when getUserMedia raises NotAllowedError', async () => {
		const err = Object.assign(new Error('blocked'), { name: 'NotAllowedError' });
		Object.defineProperty(globalThis.navigator, 'mediaDevices', {
			value: { getUserMedia: () => Promise.reject(err) },
			configurable: true
		});
		await expect(startRecording()).rejects.toMatchObject({ kind: 'permission-denied' });
	});

	it('throws no-device for other getUserMedia failures', async () => {
		const err = Object.assign(new Error('busy'), { name: 'NotReadableError' });
		Object.defineProperty(globalThis.navigator, 'mediaDevices', {
			value: { getUserMedia: () => Promise.reject(err) },
			configurable: true
		});
		await expect(startRecording()).rejects.toMatchObject({ kind: 'no-device' });
	});

	it('returns a session keyed to the picked MIME and the recorder is running', async () => {
		installGetUserMedia(() => Promise.resolve({ getTracks: () => [{ stop: vi.fn() }] }));
		const session = await startRecording();
		expect(session.mimeType).toBe('audio/webm');
		expect(session.recorderMimeType).toBe('audio/webm;codecs=opus');
		expect((session.recorder as unknown as FakeMediaRecorder).state).toBe('recording');
		expect(session.chunks).toEqual([]);
	});
});

describe('stopRecording', () => {
	beforeEach(() => {
		installMediaRecorderMock();
	});

	async function makeSession(): Promise<{
		session: MediaRecorderSession;
		track: FakeTrack;
	}> {
		const track: FakeTrack = { stop: vi.fn() };
		Object.defineProperty(globalThis.navigator, 'mediaDevices', {
			value: { getUserMedia: () => Promise.resolve({ getTracks: () => [track] }) },
			configurable: true
		});
		const session = await startRecording();
		return { session, track };
	}

	it('returns the assembled blob and releases the microphone tracks', async () => {
		const { session, track } = await makeSession();
		// Backdate so the recording is long enough.
		session.startedAt = Date.now() - (MIN_RECORDING_MS + 100);
		const result = await stopRecording(session);

		expect(result.mimeType).toBe('audio/webm');
		expect(result.blob.size).toBeGreaterThan(0);
		expect(result.durationMs).toBeGreaterThanOrEqual(MIN_RECORDING_MS);
		expect(track.stop).toHaveBeenCalledTimes(1);
	});

	it('throws too-short when the press is shorter than MIN_RECORDING_MS', async () => {
		const { session } = await makeSession();
		session.startedAt = Date.now(); // 0 ms duration
		await expect(stopRecording(session)).rejects.toMatchObject({ kind: 'too-short' });
	});

	it('throws empty when no chunks were emitted', async () => {
		const { session } = await makeSession();
		const recorder = session.recorder as unknown as FakeMediaRecorder;
		// Override recorder.stop so it emits 'stop' WITHOUT a prior dataavailable.
		recorder.stop = () => {
			recorder.state = 'inactive';
			queueMicrotask(() => recorder.emit('stop', {}));
		};
		session.startedAt = Date.now() - (MIN_RECORDING_MS + 100);
		await expect(stopRecording(session)).rejects.toMatchObject({ kind: 'empty' });
	});

	it('throws too-large when the blob exceeds MAX_BLOB_BYTES', async () => {
		const { session } = await makeSession();
		session.startedAt = Date.now() - (MIN_RECORDING_MS + 100);
		// Pre-seed an oversize chunk and stub stop to skip its own data emission.
		session.chunks.push(new Blob([new Uint8Array(MAX_BLOB_BYTES + 1)]));
		const recorder = session.recorder as unknown as FakeMediaRecorder;
		recorder.stop = () => {
			recorder.state = 'inactive';
			queueMicrotask(() => recorder.emit('stop', {}));
		};
		await expect(stopRecording(session)).rejects.toMatchObject({ kind: 'too-large' });
	});

	it('throws empty + releases the stream when the recorder was already inactive', async () => {
		const { session, track } = await makeSession();
		(session.recorder as unknown as FakeMediaRecorder).state = 'inactive';
		await expect(stopRecording(session)).rejects.toMatchObject({ kind: 'empty' });
		expect(track.stop).toHaveBeenCalledTimes(1);
	});
});

describe('cancelRecording', () => {
	beforeEach(() => {
		installMediaRecorderMock();
	});

	it('stops the recorder and releases the microphone', async () => {
		const track: FakeTrack = { stop: vi.fn() };
		Object.defineProperty(globalThis.navigator, 'mediaDevices', {
			value: { getUserMedia: () => Promise.resolve({ getTracks: () => [track] }) },
			configurable: true
		});
		const session = await startRecording();
		cancelRecording(session);
		expect((session.recorder as unknown as FakeMediaRecorder).state).toBe('inactive');
		expect(track.stop).toHaveBeenCalledTimes(1);
	});

	it('still releases the microphone when recorder.stop throws', async () => {
		const track: FakeTrack = { stop: vi.fn() };
		Object.defineProperty(globalThis.navigator, 'mediaDevices', {
			value: { getUserMedia: () => Promise.resolve({ getTracks: () => [track] }) },
			configurable: true
		});
		const session = await startRecording();
		const recorder = session.recorder as unknown as FakeMediaRecorder;
		recorder.stop = () => {
			throw new Error('boom');
		};
		expect(() => cancelRecording(session)).not.toThrow();
		expect(track.stop).toHaveBeenCalledTimes(1);
	});

	it('is a no-op on an already-inactive recorder', async () => {
		const track: FakeTrack = { stop: vi.fn() };
		Object.defineProperty(globalThis.navigator, 'mediaDevices', {
			value: { getUserMedia: () => Promise.resolve({ getTracks: () => [track] }) },
			configurable: true
		});
		const session = await startRecording();
		(session.recorder as unknown as FakeMediaRecorder).state = 'inactive';
		expect(() => cancelRecording(session)).not.toThrow();
		expect(track.stop).toHaveBeenCalledTimes(1);
	});
});
