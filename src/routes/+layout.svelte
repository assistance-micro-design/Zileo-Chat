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
	import { tauriListen, type TauriUnlistenFn } from '$lib/tauri';
	import '../styles/global.css';
	import { theme } from '$lib/stores/theme';
	import { localeStore } from '$lib/stores/locale';
	import { onboardingStore } from '$lib/stores/onboarding';
	import { i18n } from '$lib/i18n';
	import { AppContainer, FloatingMenu } from '$lib/components/layout';
	import { OnboardingModal } from '$lib/components/onboarding';
	import LegalModal from '$lib/components/legal/LegalModal.svelte';
	import ToastContainer from '$lib/components/ui/ToastContainer.svelte';
	import { sttSettingsStore } from '$lib/stores/sttSettings';

	let { children } = $props();

	let showOnboarding = $state(false);

	// Legal modal state
	let legalModalOpen = $state(false);
	let legalModalType = $state<'legal-notice' | 'privacy-policy'>('legal-notice');
	let unlistenLegal: TauriUnlistenFn | null = null;
	let unlistenPrivacy: TauriUnlistenFn | null = null;

	onMount(async () => {
		theme.init();
		localeStore.init();

		// Load STT settings — FAB visibility depends on `enabled` flag.
		void sttSettingsStore.loadSettings().catch(() => {
			/* defaults are surfaced by the store on failure */
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
	});

	onDestroy(() => {
		unlistenLegal?.();
		unlistenPrivacy?.();
	});

	function handleOnboardingComplete(): void {
		showOnboarding = false;
	}
</script>

<svelte:head>
	<!-- Fonts self-hosted in /static/fonts/ (no external CDN dependency) -->
</svelte:head>

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
