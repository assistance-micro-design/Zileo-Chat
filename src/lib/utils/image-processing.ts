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
 * Result of processing a file into a base64-encoded attachment payload.
 */
export interface ProcessedImage {
	data_base64: string;
	mime_type: string;
	size_bytes: number;
}

/**
 * Read a `File` as a `data:` URL via FileReader.
 */
export function readFileAsDataURL(file: File): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onload = () => {
			if (typeof reader.result === 'string') {
				resolve(reader.result);
			} else {
				reject(new Error('FileReader did not return a string'));
			}
		};
		reader.onerror = () => reject(reader.error ?? new Error('FileReader failed'));
		reader.readAsDataURL(file);
	});
}

/**
 * Load an `<img>` from a `data:` URL and resolve when decoded.
 */
export function loadImage(dataUrl: string): Promise<HTMLImageElement> {
	return new Promise((resolve, reject) => {
		const img = new Image();
		img.onload = () => resolve(img);
		img.onerror = () => reject(new Error('Failed to decode image'));
		img.src = dataUrl;
	});
}

/**
 * Scale a `(width, height)` pair down so the largest dimension equals
 * `maxDimension`. Returns the original size if both dimensions already fit.
 */
export function computeResizedDimensions(
	width: number,
	height: number,
	maxDimension: number
): { width: number; height: number } {
	if (width <= maxDimension && height <= maxDimension) {
		return { width, height };
	}
	const ratio = width >= height ? maxDimension / width : maxDimension / height;
	return {
		width: Math.round(width * ratio),
		height: Math.round(height * ratio)
	};
}

/**
 * Convert a canvas to a `Blob`, promisified.
 */
export function canvasToBlob(
	canvas: HTMLCanvasElement,
	mimeType: string,
	quality: number
): Promise<Blob> {
	return new Promise((resolve, reject) => {
		canvas.toBlob(
			(blob) => {
				if (blob) {
					resolve(blob);
				} else {
					reject(new Error('canvas.toBlob produced null'));
				}
			},
			mimeType,
			quality
		);
	});
}

/**
 * Convert an `ArrayBuffer` to standard base64.
 *
 * Uses `btoa` over a fromCharCode chain rather than spreading the typed array
 * into `String.fromCharCode(...bytes)` so we don't hit the per-call argument
 * limit on large images (~1 MB+ pushes V8 over the recursion threshold).
 */
export function arrayBufferToBase64(buffer: ArrayBuffer): string {
	const bytes = new Uint8Array(buffer);
	let binary = '';
	for (let i = 0; i < bytes.byteLength; i++) {
		// `bytes[i]` is `number | undefined` on TS strict + noUncheckedIndexedAccess;
		// the loop bound guarantees the value is defined.
		binary += String.fromCharCode(bytes[i] as number);
	}
	return btoa(binary);
}

/**
 * Resize and re-encode an image to keep the IPC payload bounded.
 *
 * - Pictures larger than `maxDimension` along their longest side are scaled
 *   down via a 2D canvas; the aspect ratio is preserved.
 * - Animated GIFs are re-encoded to PNG (animation is dropped — acceptable for
 *   v1, the vision model sees one frame anyway).
 * - The result is the **raw base64 payload** (no `data:` prefix); the prefix
 *   is rebuilt at the boundaries that need it.
 */
export async function processImageFile(file: File, maxDimension: number): Promise<ProcessedImage> {
	const dataUrl = await readFileAsDataURL(file);
	const img = await loadImage(dataUrl);
	const { width, height } = computeResizedDimensions(img.width, img.height, maxDimension);

	const canvas = document.createElement('canvas');
	canvas.width = width;
	canvas.height = height;
	const ctx = canvas.getContext('2d');
	if (!ctx) {
		throw new Error('Canvas 2D context unavailable');
	}
	ctx.drawImage(img, 0, 0, width, height);

	const outputType = file.type === 'image/gif' ? 'image/png' : file.type;
	const blob = await canvasToBlob(canvas, outputType, 0.92);
	const arrayBuffer = await blob.arrayBuffer();

	const fullBase64 = arrayBufferToBase64(arrayBuffer);
	return {
		data_base64: fullBase64,
		mime_type: outputType,
		size_bytes: blob.size
	};
}
