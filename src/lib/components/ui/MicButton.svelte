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
  Push-to-talk floating action button (FAB).
  Reads sttStore + sttSettings — no props.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { Mic, MicOff } from '@lucide/svelte';
	import { sttStore } from '$lib/stores/sttStore.svelte';
	import { sttSettings } from '$lib/stores/sttSettings';
	import { i18n } from '$lib/i18n';

	let focusedEditable = $state(false);

	function isEditableTarget(el: EventTarget | null): boolean {
		if (!(el instanceof HTMLElement)) return false;
		if (el instanceof HTMLTextAreaElement) return true;
		if (el instanceof HTMLInputElement) {
			const t = el.type.toLowerCase();
			return (
				t === 'text' || t === 'search' || t === 'url' || t === 'tel' || t === 'email' || t === ''
			);
		}
		return false;
	}

	/**
	 * Push-to-talk hotkey: hold Ctrl+Shift+Space to record, release to
	 * transcribe. Matches the mouse FAB behavior (auto-repeat ignored).
	 */
	function isShortcut(ev: KeyboardEvent): boolean {
		return ev.ctrlKey && ev.shiftKey && (ev.code === 'Space' || ev.key === ' ');
	}

	function handleHotkeyDown(ev: KeyboardEvent) {
		if (!isShortcut(ev) || ev.repeat) return;
		if (sttStore.phase === 'recording' || sttStore.phase === 'transcribing') return;
		if (!isEditableTarget(document.activeElement)) return;
		ev.preventDefault();
		if (!sttStore.attachToActive()) return;
		void sttStore.startRecording();
	}

	function handleHotkeyUp(ev: KeyboardEvent) {
		// Trigger on any key-up while we're recording — releasing Ctrl/Shift
		// before Space (or any combination) ends the dictation safely.
		if (sttStore.phase === 'recording') {
			if (ev.code === 'Space' || ev.key === ' ' || ev.key === 'Control' || ev.key === 'Shift') {
				ev.preventDefault();
				void sttStore.stopAndTranscribe();
			}
		} else if (sttStore.phase === 'armed' && isShortcut(ev)) {
			sttStore.detach();
		}
	}

	onMount(() => {
		const update = () => {
			focusedEditable = isEditableTarget(document.activeElement);
		};
		document.addEventListener('focusin', update);
		document.addEventListener('focusout', update);
		document.addEventListener('keydown', handleHotkeyDown);
		document.addEventListener('keyup', handleHotkeyUp);
		update();
		return () => {
			document.removeEventListener('focusin', update);
			document.removeEventListener('focusout', update);
			document.removeEventListener('keydown', handleHotkeyDown);
			document.removeEventListener('keyup', handleHotkeyUp);
		};
	});

	let phase = $derived(sttStore.phase);
	let isRecording = $derived(phase === 'recording');
	let isTranscribing = $derived(phase === 'transcribing');
	let canArm = $derived(focusedEditable && phase !== 'transcribing');

	function handlePointerDown(ev: PointerEvent) {
		if (!canArm) return;
		ev.preventDefault();
		if (!sttStore.attachToActive()) return;
		void sttStore.startRecording();
		(ev.currentTarget as HTMLElement).setPointerCapture?.(ev.pointerId);
	}

	function handlePointerUp() {
		if (sttStore.phase === 'recording') {
			void sttStore.stopAndTranscribe();
		} else if (sttStore.phase === 'armed') {
			sttStore.detach();
		}
	}

	function handlePointerCancel() {
		sttStore.cancel();
	}

	let titleKey = $derived(
		isRecording
			? 'stt.fab_recording'
			: isTranscribing
				? 'stt.fab_transcribing'
				: focusedEditable
					? 'stt.fab_ready'
					: 'stt.fab_focus_required'
	);
</script>

{#if $sttSettings?.enabled}
	<button
		type="button"
		class="mic-fab"
		class:active={focusedEditable}
		class:recording={isRecording}
		class:transcribing={isTranscribing}
		disabled={isTranscribing || !focusedEditable}
		title={$i18n(titleKey)}
		aria-label={$i18n(titleKey)}
		aria-pressed={isRecording}
		onpointerdown={handlePointerDown}
		onpointerup={handlePointerUp}
		onpointercancel={handlePointerCancel}
		onpointerleave={handlePointerUp}
	>
		{#if phase === 'error'}
			<MicOff size={16} aria-hidden="true" />
		{:else}
			<Mic size={16} aria-hidden="true" />
		{/if}
		{#if isRecording}
			<span class="rec-badge">REC</span>
		{/if}
		{#if isTranscribing}
			<span class="spinner" aria-hidden="true"></span>
		{/if}
	</button>
{/if}

<style>
	.mic-fab {
		position: relative;
		width: 36px;
		height: 36px;
		border-radius: 50%;
		border: 1px solid var(--color-border);
		background: var(--color-bg-elevated);
		color: var(--color-text-secondary);
		display: inline-flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		opacity: 0.6;
		flex-shrink: 0;
		transition:
			opacity var(--transition-fast),
			transform var(--transition-fast),
			background-color var(--transition-fast),
			color var(--transition-fast),
			border-color var(--transition-fast);
	}

	.mic-fab.active {
		opacity: 1;
		color: var(--color-accent);
		border-color: var(--color-accent);
	}

	.mic-fab.active:hover {
		background: var(--color-accent-light);
		transform: translateY(-1px);
	}

	.mic-fab.recording {
		background: var(--color-error, #dc2626);
		color: #fff;
		border-color: var(--color-error, #dc2626);
		opacity: 1;
		animation: pulse 1.2s ease-in-out infinite;
	}

	.mic-fab.transcribing {
		background: var(--color-accent-light);
		color: var(--color-accent);
		cursor: progress;
		opacity: 1;
	}

	.mic-fab:disabled {
		cursor: not-allowed;
	}

	.rec-badge {
		position: absolute;
		top: -8px;
		right: -10px;
		background: #fff;
		color: var(--color-error, #dc2626);
		font-size: 10px;
		font-weight: var(--font-weight-bold, 700);
		padding: 2px 5px;
		border-radius: 4px;
		letter-spacing: 0.5px;
	}

	.spinner {
		position: absolute;
		inset: 4px;
		border: 2px solid transparent;
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes pulse {
		0%,
		100% {
			box-shadow: 0 0 0 0 rgba(220, 38, 38, 0.6);
		}
		50% {
			box-shadow: 0 0 0 10px rgba(220, 38, 38, 0);
		}
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
