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
  ToastItem Component
  Renders a single toast notification with icon, title, message,
  optional action button, and dismiss control.
-->

<script lang="ts">
	import type { Toast } from '$types/background-workflow';
	import { toastStore } from '$lib/stores/toast';
	import { i18n } from '$lib/i18n';
	import { X, CheckCircle, AlertCircle, Info, AlertTriangle, HelpCircle } from '@lucide/svelte';

	interface Props {
		toast: Toast;
		onnavigate?: (workflowId: string) => void;
	}

	let { toast, onnavigate }: Props = $props();

	const iconMap: Record<string, typeof CheckCircle> = {
		success: CheckCircle,
		error: AlertCircle,
		info: Info,
		warning: AlertTriangle,
		'user-question': HelpCircle
	};

	let Icon = $derived(iconMap[toast.type] ?? Info);
</script>

<div class="toast-item toast-{toast.type}" role="alert">
	<div class="toast-icon">
		<Icon size={18} />
	</div>
	<div class="toast-body">
		<div class="toast-title">{toast.title}</div>
		<div class="toast-message">{toast.message}</div>
		{#if toast.workflowId && toast.type === 'user-question'}
			<button
				class="toast-action"
				onclick={() => onnavigate?.(toast.workflowId!)}
			>
				{$i18n('toast_go_to_workflow')}
			</button>
		{/if}
	</div>
	<button
		class="toast-dismiss"
		onclick={() => toastStore.dismiss(toast.id)}
		aria-label={$i18n('toast_dismiss_arialabel')}
	>
		<X size={14} />
	</button>
</div>

<style>
	.toast-item {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		border: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
		pointer-events: auto;
		animation: toast-in 0.3s ease-out;
		max-width: 400px;
	}

	@keyframes toast-in {
		from {
			opacity: 0;
			transform: translateX(100%);
		}
		to {
			opacity: 1;
			transform: translateX(0);
		}
	}

	.toast-success {
		border-left: 3px solid var(--color-success);
	}

	.toast-error {
		border-left: 3px solid var(--color-error);
	}

	.toast-warning {
		border-left: 3px solid var(--color-warning);
	}

	.toast-info {
		border-left: 3px solid var(--color-primary);
	}

	.toast-user-question {
		border-left: 3px solid var(--color-warning);
	}

	.toast-success .toast-icon {
		color: var(--color-success);
	}

	.toast-error .toast-icon {
		color: var(--color-error);
	}

	.toast-warning .toast-icon {
		color: var(--color-warning);
	}

	.toast-info .toast-icon {
		color: var(--color-primary);
	}

	.toast-user-question .toast-icon {
		color: var(--color-warning);
	}

	.toast-icon {
		flex-shrink: 0;
		margin-top: 2px;
	}

	.toast-body {
		flex: 1;
		min-width: 0;
	}

	.toast-title {
		font-weight: var(--font-weight-semibold);
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
	}

	.toast-message {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		margin-top: 2px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.toast-action {
		display: inline-block;
		margin-top: var(--spacing-xs);
		padding: 0;
		background: none;
		border: none;
		color: var(--color-primary);
		font-size: var(--font-size-xs);
		font-weight: var(--font-weight-medium);
		cursor: pointer;
		text-decoration: underline;
	}

	.toast-action:hover {
		color: var(--color-primary-hover, var(--color-primary));
	}

	.toast-dismiss {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
		padding: 0;
		background: none;
		border: none;
		color: var(--color-text-tertiary);
		cursor: pointer;
		border-radius: var(--border-radius-sm);
		transition:
			color var(--transition-fast),
			background-color var(--transition-fast);
	}

	.toast-dismiss:hover {
		color: var(--color-text-primary);
		background: var(--color-bg-hover);
	}
</style>
