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
  ValidationModal Component
  Human-in-the-loop validation modal for approving or rejecting operations.
  Shows operation details, risk level, and action buttons.

  @example
  <ValidationModal request={validationRequest} onapprove={handleApprove} onreject={handleReject} />
-->
<script lang="ts">
	import type { ValidationRequest, RiskLevel } from '$types/validation';
	import Modal from '$lib/components/ui/Modal.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Badge from '$lib/components/ui/Badge.svelte';
	import { AlertTriangle, ShieldCheck, Info } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * ValidationModal props
	 */
	interface Props {
		/** Validation request data */
		request: ValidationRequest | null;
		/** Open state */
		open?: boolean;
		/** Approve handler */
		onapprove?: (request: ValidationRequest) => void;
		/** Reject handler */
		onreject?: (request: ValidationRequest, reason?: string) => void;
		/** Close handler */
		onclose?: () => void;
	}

	let { request, open = $bindable(false), onapprove, onreject, onclose }: Props = $props();

	let rejectReason = $state('');

	/**
	 * Map risk level to badge variant
	 */
	function getRiskBadgeVariant(level: RiskLevel): 'success' | 'warning' | 'error' {
		const variants = {
			low: 'success',
			medium: 'warning',
			high: 'error',
			critical: 'error'
		} as const;
		return variants[level];
	}

	/**
	 * Handle approval
	 */
	function handleApprove(): void {
		if (request) {
			onapprove?.(request);
			handleClose();
		}
	}

	/**
	 * Handle rejection
	 */
	function handleReject(): void {
		if (request) {
			onreject?.(request, rejectReason || undefined);
			handleClose();
		}
	}

	/**
	 * Handle modal close
	 */
	function handleClose(): void {
		open = false;
		rejectReason = '';
		onclose?.();
	}

	/**
	 * Format details object for display
	 */
	function formatDetails(details: Record<string, unknown>): string {
		return JSON.stringify(details, null, 2);
	}
</script>

<Modal {open} title={$i18n('workflow_validation_title')} onclose={handleClose}>
	{#snippet body()}
		{#if request}
			<div class="validation-content">
				<div class="validation-header">
					{#if request.risk_level === 'critical'}
						<AlertTriangle size={24} class="risk-icon critical" />
					{:else if request.risk_level === 'high'}
						<AlertTriangle size={24} class="risk-icon high" />
					{:else if request.risk_level === 'medium'}
						<Info size={24} class="risk-icon medium" />
					{:else}
						<ShieldCheck size={24} class="risk-icon low" />
					{/if}
					<div class="validation-info">
						<span class="validation-type">{request.type.replace('_', ' ')}</span>
						<Badge variant={getRiskBadgeVariant(request.risk_level)}>
							{$i18n('workflow_validation_risk').replace('{level}', request.risk_level)}
						</Badge>
					</div>
				</div>

				<div class="validation-operation">
					<h4>{$i18n('workflow_validation_operation')}</h4>
					<p>{request.operation}</p>
				</div>

				{#if Object.keys(request.details).length > 0}
					<div class="validation-details">
						<h4>{$i18n('workflow_validation_details')}</h4>
						<pre>{formatDetails(request.details)}</pre>
					</div>
				{/if}

				<div class="validation-warning">
					{#if request.risk_level === 'critical'}
						<AlertTriangle size={16} />
						<span>{$i18n('workflow_validation_critical_warning')}</span>
					{:else if request.risk_level === 'high'}
						<AlertTriangle size={16} />
						<span>{$i18n('workflow_validation_high_warning')}</span>
					{:else if request.risk_level === 'medium'}
						<Info size={16} />
						<span>{$i18n('workflow_validation_medium_warning')}</span>
					{/if}
				</div>

				<div class="reject-reason">
					<label for="reject-reason">{$i18n('workflow_validation_reject_label')}</label>
					<textarea
						id="reject-reason"
						bind:value={rejectReason}
						placeholder={$i18n('workflow_validation_reject_placeholder')}
						rows="2"
					></textarea>
				</div>
			</div>
		{/if}
	{/snippet}

	{#snippet footer()}
		<Button variant="ghost" onclick={handleClose}>{$i18n('common_cancel')}</Button>
		<Button variant="danger" onclick={handleReject}>{$i18n('workflow_validation_reject')}</Button>
		<Button variant="primary" onclick={handleApprove}>{$i18n('workflow_validation_approve')}</Button>
	{/snippet}
</Modal>

<style>
	.validation-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.validation-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
	}

	.validation-header :global(.risk-icon.critical) {
		color: var(--color-error);
		animation: pulse 1s infinite;
	}

	.validation-header :global(.risk-icon.high) {
		color: var(--color-error);
	}

	@keyframes pulse {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.5; }
	}

	.validation-header :global(.risk-icon.medium) {
		color: var(--color-warning);
	}

	.validation-header :global(.risk-icon.low) {
		color: var(--color-success);
	}

	.validation-info {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.validation-type {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		text-transform: capitalize;
	}

	.validation-operation h4,
	.validation-details h4 {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
		margin-bottom: var(--spacing-sm);
	}

	.validation-operation p {
		font-size: var(--font-size-base);
		color: var(--color-text-primary);
		margin: 0;
	}

	.validation-details pre {
		font-family: var(--font-mono);
		font-size: var(--font-size-xs);
		background: var(--color-bg-tertiary);
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		overflow-x: auto;
		margin: 0;
	}

	.validation-warning {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		background: var(--color-warning-light);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
		color: var(--color-warning);
	}

	.validation-warning:empty {
		display: none;
	}

	.reject-reason {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.reject-reason label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
	}

	.reject-reason textarea {
		width: 100%;
		padding: var(--spacing-sm) var(--spacing-md);
		font-size: var(--font-size-sm);
		font-family: inherit;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		resize: vertical;
	}

	.reject-reason textarea:focus {
		outline: none;
		border-color: var(--color-accent);
		box-shadow: 0 0 0 3px var(--color-accent-light);
	}
</style>
