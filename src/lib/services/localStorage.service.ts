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
 * LocalStorage Service
 *
 * Provides type-safe access to localStorage with validation and error handling.
 * All keys used in the application should be defined in the KEYS constant.
 */

const KEYS = {
	RIGHT_SIDEBAR_COLLAPSED: 'zileo_right_sidebar_collapsed',
	LEFT_SIDEBAR_COLLAPSED: 'zileo_left_sidebar_collapsed',
	SELECTED_WORKFLOW_ID: 'zileo_last_workflow_id'
} as const;

type StorageKey = (typeof KEYS)[keyof typeof KEYS];

export const LocalStorage = {
	/**
	 * Get a value from localStorage with type safety and error handling.
	 * @param key - The storage key to retrieve
	 * @param defaultValue - The default value to return if key doesn't exist or parsing fails
	 * @returns The stored value or the default value
	 */
	get<T>(key: StorageKey, defaultValue: T): T {
		if (typeof window === 'undefined') {
			return defaultValue;
		}

		try {
			const item = localStorage.getItem(key);
			return item ? JSON.parse(item) : defaultValue;
		} catch {
			return defaultValue;
		}
	},

	/**
	 * Set a value in localStorage with type safety.
	 * @param key - The storage key to set
	 * @param value - The value to store (will be JSON stringified)
	 */
	set<T>(key: StorageKey, value: T): void {
		if (typeof window === 'undefined') {
			return;
		}

		try {
			localStorage.setItem(key, JSON.stringify(value));
		} catch (error) {
			console.error('Failed to save to localStorage:', error);
		}
	},

	/**
	 * Remove a value from localStorage.
	 * @param key - The storage key to remove
	 */
	remove(key: StorageKey): void {
		if (typeof window === 'undefined') {
			return;
		}

		try {
			localStorage.removeItem(key);
		} catch (error) {
			console.error('Failed to remove from localStorage:', error);
		}
	}
};

/**
 * Export the KEYS constant for use in components.
 */
export const STORAGE_KEYS = KEYS;
