<!--
  NewWorkflowModal Component
  A modal dialog for creating new workflows with name input and agent selection.

  @example
  <NewWorkflowModal
    open={showModal}
    agents={agentList}
    selectedAgentId={currentAgentId}
    oncreate={handleCreate}
    onclose={() => showModal = false}
  />
-->
<script lang="ts">
	import { X, Workflow, Bot } from 'lucide-svelte';
	import type { AgentSummary } from '$types/agent';
	import { Button } from '$lib/components/ui';

	/**
	 * NewWorkflowModal props
	 */
	interface Props {
		/** Whether the modal is open */
		open: boolean;
		/** Available agents for selection */
		agents: AgentSummary[];
		/** Pre-selected agent ID */
		selectedAgentId?: string | null;
		/** Create handler - receives workflow name and agent ID */
		oncreate: (name: string, agentId: string) => void;
		/** Close handler */
		onclose: () => void;
	}

	let { open, agents, selectedAgentId = null, oncreate, onclose }: Props = $props();

	/** Form state */
	let workflowName = $state('');
	let chosenAgentId = $state<string | null>(null);
	let nameInputRef = $state<HTMLInputElement | null>(null);
	let isSubmitting = $state(false);
	let error = $state<string | null>(null);

	/** Sync chosen agent when modal opens or selectedAgentId changes */
	$effect(() => {
		if (open) {
			chosenAgentId = selectedAgentId || (agents.length > 0 ? agents[0].id : null);
			workflowName = '';
			error = null;
			isSubmitting = false;
			// Focus input after mount
			setTimeout(() => nameInputRef?.focus(), 50);
		}
	});

	/**
	 * Validate and submit the form
	 */
	function handleSubmit(event: Event): void {
		event.preventDefault();
		error = null;

		const trimmedName = workflowName.trim();
		if (!trimmedName) {
			error = 'Please enter a workflow name';
			nameInputRef?.focus();
			return;
		}

		if (!chosenAgentId) {
			error = 'Please select an agent';
			return;
		}

		isSubmitting = true;
		oncreate(trimmedName, chosenAgentId);
	}

	/**
	 * Handle keyboard events for accessibility
	 */
	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape') {
			onclose();
		}
	}

	/**
	 * Handle backdrop click to close modal
	 */
	function handleBackdropClick(event: MouseEvent): void {
		if (event.target === event.currentTarget) {
			onclose();
		}
	}
</script>

<svelte:window onkeydown={open ? handleKeydown : undefined} />

{#if open}
	<div class="modal-backdrop" role="presentation" onclick={handleBackdropClick} onkeydown={handleKeydown}>
		<div class="modal new-workflow-modal" role="dialog" aria-modal="true" aria-labelledby="modal-title">
			<div class="modal-header">
				<div class="modal-title-wrapper">
					<Workflow size={24} class="modal-icon" />
					<h3 id="modal-title" class="modal-title">New Workflow</h3>
				</div>
				<button type="button" class="close-button" onclick={onclose} aria-label="Close">
					<X size={20} />
				</button>
			</div>

			<form onsubmit={handleSubmit}>
				<div class="modal-body">
					{#if error}
						<div class="error-message" role="alert">
							{error}
						</div>
					{/if}

					<div class="form-field">
						<label for="workflow-name" class="form-label">
							Workflow Name
							<span class="required">*</span>
						</label>
						<input
							bind:this={nameInputRef}
							bind:value={workflowName}
							type="text"
							id="workflow-name"
							class="form-input"
							placeholder="Enter workflow name..."
							autocomplete="off"
							disabled={isSubmitting}
						/>
						<p class="form-hint">Give your workflow a descriptive name</p>
					</div>

					<div class="form-field">
						<label for="workflow-agent" class="form-label">
							<Bot size={16} class="label-icon" />
							Agent
						</label>
						{#if agents.length === 0}
							<div class="no-agents-warning">
								No agents available. Please create an agent in Settings first.
							</div>
						{:else}
							<div class="agent-selector">
								{#each agents as agent (agent.id)}
									<button
										type="button"
										class="agent-option"
										class:selected={chosenAgentId === agent.id}
										onclick={() => chosenAgentId = agent.id}
										disabled={isSubmitting}
									>
										<span class="agent-avatar">{agent.name.charAt(0).toUpperCase()}</span>
										<span class="agent-name">{agent.name}</span>
										{#if chosenAgentId === agent.id}
											<span class="selected-indicator"></span>
										{/if}
									</button>
								{/each}
							</div>
						{/if}
					</div>
				</div>

				<div class="modal-footer">
					<Button variant="ghost" onclick={onclose} disabled={isSubmitting}>
						Cancel
					</Button>
					<Button
						variant="primary"
						type="submit"
						disabled={isSubmitting || agents.length === 0 || !workflowName.trim()}
					>
						{#if isSubmitting}
							Creating...
						{:else}
							Create Workflow
						{/if}
					</Button>
				</div>
			</form>
		</div>
	</div>
{/if}

<style>
	.modal-backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		backdrop-filter: blur(4px);
		z-index: var(--z-index-modal-backdrop);
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-xl);
		animation: fadeIn 0.15s ease-out;
	}

	@keyframes fadeIn {
		from { opacity: 0; }
		to { opacity: 1; }
	}

	.new-workflow-modal {
		background: var(--color-bg-primary);
		border-radius: var(--border-radius-xl);
		box-shadow: var(--shadow-xl);
		width: 100%;
		max-width: 480px;
		overflow: hidden;
		animation: slideUp 0.2s ease-out;
	}

	@keyframes slideUp {
		from {
			opacity: 0;
			transform: translateY(20px) scale(0.98);
		}
		to {
			opacity: 1;
			transform: translateY(0) scale(1);
		}
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--spacing-lg);
		border-bottom: 1px solid var(--color-border);
		background: linear-gradient(180deg, var(--color-bg-secondary) 0%, var(--color-bg-primary) 100%);
	}

	.modal-title-wrapper {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
	}

	.modal-title-wrapper :global(.modal-icon) {
		color: var(--color-accent);
	}

	.modal-title {
		font-size: var(--font-size-xl);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.close-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 36px;
		height: 36px;
		background: transparent;
		border: none;
		border-radius: var(--border-radius-md);
		color: var(--color-text-tertiary);
		cursor: pointer;
		transition: all var(--transition-fast);
	}

	.close-button:hover {
		background: var(--color-bg-hover);
		color: var(--color-text-primary);
	}

	.modal-body {
		padding: var(--spacing-lg);
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.error-message {
		padding: var(--spacing-sm) var(--spacing-md);
		background: var(--color-error-light);
		color: var(--color-error);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
	}

	.form-field {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.form-label {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.form-label :global(.label-icon) {
		color: var(--color-text-tertiary);
	}

	.required {
		color: var(--color-error);
	}

	.form-input {
		width: 100%;
		padding: var(--spacing-md);
		font-size: var(--font-size-base);
		font-family: var(--font-family);
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
	}

	.form-input:focus {
		outline: none;
		border-color: var(--color-accent);
		box-shadow: 0 0 0 3px var(--color-accent-light);
	}

	.form-input:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.form-input::placeholder {
		color: var(--color-text-tertiary);
	}

	.form-hint {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin: 0;
	}

	.no-agents-warning {
		padding: var(--spacing-md);
		background: var(--color-warning-light);
		color: var(--color-warning);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
		text-align: center;
	}

	.agent-selector {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		max-height: 200px;
		overflow-y: auto;
	}

	.agent-option {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-sm) var(--spacing-md);
		background: var(--color-bg-secondary);
		border: 2px solid transparent;
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: all var(--transition-fast);
		text-align: left;
	}

	.agent-option:hover {
		background: var(--color-bg-hover);
	}

	.agent-option.selected {
		background: var(--color-accent-light);
		border-color: var(--color-accent);
	}

	.agent-option:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.agent-avatar {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-secondary);
	}

	.agent-option.selected .agent-avatar {
		background: var(--color-accent);
		color: var(--color-accent-text);
	}

	.agent-name {
		flex: 1;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.selected-indicator {
		width: 8px;
		height: 8px;
		background: var(--color-accent);
		border-radius: var(--border-radius-full);
	}

	.modal-footer {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-md);
		padding: var(--spacing-lg);
		border-top: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
	}
</style>
