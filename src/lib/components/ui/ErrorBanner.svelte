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

ErrorBanner - Reusable error display banner with dismiss button.
Extracted from AgentSettings and PromptSettings.
-->

<script lang="ts">
	import { i18n } from '$lib/i18n';

	interface Props {
		/** Error message to display */
		message: string;
		/** Callback when dismiss button is clicked */
		onDismiss: () => void;
		/** Optional dismiss button label (defaults to common_close) */
		dismissLabel?: string;
	}

	let { message, onDismiss, dismissLabel }: Props = $props();
</script>

<div class="error-banner" role="alert">
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
		background: var(--color-error-light);
		color: var(--color-error);
		border-radius: var(--border-radius-md);
	}

	.error-text {
		font-size: var(--font-size-sm);
	}

	.dismiss-btn {
		background: transparent;
		border: none;
		color: var(--color-error);
		cursor: pointer;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		padding: var(--spacing-xs) var(--spacing-sm);
		border-radius: var(--border-radius-sm);
	}

	.dismiss-btn:hover {
		background: rgba(0, 0, 0, 0.1);
	}
</style>
