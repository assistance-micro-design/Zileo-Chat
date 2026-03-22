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
 * Utility functions for context menu positioning and keyboard navigation.
 * @module utils/contextMenu
 */

import type { ContextMenuItem } from '$types/sidebar';

/**
 * Adjust menu position to keep it within the viewport.
 * @param x - Desired X position
 * @param y - Desired Y position
 * @param menuWidth - Width of the menu element
 * @param menuHeight - Height of the menu element
 * @param viewportWidth - Viewport width
 * @param viewportHeight - Viewport height
 * @returns Adjusted { x, y } coordinates
 */
export function adjustMenuPosition(
	x: number,
	y: number,
	menuWidth: number,
	menuHeight: number,
	viewportWidth: number,
	viewportHeight: number
): { x: number; y: number } {
	const adjustedX = x + menuWidth > viewportWidth ? Math.max(0, x - menuWidth) : x;
	const adjustedY = y + menuHeight > viewportHeight ? Math.max(0, y - menuHeight) : y;
	return { x: adjustedX, y: adjustedY };
}

/**
 * Get the next focusable index, skipping disabled items.
 * @param items - Menu items array
 * @param currentIndex - Current focused index
 * @param direction - 1 for down, -1 for up
 * @returns Next valid index
 */
export function getNextFocusableIndex(
	items: ContextMenuItem[],
	currentIndex: number,
	direction: 1 | -1
): number {
	if (items.length === 0) return -1;

	let nextIndex = currentIndex + direction;

	// Wrap around
	if (nextIndex >= items.length) nextIndex = 0;
	if (nextIndex < 0) nextIndex = items.length - 1;

	// Skip disabled items (with circuit breaker to avoid infinite loop)
	let attempts = 0;
	while (items[nextIndex]?.disabled && attempts < items.length) {
		nextIndex += direction;
		if (nextIndex >= items.length) nextIndex = 0;
		if (nextIndex < 0) nextIndex = items.length - 1;
		attempts++;
	}

	return nextIndex;
}
