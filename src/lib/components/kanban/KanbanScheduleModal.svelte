<!--
  Copyright 2025 Assistance Micro Design

  KanbanScheduleModal — manage recurrence on a completed kanban card.

  Loads the existing schedule for the given card (if any) from kanbanSchedules,
  lets the user pick days + time via KanbanScheduleForm, then creates / updates
  / deletes the kanban_schedule row through kanbanScheduleStore.
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';
	import { Button, Modal, DeleteConfirmModal } from '$lib/components/ui';
	import { getErrorMessage } from '$lib/utils/error';
	import { kanbanScheduleStore, kanbanSchedules } from '$lib/stores/kanban-schedule';
	import KanbanScheduleForm from './KanbanScheduleForm.svelte';
	import type { KanbanCard, KanbanSchedule } from '$types/kanban';

	interface Props {
		open: boolean;
		card: KanbanCard | null;
		onclose: () => void;
		onsaved?: () => void | Promise<void>;
	}

	let { open, card, onclose, onsaved }: Props = $props();

	const existing = $derived<KanbanSchedule | undefined>(
		card ? $kanbanSchedules.find((s) => s.card_template_id === card.id) : undefined
	);

	// Local form state — initialised from the existing schedule (if any) every
	// time the modal opens for a new card.
	let daysOfWeek = $state<number[]>([]);
	let hour = $state(9);
	let minute = $state(0);
	let enabled = $state(true);
	let skipIfPending = $state(false);
	let error = $state<string | null>(null);
	let saving = $state(false);

	let removeConfirmOpen = $state(false);

	let lastSyncedCardId = $state<string | null>(null);
	$effect(() => {
		if (!open || !card) {
			lastSyncedCardId = null;
			return;
		}
		if (lastSyncedCardId === card.id) return;
		lastSyncedCardId = card.id;
		if (existing) {
			daysOfWeek = existing.days_of_week.slice();
			hour = existing.hour;
			minute = existing.minute;
			enabled = existing.enabled;
			skipIfPending = existing.skip_if_pending;
		} else {
			daysOfWeek = [];
			hour = 9;
			minute = 0;
			enabled = true;
			skipIfPending = false;
		}
		error = null;
	});

	const canSubmit = $derived(daysOfWeek.length > 0 && !saving);

	function handleFormChange(value: {
		enabled: boolean;
		daysOfWeek: number[];
		hour: number;
		minute: number;
	}): void {
		enabled = value.enabled;
		daysOfWeek = value.daysOfWeek;
		hour = value.hour;
		minute = value.minute;
	}

	async function handleSave(): Promise<void> {
		if (!card) return;
		if (daysOfWeek.length === 0) {
			error = $i18n('kanban_error_schedule_days_required');
			return;
		}
		saving = true;
		error = null;
		try {
			if (existing) {
				await kanbanScheduleStore.updateSchedule(existing.id, {
					days_of_week: daysOfWeek,
					hour,
					minute,
					enabled,
					skip_if_pending: skipIfPending
				});
			} else {
				await kanbanScheduleStore.createSchedule({
					card_template_id: card.id,
					days_of_week: daysOfWeek,
					hour,
					minute,
					skip_if_pending: skipIfPending
				});
			}
			await onsaved?.();
			onclose();
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			saving = false;
		}
	}

	function requestRemove(): void {
		if (!existing) return;
		removeConfirmOpen = true;
	}

	function cancelRemove(): void {
		if (saving) return;
		removeConfirmOpen = false;
	}

	async function confirmRemove(): Promise<void> {
		if (!existing) return;
		saving = true;
		error = null;
		try {
			await kanbanScheduleStore.deleteSchedule(existing.id);
			removeConfirmOpen = false;
			await onsaved?.();
			onclose();
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			saving = false;
		}
	}
</script>

<Modal {open} title={$i18n('kanban_schedule_modal_title')} {onclose}>
	{#snippet body()}
		<p class="modal-hint">{$i18n('kanban_schedule_modal_subtitle')}</p>

		{#if card}
			<p class="modal-card-title"><strong>{card.title}</strong></p>
		{/if}

		<KanbanScheduleForm {daysOfWeek} {hour} {minute} {enabled} onchange={handleFormChange} />

		<label class="skip-pending">
			<input
				type="checkbox"
				checked={skipIfPending}
				onchange={(e) => (skipIfPending = (e.target as HTMLInputElement).checked)}
			/>
			<span>
				<strong>{$i18n('kanban_schedule_skip_if_pending')}</strong>
				<small>{$i18n('kanban_schedule_skip_if_pending_hint')}</small>
			</span>
		</label>

		{#if error}
			<p class="modal-error" role="alert">{error}</p>
		{/if}
	{/snippet}

	{#snippet footer()}
		{#if existing}
			<Button type="button" variant="ghost" onclick={requestRemove} disabled={saving}>
				{$i18n('kanban_schedule_remove_btn')}
			</Button>
		{/if}
		<Button type="button" variant="ghost" onclick={onclose} disabled={saving}>
			{$i18n('common_cancel')}
		</Button>
		<Button type="button" variant="primary" onclick={handleSave} disabled={!canSubmit}>
			{existing ? $i18n('kanban_schedule_save_btn') : $i18n('kanban_schedule_create_btn')}
		</Button>
	{/snippet}
</Modal>

<DeleteConfirmModal
	open={removeConfirmOpen}
	titleKey="kanban_schedule_remove_modal_title"
	confirmMessageKey="kanban_schedule_confirm_remove"
	deleting={saving}
	onConfirm={confirmRemove}
	onCancel={cancelRemove}
/>

<style>
	.modal-hint {
		margin: 0 0 0.5rem;
		color: var(--color-text-muted);
		font-size: 0.85rem;
	}
	.modal-card-title {
		margin: 0 0 0.75rem;
		font-size: 0.9rem;
	}
	.skip-pending {
		display: flex;
		align-items: flex-start;
		gap: 0.5rem;
		margin: 0.75rem 0 0;
		padding: 0.6rem 0.75rem;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		cursor: pointer;
	}
	.skip-pending input {
		margin-top: 0.2rem;
	}
	.skip-pending span {
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
	}
	.skip-pending small {
		color: var(--color-text-muted);
		font-size: 0.8rem;
	}
	.modal-error {
		color: var(--color-error);
		margin: 0.5rem 0 0;
		font-size: 0.85rem;
	}
</style>
