<script lang="ts">
	import { onMount } from 'svelte';
	import '../styles/global.css';
	import { theme } from '$lib/stores/theme';
	import { localeStore } from '$lib/stores/locale';
	import { onboardingStore } from '$lib/stores/onboarding';
	import { i18n } from '$lib/i18n';
	import { AppContainer, FloatingMenu } from '$lib/components/layout';
	import { OnboardingModal } from '$lib/components/onboarding';

	let { children } = $props();

	let showOnboarding = $state(false);

	onMount(() => {
		theme.init();
		localeStore.init();

		// Check if onboarding should be shown (first launch)
		showOnboarding = onboardingStore.shouldShow();
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
