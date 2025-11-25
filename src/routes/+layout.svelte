<script lang="ts">
	import { onMount } from 'svelte';
	import '../styles/global.css';
	import { theme } from '$lib/stores/theme';
	import { AppContainer, FloatingMenu } from '$lib/components/layout';

	let { children } = $props();

	onMount(() => {
		theme.init();
	});
</script>

<svelte:head>
	<link rel="preconnect" href="https://fonts.googleapis.com" />
	<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
	<link
		href="https://fonts.googleapis.com/css2?family=Signika:wght@400;500;600;700&family=JetBrains+Mono&display=swap"
		rel="stylesheet"
	/>
</svelte:head>

<AppContainer>
	<FloatingMenu />
	<div class="main-content">
		{@render children()}
	</div>
</AppContainer>
