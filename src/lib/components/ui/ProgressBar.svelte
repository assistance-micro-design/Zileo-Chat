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
  ProgressBar Component
  A progress indicator showing completion percentage.

  @example
  <ProgressBar value={50} />
  <ProgressBar value={75} max={100} />
  <ProgressBar value={3} max={10} showLabel />
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';

	/**
	 * ProgressBar props
	 */
	interface Props {
		/** Current progress value */
		value: number;
		/** Maximum value (default 100) */
		max?: number;
		/** Show percentage label */
		showLabel?: boolean;
		/** Accessible label (uses i18n default if not provided) */
		label?: string;
	}

	let { value, max = 100, showLabel = false, label }: Props = $props();

	/**
	 * Get label with i18n fallback
	 */
	const displayLabel = $derived(label ?? $i18n('ui_progress_label'));

	/**
	 * Calculate percentage
	 */
	const percentage = $derived(Math.min(100, Math.max(0, (value / max) * 100)));
</script>

<div class="progress-wrapper">
	<div
		class="progress-bar"
		role="progressbar"
		aria-valuenow={value}
		aria-valuemin={0}
		aria-valuemax={max}
		aria-label={displayLabel}
	>
		<div class="progress-fill" style="width: {percentage}%;"></div>
	</div>
	{#if showLabel}
		<span class="progress-label">{Math.round(percentage)}%</span>
	{/if}
</div>

<style>
	.progress-wrapper {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.progress-bar {
		flex: 1;
	}

	.progress-label {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		min-width: 40px;
		text-align: right;
	}
</style>
