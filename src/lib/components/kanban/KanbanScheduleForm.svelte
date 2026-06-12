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
			<label class="time-label" for="kanban-schedule-time">{$i18n('kanban_schedule_time')}</label>
			<input
				id="kanban-schedule-time"
				class="form-input time-input"
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
		border-radius: var(--border-radius-md);
		padding: var(--spacing-md);
		margin: 0;
	}
	.kanban-schedule-form legend {
		padding: 0 6px;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
	}
	.kanban-schedule-form legend label {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		cursor: pointer;
	}
	.days {
		display: flex;
		gap: 0.25rem;
		flex-wrap: wrap;
		margin: 0.5rem 0;
	}
	/* Day chips follow the mock's chip recipe: outline at rest, translucent
	   brand surface + deep-accent ink when active (not the solid accent fill,
	   whose contrast fails for unselected-vs-selected scanning). */
	.day-chip {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		padding: 0.22rem 0.7rem;
		border-radius: var(--border-radius-full);
		border: 1px solid var(--color-border);
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		font-size: var(--font-size-xs);
		font-weight: var(--font-weight-medium);
		transition:
			border-color var(--transition-fast),
			color var(--transition-fast),
			background-color var(--transition-fast);
	}
	.day-chip:hover {
		border-color: var(--color-border-dark);
		color: var(--color-text-primary);
	}
	.day-chip.active {
		background: var(--color-accent-light);
		color: var(--color-accent-deep);
		border-color: var(--color-accent-deep);
	}
	.day-chip input {
		display: none;
	}
	.time-row {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		align-items: flex-start;
	}
	.time-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
	}
	.time-input {
		width: 140px;
	}
</style>
