// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/**
 * Allowed URL schemes for external link opening.
 * Defense-in-depth: DOMPurify handles XSS, but we also validate
 * before passing URLs to Tauri's openUrl().
 */
const ALLOWED_SCHEMES = ['https:', 'http:', 'mailto:'];

/**
 * Checks if a URL has an allowed scheme for external opening.
 * Blocks dangerous schemes like javascript:, data:, vbscript:.
 * Allows relative URLs and fragment-only URLs.
 *
 * @param url - The URL to validate
 * @returns true if the URL scheme is safe to open
 */
export function isAllowedScheme(url: string): boolean {
	if (!url) return false;

	// Relative URLs and fragments are safe
	if (url.startsWith('/') || url.startsWith('#')) return true;

	// Check if URL has a scheme
	const colonIndex = url.indexOf(':');
	if (colonIndex === -1) return true; // No scheme = relative URL

	const scheme = url.slice(0, colonIndex + 1).toLowerCase();
	return ALLOWED_SCHEMES.includes(scheme);
}
