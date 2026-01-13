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
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import '../styles/global.css';
	import { theme } from '$lib/stores/theme';
	import { localeStore } from '$lib/stores/locale';
	import { onboardingStore } from '$lib/stores/onboarding';
	import { i18n } from '$lib/i18n';
	import { AppContainer, FloatingMenu } from '$lib/components/layout';
	import { OnboardingModal } from '$lib/components/onboarding';
	import LegalModal from '$lib/components/legal/LegalModal.svelte';

	let { children } = $props();

	let showOnboarding = $state(false);

	// Legal modal state
	let legalModalOpen = $state(false);
	let legalModalType = $state<'legal-notice' | 'privacy-policy'>('legal-notice');
	let unlistenLegal: UnlistenFn | null = null;
	let unlistenPrivacy: UnlistenFn | null = null;

	onMount(async () => {
		theme.init();
		localeStore.init();

		// Check if onboarding should be shown (first launch)
		showOnboarding = onboardingStore.shouldShow();

		// Listen for legal menu events from Tauri
		unlistenLegal = await listen('open-legal-notice', () => {
			legalModalType = 'legal-notice';
			legalModalOpen = true;
		});

		unlistenPrivacy = await listen('open-privacy-policy', () => {
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
	<link rel="preconnect" href="https://fonts.googleapis.com" />
	<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
	<link
		href="https://fonts.googleapis.com/css2?family=Signika:wght@400;500;600;700&family=JetBrains+Mono&display=swap"
		rel="stylesheet"
	/>
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

<!-- Legal modals accessible from Tauri Help menu -->
<LegalModal type={legalModalType} open={legalModalOpen} onclose={() => (legalModalOpen = false)} />
