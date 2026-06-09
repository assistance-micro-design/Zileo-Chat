<!--
  Copyright 2025 Assistance Micro Design

  KanbanScheduleForm — recurrence sub-form (days of week + time of day).
  Designed to be embedded in a card creator. The parent collects the values
  via `onchange` and creates the schedule separately after card creation.
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';

	interface Props {
		/** Selected weekdays — 0 = Monday … 6 = Sunday (Rust convention). */
		daysOfWeek?: number[];
		hour?: number;
		minute?: number;
		enabled?: boolean;
		onchange: (value: {
			enabled: boolean;
			daysOfWeek: number[];
			hour: number;
			minute: number;
		}) => void;
	}

	let { daysOfWeek = [], hour = 9, minute = 0, enabled = false, onchange }: Props = $props();

	const dayKeys = ['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'] as const;

	function emit(part: Partial<Parameters<typeof onchange>[0]>): void {
		const next = {
			enabled,
			daysOfWeek: daysOfWeek.slice().sort((a, b) => a - b),
			hour,
			minute,
			...part
		};
		next.daysOfWeek = next.daysOfWeek.slice().sort((a, b) => a - b);
		onchange(next);
	}

	function toggleDay(idx: number): void {
		const exists = daysOfWeek.includes(idx);
		const next = exists ? daysOfWeek.filter((d) => d !== idx) : [...daysOfWeek, idx];
		emit({ daysOfWeek: next });
	}

	function handleTimeChange(event: Event): void {
		const [h, m] = (event.target as HTMLInputElement).value.split(':');
		const hh = Math.max(0, Math.min(23, Number.parseInt(h ?? '0', 10) || 0));
		const mm = Math.max(0, Math.min(59, Number.parseInt(m ?? '0', 10) || 0));
		emit({ hour: hh, minute: mm });
	}

	const timeValue = $derived(
		`${hour.toString().padStart(2, '0')}:${minute.toString().padStart(2, '0')}`
	);
</script>

<fieldset class="kanban-schedule-form">
	<legend>
		<label>
			<input
				type="checkbox"
				checked={enabled}
				onchange={(e) => emit({ enabled: (e.target as HTMLInputElement).checked })}
			/>
			{$i18n('kanban_schedule_enable')}
		</label>
	</legend>

	{#if enabled}
		<div class="days">
			{#each dayKeys as key, idx (key)}
				<label class="day-chip" class:active={daysOfWeek.includes(idx)}>
					<input
						type="checkbox"
						checked={daysOfWeek.includes(idx)}
						onchange={() => toggleDay(idx)}
					/>
					<span>{$i18n(`kanban_day_${key}`)}</span>
				</label>
			{/each}
		</div>

		<div class="time-row">
			<label for="kanban-schedule-time">{$i18n('kanban_schedule_time')}</label>
			<input
				id="kanban-schedule-time"
				class="form-input"
				type="time"
				value={timeValue}
				onchange={handleTimeChange}
			/>
		</div>
	{/if}
</fieldset>

<style>
	.kanban-schedule-form {
		border: 1px solid var(--color-border);
		border-radius: 6px;
		padding: 0.75rem;
		margin: 0;
	}
	.days {
		display: flex;
		gap: 0.3rem;
		flex-wrap: wrap;
		margin: 0.5rem 0;
	}
	.day-chip {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		padding: 0.25rem 0.5rem;
		border-radius: 999px;
		border: 1px solid var(--color-border);
		background: var(--color-surface);
		cursor: pointer;
		font-size: var(--font-size-xs);
	}
	.day-chip.active {
		background: var(--color-accent);
		color: var(--color-accent-text);
		border-color: var(--color-accent);
	}
	.day-chip input {
		display: none;
	}
	.time-row {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}
</style>
