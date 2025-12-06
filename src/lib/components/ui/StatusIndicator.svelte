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
  StatusIndicator Component
  A small dot indicator showing workflow/process status.
  Includes animation for running state.

  @example
  <StatusIndicator status="idle" />
  <StatusIndicator status="running" />
  <StatusIndicator status="completed" />
  <StatusIndicator status="error" />
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';

	/**
	 * Possible status states
	 */
	export type Status = 'idle' | 'running' | 'completed' | 'error';

	/**
	 * StatusIndicator props
	 */
	interface Props {
		/** Current status */
		status: Status;
		/** Optional size modifier */
		size?: 'sm' | 'md' | 'lg';
	}

	let { status, size = 'md' }: Props = $props();

	/**
	 * Map status to i18n key
	 */
	const statusI18nKeys: Record<Status, string> = {
		idle: 'ui_status_idle',
		running: 'ui_status_running',
		completed: 'ui_status_completed',
		error: 'ui_status_error'
	};

	/**
	 * Get translated status label
	 */
	const translatedStatus = $derived($i18n(statusI18nKeys[status]));

	/**
	 * Get aria label for accessibility
	 */
	const ariaLabel = $derived($i18n('ui_status_label').replace('{status}', translatedStatus));
</script>

<span
	class="status-indicator status-{status}"
	class:status-sm={size === 'sm'}
	class:status-lg={size === 'lg'}
	role="status"
	aria-label={ariaLabel}
></span>

<style>
	.status-sm {
		width: 6px;
		height: 6px;
	}

	.status-lg {
		width: 12px;
		height: 12px;
	}
</style>
