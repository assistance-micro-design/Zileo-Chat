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

import { describe, expect, it } from 'vitest';
import { validateVoxtralModelId } from '../stt-validation';

describe('validateVoxtralModelId', () => {
	it('rejects empty input', () => {
		expect(validateVoxtralModelId('')).toBe('errors.stt.model_id_required');
	});

	it('rejects whitespace-only input', () => {
		expect(validateVoxtralModelId('   \t\n')).toBe('errors.stt.model_id_required');
	});

	it('rejects strings longer than 128 chars', () => {
		const long = 'voxtral-' + 'a'.repeat(200);
		expect(validateVoxtralModelId(long)).toBe('errors.stt.model_id_too_long');
	});

	it('rejects strings containing forbidden characters', () => {
		expect(validateVoxtralModelId('voxtral mini')).toBe('errors.stt.model_id_invalid_chars');
		expect(validateVoxtralModelId('voxtral/mini')).toBe('errors.stt.model_id_invalid_chars');
		expect(validateVoxtralModelId('voxtral$mini')).toBe('errors.stt.model_id_invalid_chars');
	});

	it('rejects IDs that do not contain "voxtral"', () => {
		expect(validateVoxtralModelId('whisper-large')).toBe(
			'errors.stt.model_id_must_contain_voxtral'
		);
		expect(validateVoxtralModelId('mistral-small')).toBe(
			'errors.stt.model_id_must_contain_voxtral'
		);
	});

	it('rejects the realtime variant (batch endpoint cannot serve it)', () => {
		expect(validateVoxtralModelId('voxtral-mini-realtime')).toBe(
			'errors.stt.realtime_not_supported'
		);
		expect(validateVoxtralModelId('voxtral-realtime-2509')).toBe(
			'errors.stt.realtime_not_supported'
		);
	});

	it('matches "voxtral" case-insensitively', () => {
		expect(validateVoxtralModelId('Voxtral-Mini-2509')).toBeNull();
		expect(validateVoxtralModelId('VOXTRAL_LATEST')).toBeNull();
	});

	it('also catches the realtime variant case-insensitively', () => {
		expect(validateVoxtralModelId('Voxtral-Mini-REALTIME')).toBe(
			'errors.stt.realtime_not_supported'
		);
	});

	it('accepts canonical Voxtral model identifiers', () => {
		expect(validateVoxtralModelId('voxtral-mini-2509')).toBeNull();
		expect(validateVoxtralModelId('voxtral-small-latest')).toBeNull();
		expect(validateVoxtralModelId('voxtral.experimental-1')).toBeNull();
	});

	it('trims surrounding whitespace before validating', () => {
		expect(validateVoxtralModelId('   voxtral-mini-2509   ')).toBeNull();
	});

	it('treats the boundary length (128 chars) as valid', () => {
		const ok = 'voxtral-' + 'a'.repeat(120);
		expect(ok.length).toBe(128);
		expect(validateVoxtralModelId(ok)).toBeNull();
	});
});
