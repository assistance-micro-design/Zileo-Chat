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
 * UI Components Index
 * Re-exports all UI components for easy importing
 *
 * @example
 * import { Button, Card, Modal } from '$lib/components/ui';
 */

export { default as Button } from './Button.svelte';
export { default as Badge } from './Badge.svelte';
export { default as StatusIndicator } from './StatusIndicator.svelte';
export type { Status } from './StatusIndicator.svelte';
export { default as Spinner } from './Spinner.svelte';
export { default as ProgressBar } from './ProgressBar.svelte';
export { default as Card } from './Card.svelte';
export { default as Modal } from './Modal.svelte';
export { default as Input } from './Input.svelte';
export { default as Select } from './Select.svelte';
export type { SelectOption } from './Select.svelte';
export { default as Textarea } from './Textarea.svelte';
export { default as Skeleton } from './Skeleton.svelte';
export type { SkeletonVariant } from './Skeleton.svelte';
export { default as LanguageSelector } from './LanguageSelector.svelte';
export { default as HelpButton } from './HelpButton.svelte';
