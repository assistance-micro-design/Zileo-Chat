<!--
  Copyright 2025 Assistance Micro Design

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

<!--
  Spinner Component
  A loading spinner with configurable size.

  @example
  <Spinner />
  <Spinner size="lg" />
  <Spinner size={32} />
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';

	/**
	 * Spinner props
	 */
	interface Props {
		/** Size preset or pixel value */
		size?: 'sm' | 'md' | 'lg' | number;
		/** Accessible label (uses i18n default if not provided) */
		label?: string;
	}

	let { size = 'md', label }: Props = $props();

	/**
	 * Get label with i18n fallback
	 */
	const displayLabel = $derived(label ?? $i18n('ui_spinner_loading'));

	/**
	 * Compute actual size in pixels
	 */
	const pixelSize = $derived(
		typeof size === 'number' ? size : size === 'sm' ? 16 : size === 'lg' ? 32 : 20
	);
</script>

<span
	class="spinner"
	style="width: {pixelSize}px; height: {pixelSize}px;"
	role="status"
	aria-label={displayLabel}
>
	<span class="sr-only">{displayLabel}</span>
</span>

<style>
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}
</style>
