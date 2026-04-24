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
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

ErrorBanner - Reusable persistent banner for blocking feedback that must stay
in view until acknowledged. Two variants are supported: `error` (default) for
blocking failures and `warning` for recoverable issues that require attention.
For transient action feedback (success/error after CRUD), use the toast store.
-->

<script lang="ts">
	import { i18n } from '$lib/i18n';

	interface Props {
		/** Message to display */
		message: string;
		/** Callback when dismiss button is clicked */
		onDismiss: () => void;
		/** Visual variant: error (default) for blocking issues, warning for advisories */
		variant?: 'error' | 'warning';
		/** Optional dismiss button label (defaults to common_close) */
		dismissLabel?: string;
	}

	let { message, onDismiss, variant = 'error', dismissLabel }: Props = $props();
</script>

<div class="error-banner {variant}" role="alert">
	<span class="error-text">{message}</span>
	<button type="button" class="dismiss-btn" onclick={onDismiss}>
		{dismissLabel ?? $i18n('common_close')}
	</button>
</div>

<style>
	.error-banner {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
	}

	.error-banner.error {
		background: var(--color-error-light);
		color: var(--color-error);
	}

	.error-banner.warning {
		background: var(--color-warning-bg);
		color: var(--color-warning);
	}

	.error-text {
		font-size: var(--font-size-sm);
	}

	.dismiss-btn {
		background: transparent;
		border: none;
		cursor: pointer;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		padding: var(--spacing-xs) var(--spacing-sm);
		border-radius: var(--border-radius-sm);
		color: inherit;
	}

	.dismiss-btn:hover {
		background: rgba(0, 0, 0, 0.1);
	}
</style>
