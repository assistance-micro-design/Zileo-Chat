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
 * @fileoverview Push-to-talk audio capture wrapper around the platform's
 * MediaRecorder + getUserMedia APIs.
 *
 * The accepted MIME list is mirrored on the backend at
 * `src-tauri/src/llm/stt/mistral_batch.rs::SUPPORTED_AUDIO_MIMES`. Update
 * both sides together when the codec set changes.
 */

/** Whitelist of audio MIMEs accepted by both MediaRecorder and the backend. */
export const SUPPORTED_AUDIO_MIMES = [
	'audio/webm',
	'audio/ogg',
	'audio/mp4',
	'audio/wav',
	'audio/mpeg',
	'audio/x-m4a'
] as const;

export type SupportedAudioMime = (typeof SUPPORTED_AUDIO_MIMES)[number];

/** Candidate codecs probed in priority order. */
const CODEC_PROBES: { full: string; base: SupportedAudioMime }[] = [
	{ full: 'audio/webm;codecs=opus', base: 'audio/webm' },
	{ full: 'audio/webm', base: 'audio/webm' },
	{ full: 'audio/ogg;codecs=opus', base: 'audio/ogg' },
	{ full: 'audio/mp4;codecs=mp4a.40.2', base: 'audio/mp4' },
	{ full: 'audio/mp4', base: 'audio/mp4' }
];

/** Minimum recording duration. Below this we treat the press as a misclick. */
export const MIN_RECORDING_MS = 250;

/** Soft cap matching the backend `MAX_AUDIO_BYTES`. */
export const MAX_BLOB_BYTES = 25 * 1024 * 1024;

/** Categorised error codes emitted by the capture pipeline. */
export type AudioCaptureErrorKind =
	| 'permission-denied'
	| 'no-codec'
	| 'no-device'
	| 'too-short'
	| 'too-large'
	| 'empty'
	| 'recorder-failed';

/** Custom error so callers can branch on the kind without parsing strings. */
export class AudioCaptureError extends Error {
	constructor(
		public readonly kind: AudioCaptureErrorKind,
		message: string
	) {
		super(message);
		this.name = 'AudioCaptureError';
	}
}

/** Active session returned by {@link startRecording}. */
export interface MediaRecorderSession {
	recorder: MediaRecorder;
	stream: MediaStream;
	mimeType: SupportedAudioMime;
	recorderMimeType: string;
	chunks: Blob[];
	startedAt: number;
}

/** Result of a completed session. */
export interface RecordedAudio {
	blob: Blob;
	mimeType: SupportedAudioMime;
	durationMs: number;
}

/**
 * Walks the codec probe list and returns the first MIME the browser claims
 * to support, or `null` when none are available.
 */
export function pickSupportedMime(
	recorderClass: { isTypeSupported(mime: string): boolean } | null = typeof MediaRecorder !==
	'undefined'
		? (MediaRecorder as unknown as { isTypeSupported(mime: string): boolean })
		: null
): { full: string; base: SupportedAudioMime } | null {
	if (!recorderClass) return null;
	for (const probe of CODEC_PROBES) {
		if (recorderClass.isTypeSupported(probe.full)) {
			return probe;
		}
	}
	return null;
}

/**
 * Starts a recording session. Caller MUST eventually call
 * {@link stopRecording} (or {@link cancelRecording}) to release the
 * underlying microphone stream.
 */
export async function startRecording(): Promise<MediaRecorderSession> {
	if (typeof navigator === 'undefined' || !navigator.mediaDevices?.getUserMedia) {
		throw new AudioCaptureError('no-device', 'getUserMedia is unavailable in this runtime');
	}

	const probe = pickSupportedMime();
	if (!probe) {
		throw new AudioCaptureError('no-codec', 'No supported audio codec found for MediaRecorder');
	}

	let stream: MediaStream;
	try {
		stream = await navigator.mediaDevices.getUserMedia({
			audio: {
				sampleRate: 16000,
				channelCount: 1,
				echoCancellation: true,
				noiseSuppression: true
			}
		});
	} catch (e) {
		const err = e as DOMException;
		if (err.name === 'NotAllowedError' || err.name === 'SecurityError') {
			throw new AudioCaptureError('permission-denied', 'Microphone permission was denied');
		}
		throw new AudioCaptureError('no-device', err.message || 'Failed to open microphone');
	}

	const recorder = new MediaRecorder(stream, { mimeType: probe.full });
	const chunks: Blob[] = [];
	recorder.addEventListener('dataavailable', (ev) => {
		if (ev.data && ev.data.size > 0) {
			chunks.push(ev.data);
		}
	});

	recorder.start();

	return {
		recorder,
		stream,
		mimeType: probe.base,
		recorderMimeType: probe.full,
		chunks,
		startedAt: Date.now()
	};
}

/** Releases the microphone tracks of a session. */
function releaseStream(session: MediaRecorderSession): void {
	for (const track of session.stream.getTracks()) {
		track.stop();
	}
}

/**
 * Stops the recorder, releases the microphone, and returns the assembled
 * blob. Throws {@link AudioCaptureError} when the recording is too short,
 * too large or otherwise unusable.
 */
export async function stopRecording(session: MediaRecorderSession): Promise<RecordedAudio> {
	if (session.recorder.state === 'inactive') {
		releaseStream(session);
		throw new AudioCaptureError('empty', 'Recorder was already stopped');
	}

	const stopped = new Promise<void>((resolve) => {
		const onStop = () => {
			session.recorder.removeEventListener('stop', onStop);
			resolve();
		};
		session.recorder.addEventListener('stop', onStop);
	});

	try {
		session.recorder.stop();
		await stopped;
	} finally {
		releaseStream(session);
	}

	const durationMs = Date.now() - session.startedAt;
	const blob = new Blob(session.chunks, { type: session.recorderMimeType });

	if (blob.size === 0) {
		throw new AudioCaptureError('empty', 'No audio captured');
	}
	if (durationMs < MIN_RECORDING_MS) {
		throw new AudioCaptureError('too-short', `Recording shorter than ${MIN_RECORDING_MS}ms`);
	}
	if (blob.size > MAX_BLOB_BYTES) {
		throw new AudioCaptureError('too-large', `Recording exceeds ${MAX_BLOB_BYTES} bytes`);
	}

	return { blob, mimeType: session.mimeType, durationMs };
}

/** Discards an active recording without surfacing its bytes. */
export function cancelRecording(session: MediaRecorderSession): void {
	try {
		if (session.recorder.state !== 'inactive') {
			session.recorder.stop();
		}
	} catch {
		// best-effort — release the stream regardless.
	}
	releaseStream(session);
}

async function readBlobArrayBuffer(blob: Blob): Promise<ArrayBuffer> {
	if (typeof blob.arrayBuffer === 'function') {
		return blob.arrayBuffer();
	}

	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onerror = () => reject(reader.error ?? new Error('Failed to read blob'));
		reader.onload = () => resolve(reader.result as ArrayBuffer);
		reader.readAsArrayBuffer(blob);
	});
}

/**
 * Reads a `Blob` and returns its base64-encoded payload without the
 * `data:<mime>;base64,` prefix. Suitable for forwarding to a Tauri command.
 */
export async function blobToBase64(blob: Blob): Promise<string> {
	const buffer = await readBlobArrayBuffer(blob);
	const bytes = new Uint8Array(buffer);
	const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
	let encoded = '';

	for (let i = 0; i < bytes.length; i += 3) {
		const first = bytes[i] ?? 0;
		const second = bytes[i + 1];
		const third = bytes[i + 2];

		encoded += alphabet[first >> 2];
		encoded += alphabet[((first & 0x03) << 4) | ((second ?? 0) >> 4)];
		encoded += second === undefined ? '=' : alphabet[((second & 0x0f) << 2) | ((third ?? 0) >> 6)];
		encoded += third === undefined ? '=' : alphabet[third & 0x3f];
	}

	return encoded;
}
