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

<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		tauriListen,
		tauriInvoke,
		getAppVersion,
		isTauriRuntime,
		type TauriUnlistenFn
	} from '$lib/tauri';
	import '../styles/global.css';
	import { theme } from '$lib/stores/theme';
	import { localeStore } from '$lib/stores/locale';
	import { onboardingStore } from '$lib/stores/onboarding';
	import { i18n } from '$lib/i18n';
	import { AppContainer, FloatingMenu } from '$lib/components/layout';
	import { OnboardingModal } from '$lib/components/onboarding';
	import LegalModal from '$lib/components/legal/LegalModal.svelte';
	import ToastContainer from '$lib/components/ui/ToastContainer.svelte';
	import SplashScreen from '$lib/components/SplashScreen.svelte';
	import { UserQuestionModal, GlobalValidationModal } from '$lib/components/workflow';
	import KanbanCardCreator from '$lib/components/kanban/KanbanCardCreator.svelte';
	import { sttSettingsStore } from '$lib/stores/sttSettings';
	import { validationSettingsStore } from '$lib/stores/validation-settings';
	import { kanbanRuntimeStore } from '$lib/stores/kanban-runtime';
	import { backgroundWorkflowsStore } from '$lib/stores/background-workflows';
	import { kanbanEventsStore } from '$lib/stores/kanban-events';
	import { composingStore } from '$lib/stores/kanban-compose';
	import { cardCreatorStore, cardCreatorOpen } from '$lib/stores/card-creator';
	import { kanbanStore } from '$lib/stores/kanban';
	import { kanbanScheduleStore } from '$lib/stores/kanban-schedule';
	import { agents as agentsStore, agentStore } from '$lib/stores/agents';
	import { prompts as promptsStore, promptStore } from '$lib/stores/prompts';
	import { folders as foldersStore, folderStore } from '$lib/stores/folders';
	import { toastStore } from '$lib/stores/toast';
	import type { KanbanCardCreate, KanbanScheduleCreate } from '$types/kanban';

	let { children } = $props();

	// Lazy-load the creator's option lists when the global modal opens — these
	// stores are NOT populated at boot (only /kanban and /settings load them),
	// so opening "Nouvelle tâche à faire" from /agent would otherwise show empty
	// selects. The loaders are idempotent (no concurrent double-fetch).
	$effect(() => {
		if ($cardCreatorOpen) {
			void agentStore.loadAgents();
			void promptStore.loadPrompts();
			void folderStore.loadFolders();
		}
	});

	/**
	 * Persist a card created from the global creator, then land the user on
	 * /kanban. Mirrors the page's former `createCard` (manual mode may attach a
	 * recurrence). The shared `kanbanStore` is reloaded so an already-mounted
	 * /kanban reflects the new card without waiting for a navigation.
	 */
	async function handleGlobalCardCreated(
		payload: KanbanCardCreate,
		schedule?: Omit<KanbanScheduleCreate, 'card_template_id'>
	): Promise<void> {
		const created = await kanbanStore.createCard(payload);
		if (schedule) {
			const cardTemplateId = typeof created === 'string' ? created : '';
			if (cardTemplateId) {
				await kanbanScheduleStore.createSchedule({
					card_template_id: cardTemplateId,
					days_of_week: schedule.days_of_week,
					hour: schedule.hour,
					minute: schedule.minute
				});
			}
		}
		await kanbanStore.loadCards();
		cardCreatorStore.close();
		await goto('/kanban');
	}

	let showOnboarding = $state(false);

	// Legal modal state
	let legalModalOpen = $state(false);
	let legalModalType = $state<'legal-notice' | 'privacy-policy'>('legal-notice');
	let unlistenLegal: TauriUnlistenFn | null = null;
	let unlistenPrivacy: TauriUnlistenFn | null = null;
	let unlistenManagerWrite: TauriUnlistenFn | null = null;

	/**
	 * Payload of `manager_write_notice` — a *Manager self-improvement write that
	 * executed without a human review (Auto/permissive mode). Surfaced as an
	 * opportunistic toast; the durable record is the PreApproved audit entry.
	 */
	interface ManagerWriteNoticeEvent {
		workflow_id: string;
		tool_name: string;
		operation: string;
		risk_level: string;
	}

	// Startup splash. The window now appears before the backend finishes its
	// deferred boot init, so this overlay covers the app — blocking interaction —
	// until the UI-critical services (providers, embedding) are ready; MCP keeps
	// connecting in the background. Outside the Tauri runtime there is no backend
	// to signal readiness, so it never shows.
	let booting = $state(isTauriRuntime());
	let appVersion = $state('');
	let unlistenBootReady: TauriUnlistenFn | null = null;
	let bootSafetyTimer: ReturnType<typeof setTimeout> | null = null;
	let bootMinTimer: ReturnType<typeof setTimeout> | null = null;

	// Fail-safe: never trap the user on the splash if `boot_ready` is missed.
	const BOOT_SPLASH_TIMEOUT_MS = 20000;
	// Keep the splash up at least this long so the reveal feels intentional and
	// the loading ticker is visible, even when the backend is ready almost
	// instantly (provider/embedding init is sub-second).
	const BOOT_SPLASH_MIN_MS = 900;
	const splashStart = Date.now();

	/** Dismiss the splash, but not before the minimum display time has elapsed. */
	function requestDismiss(): void {
		if (!booting || bootMinTimer) {
			return;
		}
		const remaining = BOOT_SPLASH_MIN_MS - (Date.now() - splashStart);
		if (remaining <= 0) {
			finishDismiss();
		} else {
			bootMinTimer = setTimeout(finishDismiss, remaining);
		}
	}

	function finishDismiss(): void {
		booting = false;
		if (bootSafetyTimer) {
			clearTimeout(bootSafetyTimer);
			bootSafetyTimer = null;
		}
		if (bootMinTimer) {
			clearTimeout(bootMinTimer);
			bootMinTimer = null;
		}
	}

	onMount(async () => {
		// Wire the splash first so the ready signal is not missed. The window can
		// come up after the backend already reported readiness, so also query the
		// current state to cover that race.
		if (isTauriRuntime()) {
			void getAppVersion()
				.then((version) => {
					appVersion = version;
				})
				.catch(() => {
					/* version is decorative; ignore failures */
				});

			unlistenBootReady = await tauriListen('boot_ready', () => {
				requestDismiss();
			});

			try {
				if (await tauriInvoke<boolean>('boot_ready_state')) {
					requestDismiss();
				}
			} catch {
				/* the event or the safety timer will dismiss the splash */
			}

			bootSafetyTimer = setTimeout(finishDismiss, BOOT_SPLASH_TIMEOUT_MS);
		}

		theme.init();
		localeStore.init();

		// Initialise the background workflows store at the app root so its
		// global Tauri listeners for `workflow_stream` / `workflow_complete`
		// stay attached across page navigations. Without this, a workflow
		// started from /kanban that the user navigates away from would lose
		// its streaming chunks until /agent re-attached the listener.
		void backgroundWorkflowsStore.init().catch(() => {
			/* listener failures are logged inside the store */
		});

		// Initialise the Kanban events store at the app root for the same
		// reason: its analyze-lifecycle listeners must stay attached across
		// navigations so a verdict that arrives while the user is away from
		// /kanban still refreshes the board and pre-opens the improve modal.
		void kanbanEventsStore.init().catch(() => {
			/* listener failures are non-fatal; the board self-heals on remount */
		});

		// Initialise the async-compose store at the app root so a "Générer
		// l'aperçu" launched from the global creator still toasts and refreshes
		// the proposed zone if it finishes while the user is on another route.
		void composingStore.init().catch(() => {
			/* listener failures are non-fatal */
		});

		// Load STT settings — FAB visibility depends on `enabled` flag.
		void sttSettingsStore.loadSettings().catch(() => {
			/* defaults are surfaced by the store on failure */
		});

		// Load validation settings at the root so the concurrency gate
		// (`backgroundWorkflowsStore.canStart`) reads the real persisted mode on
		// EVERY route — not just /agent and /settings. Without this, arriving on
		// or reloading /kanban left the derived `settings` store null, so an
		// `auto`-mode user was wrongly throttled to the non-auto cap (1) and a
		// second validated card stalled. Idempotent: /agent re-fetches silently.
		void validationSettingsStore.loadSettings().catch(() => {
			// Fail-safe: the gate falls back to the conservative cap (1, never
			// over-concurrency) until a later load succeeds, and the failure is not
			// lost — loadSettings records it in the store's `error` state, surfaced
			// on the Validation settings page for diagnosis (a /agent or /settings
			// visit re-fetches). No console here (project bans it in production).
		});

		// Load the backend worker-concurrency cap once so the Kanban board's
		// "X / N actifs" badge reflects the SAME value the scheduler promotes
		// with (DEFAULT_MAX_CONCURRENT_WORKFLOWS) instead of a recopied literal.
		void kanbanRuntimeStore.load().catch(() => {
			/* badge degrades to a placeholder until a later load succeeds */
		});

		// Check if onboarding should be shown (first launch)
		showOnboarding = onboardingStore.shouldShow();

		// Listen for legal menu events from Tauri
		unlistenLegal = await tauriListen('open-legal-notice', () => {
			legalModalType = 'legal-notice';
			legalModalOpen = true;
		});

		unlistenPrivacy = await tauriListen('open-privacy-policy', () => {
			legalModalType = 'privacy-policy';
			legalModalOpen = true;
		});

		// Opportunistic toast when an agent rewrote a prompt/skill (or granted a
		// skill) without review under a permissive validation mode. The audit log
		// is the durable record — this is just a live heads-up so the auto-write
		// is not silent. Attached at the root so it shows on every route.
		unlistenManagerWrite = await tauriListen<ManagerWriteNoticeEvent>(
			'manager_write_notice',
			(event) => {
				const { tool_name, operation } = event.payload;
				toastStore.add({
					type: 'warning',
					title: $i18n('toast_manager_write_title'),
					message: $i18n('toast_manager_write_message')
						.replace('{tool}', tool_name)
						.replace('{operation}', operation),
					persistent: false,
					duration: 8000
				});
			}
		);
	});

	onDestroy(() => {
		unlistenLegal?.();
		unlistenPrivacy?.();
		unlistenManagerWrite?.();
		unlistenBootReady?.();
		if (bootSafetyTimer) {
			clearTimeout(bootSafetyTimer);
		}
		if (bootMinTimer) {
			clearTimeout(bootMinTimer);
		}
		backgroundWorkflowsStore.destroy();
		kanbanEventsStore.destroy();
		composingStore.destroy();
	});

	function handleOnboardingComplete(): void {
		showOnboarding = false;
	}
</script>

<svelte:head>
	<!-- Fonts self-hosted in /static/fonts/ (no external CDN dependency) -->
</svelte:head>

{#if booting}
	<SplashScreen version={appVersion} />
{/if}

{#if showOnboarding}
	<OnboardingModal onComplete={handleOnboardingComplete} />
{:else}
	<a href="#main-content" class="skip-link">{$i18n('nav_skip_to_content')}</a>
	<AppContainer>
		<FloatingMenu />
		<div id="main-content" class="main-content" role="main">
			{@render children()}
		</div>
	</AppContainer>
{/if}

<!-- Toast notifications (global, visible on all pages) -->
<ToastContainer />

<!-- Legal modals accessible from Tauri Help menu -->
<LegalModal type={legalModalType} open={legalModalOpen} onclose={() => (legalModalOpen = false)} />

<!--
  User question modal (global, K3): mounted at the root so a question raised by
  ANY workflow — including a Kanban worker running while the user is on /kanban
  (which does not host its own modal) — surfaces on every route. Driven entirely
  by the workflow-aware userQuestionStore; no page context required.
-->
<UserQuestionModal />

<!--
  Human-in-the-loop validation modal (global): same rationale as above. An
  ATTENDED workflow (e.g. a Kanban card supervisor chat) that requests a tool
  validation must surface its prompt on every route, not only /agent — otherwise
  the backend poll times out and applies the default (reject) silently. Owns the
  validationStore lifecycle; /agent no longer hosts its own validation branch.
-->
<GlobalValidationModal />

<!--
  Global "new task" creator (DP-5): a SINGLE instance mounted at the root so the
  FloatingMenu's "Nouvelle tâche à faire" button can open it from any route. The
  /kanban page no longer hosts its own creator (two instances would diverge in
  form state). Option lists are loaded lazily on open (see the $effect above).
-->
<KanbanCardCreator
	open={$cardCreatorOpen}
	agents={$agentsStore}
	prompts={$promptsStore}
	folders={$foldersStore}
	onclose={() => cardCreatorStore.close()}
	oncreated={handleGlobalCardCreated}
/>
