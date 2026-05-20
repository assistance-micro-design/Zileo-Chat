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
 * @fileoverview Client-side validators for STT settings. Backend re-applies
 * each check before persistence — this is purely for UX (inline form errors).
 */

const MODEL_ID_REGEX = /^[a-zA-Z0-9._-]+$/;
const MAX_MODEL_ID_LEN = 128;

/**
 * Returns an i18n key describing the error, or `null` when valid.
 */
export function validateVoxtralModelId(id: string): string | null {
	const trimmed = id.trim();
	if (!trimmed) return 'errors.stt.model_id_required';
	if (trimmed.length > MAX_MODEL_ID_LEN) return 'errors.stt.model_id_too_long';
	if (!MODEL_ID_REGEX.test(trimmed)) return 'errors.stt.model_id_invalid_chars';
	const lower = trimmed.toLowerCase();
	if (!lower.includes('voxtral')) return 'errors.stt.model_id_must_contain_voxtral';
	if (lower.includes('realtime')) return 'errors.stt.realtime_not_supported';
	return null;
}
