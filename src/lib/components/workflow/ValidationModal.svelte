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
	import { TriangleAlert, ShieldCheck, Info } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * ValidationModal props
	 */
	interface Props {
		/** Validation request data */
		request: ValidationRequest | null;
		/** Open state — driven ONE-WAY by the store; never mutated locally. */
		open?: boolean;
		/** True while a decision is in flight to the backend (disables buttons). */
		processing?: boolean;
		/** Last validation error to surface in the modal (null when none). */
		error?: string | null;
		/** Number of pending validations in the FIFO queue (this one + waiting). */
		queueCount?: number;
		/** Approve handler */
		onapprove?: (request: ValidationRequest) => void;
		/** Reject handler */
		onreject?: (request: ValidationRequest, reason?: string) => void;
	}

	let {
		request,
		open = false,
		processing = false,
		error = null,
		queueCount = 1,
		onapprove,
		onreject
	}: Props = $props();

	let rejectReason = $state('');

	/**
	 * A `manager_write` request carries a backend-authored authority pair
	 * (`tool_id` + `operation`) that is shown dominantly, plus an untrusted,
	 * agent-supplied `agent_preview` shown with a clear "not trustworthy" label.
	 */
	let isManagerWrite = $derived(request?.type === 'manager_write');

	function detailString(key: string): string {
		const v = request?.details?.[key];
		return typeof v === 'string' ? v : '';
	}

	// Reset the reject reason whenever a NEW request arrives, so a fresh
	// validation starts with an empty field (mirrors UserQuestionModal). Never
	// touches `open`: the modal's visibility is owned solely by the store.
	$effect(() => {
		if (request) rejectReason = '';
	});

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

	// Emit the decision and let the STORE close the modal by clearing `pending`
	// on success. On an IPC error the store keeps `pending` set (only lastError
	// is posted), so the modal STAYS open and the user can retry — there is no
	// local `open = false` that would hide a still-pending validation. This is
	// what makes the error path fail-closed.
	function handleApprove(): void {
		if (request) onapprove?.(request);
	}

	function handleReject(): void {
		if (request) onreject?.(request, rejectReason || undefined);
	}

	// Required by Modal but never fires: backdrop, Escape and the header close
	// button are all disabled, so there is no dismiss path. Visibility is
	// entirely store-driven.
	function onCloseNoop(): void {
		/* intentionally empty — the modal is non-dismissable */
	}

	/**
	 * Format details object for display
	 */
	function formatDetails(details: Record<string, unknown>): string {
		return JSON.stringify(details, null, 2);
	}
</script>

<!--
	Fail-safe: this modal must NOT be dismissable without an explicit decision,
	AND it must not close on a failed decision. Backdrop, Escape and the header
	close button are all disabled, and the footer has no Cancel. The modal closes
	ONLY when the STORE clears the pending entry — i.e. a decision that the
	backend actually confirmed (success), or a backend resolution (timeout). An
	IPC error keeps the pending entry set, so the modal stays open and the user
	can retry. Reject is the explicit "no" (optional reason). Buttons are disabled
	while a decision is in flight to prevent a double / approve-then-reject.
-->
<Modal
	{open}
	title={$i18n('workflow_validation_title')}
	onclose={onCloseNoop}
	closeOnBackdrop={false}
	closeOnEscape={false}
	showCloseButton={false}
>
	{#snippet body()}
		{#if request}
			<div class="validation-content">
				<div class="validation-header">
					{#if request.risk_level === 'critical'}
						<TriangleAlert size={24} class="risk-icon critical" />
					{:else if request.risk_level === 'high'}
						<TriangleAlert size={24} class="risk-icon high" />
					{:else if request.risk_level === 'medium'}
						<Info size={24} class="risk-icon medium" />
					{:else}
						<ShieldCheck size={24} class="risk-icon low" />
					{/if}
					<div class="validation-info">
						<span class="validation-type">{request.type.replace('_', ' ')}</span>
						<div class="validation-badges">
							<Badge variant={getRiskBadgeVariant(request.risk_level)}>
								{$i18n('workflow_validation_risk').replace('{level}', request.risk_level)}
							</Badge>
							<Badge variant="primary">{$i18n('workflow_validation_badge_attached')}</Badge>
							{#if queueCount > 1}
								<Badge variant="warning">
									{$i18n('workflow_validation_queue_count').replace('{count}', String(queueCount))}
								</Badge>
							{/if}
						</div>
					</div>
				</div>

				{#if isManagerWrite}
					<!--
						manager_write: the authority pair (tool + operation) is the
						BACKEND-decided truth and is shown dominantly; the agent-supplied
						preview below it is explicitly labeled untrusted.
					-->
					<div class="validation-authority">
						<h4>{$i18n('workflow_validation_authority')}</h4>
						<p class="authority-line">
							<span class="authority-tool">{detailString('tool_id')}</span>
							<span class="authority-op">{detailString('operation')}</span>
						</p>
					</div>
					{#if detailString('agent_preview')}
						<div class="validation-untrusted">
							<h4>{$i18n('workflow_validation_untrusted_preview')}</h4>
							<pre class="untrusted">{detailString('agent_preview')}</pre>
						</div>
					{/if}
				{:else}
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
				{/if}

				<div class="validation-warning">
					{#if request.risk_level === 'critical'}
						<TriangleAlert size={16} />
						<span>{$i18n('workflow_validation_critical_warning')}</span>
					{:else if request.risk_level === 'high'}
						<TriangleAlert size={16} />
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

				{#if error}
					<p class="validation-error" role="alert">{error}</p>
				{/if}
			</div>
		{/if}
	{/snippet}

	{#snippet footer()}
		<Button variant="danger" onclick={handleReject} disabled={processing}
			>{$i18n('workflow_validation_reject')}</Button
		>
		<Button variant="primary" onclick={handleApprove} disabled={processing}
			>{$i18n('workflow_validation_approve')}</Button
		>
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

	.validation-badges {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-xs);
		align-items: center;
	}

	.validation-authority h4,
	.validation-untrusted h4 {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
		margin-bottom: var(--spacing-sm);
	}

	.authority-line {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-sm);
		align-items: baseline;
		margin: 0;
	}

	.authority-tool {
		font-weight: var(--font-weight-bold, 700);
		color: var(--color-text-primary);
	}

	.authority-op {
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		color: var(--color-accent);
	}

	.validation-untrusted pre.untrusted {
		font-family: var(--font-mono);
		font-size: var(--font-size-xs);
		background: var(--color-bg-tertiary);
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		border-left: 3px solid var(--color-warning);
		overflow-x: auto;
		white-space: pre-wrap;
		word-break: break-word;
		margin: 0;
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
		background: var(--color-warning-bg);
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

	.validation-error {
		margin: 0;
		padding: var(--spacing-sm);
		background: var(--color-danger-light);
		border-left: 3px solid var(--color-danger);
		border-radius: var(--border-radius-sm);
		font-size: var(--font-size-sm);
		color: var(--color-danger);
	}
</style>
