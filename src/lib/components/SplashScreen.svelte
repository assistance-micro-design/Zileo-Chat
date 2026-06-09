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
  Startup splash screen.

  A full-screen overlay shown while the deferred boot init runs. It appears
  immediately with the window so the user sees the app is starting instead of a
  blank screen, and covers the app to block interaction until the backend
  reports the UI-critical services are ready. On dismissal it fades out,
  revealing the app underneath. Layout: app name centered with a spinner and a
  vertical ticker of the subsystems coming online, version bottom-right, and the
  publisher site link bottom-left.
-->
<script lang="ts">
	import { fade } from 'svelte/transition';
	import { motionDuration } from '$lib/utils/motion';
	import { i18n } from '$lib/i18n';
	import { openExternalUrl } from '$lib/tauri';
	import Spinner from '$lib/components/ui/Spinner.svelte';

	interface Props {
		/** Application version (e.g. `"0.26.0"`); hidden when empty. */
		version: string;
	}

	let { version }: Props = $props();

	const SITE_URL = 'https://assistance-micro-design.fr';
	const SITE_DISPLAY = 'assistance-micro-design.fr';

	// Decorative, looping list of the subsystems coming online during boot.
	const TICKER_KEYS = [
		'splash_item_mcp',
		'splash_item_providers',
		'splash_item_embedding',
		'splash_item_models',
		'splash_item_memory',
		'splash_item_agents'
	];

	/**
	 * Opens the publisher site through Tauri's opener. Best-effort: a failure
	 * (e.g. outside the Tauri runtime) is swallowed — the splash is transient.
	 */
	async function openSite(): Promise<void> {
		try {
			await openExternalUrl(SITE_URL);
		} catch {
			/* opener unavailable: no-op */
		}
	}
</script>

<div
	class="splash"
	role="status"
	aria-live="polite"
	aria-busy="true"
	out:fade={{ duration: motionDuration(350) }}
>
	<div class="splash-center">
		<h1 class="splash-title">{$i18n('splash_app_name')}</h1>
		<Spinner size="md" label={$i18n('splash_step_loading')} />

		<div class="splash-ticker" aria-hidden="true">
			<div class="splash-ticker-track">
				{#each TICKER_KEYS as key (key)}
					<span class="splash-ticker-item">{$i18n(key)}</span>
				{/each}
				{#each TICKER_KEYS as key (key)}
					<span class="splash-ticker-item">{$i18n(key)}</span>
				{/each}
			</div>
		</div>
	</div>

	<a
		class="splash-site"
		href={SITE_URL}
		onclick={(event) => {
			event.preventDefault();
			void openSite();
		}}
	>
		{SITE_DISPLAY}
	</a>

	{#if version}
		<span class="splash-version">v{version}</span>
	{/if}
</div>

<style>
	.splash {
		position: fixed;
		inset: 0;
		z-index: 9999;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-family);
	}

	.splash-center {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1.25rem;
		padding: 1rem;
		text-align: center;
	}

	.splash-title {
		margin: 0;
		font-size: var(--font-size-2xl);
		font-weight: var(--font-weight-bold);
		letter-spacing: 0.02em;
	}

	/* Masked viewport showing ~3 ticker rows, edges faded for a marquee feel. */
	.splash-ticker {
		height: 4.5em;
		overflow: hidden;
		-webkit-mask-image: linear-gradient(to bottom, transparent, black 30%, black 70%, transparent);
		mask-image: linear-gradient(to bottom, transparent, black 30%, black 70%, transparent);
	}

	.splash-ticker-track {
		display: flex;
		flex-direction: column;
		align-items: center;
		/* The list is rendered twice; translating up by exactly half its height
		   loops seamlessly. Each item owns its trailing gap via margin (not flex
		   gap) so the two halves are pixel-identical. */
		animation: splash-ticker-scroll 7s linear infinite;
	}

	.splash-ticker-item {
		margin-bottom: 0.5em;
		font-size: var(--font-size-sm);
		line-height: 1.4;
		color: var(--color-text-tertiary);
	}

	@keyframes splash-ticker-scroll {
		from {
			transform: translateY(0);
		}
		to {
			transform: translateY(-50%);
		}
	}

	.splash-site {
		position: absolute;
		left: 1.25rem;
		bottom: 1rem;
		font-size: var(--font-size-xs);
		color: var(--color-accent);
		text-decoration: none;
	}

	.splash-site:hover {
		color: var(--color-accent-hover);
		text-decoration: underline;
	}

	.splash-version {
		position: absolute;
		right: 1.25rem;
		bottom: 1rem;
		font-size: var(--font-size-xs);
		font-family: var(--font-mono);
		color: var(--color-text-tertiary);
	}

	@media (prefers-reduced-motion: reduce) {
		.splash-ticker-track {
			animation: none;
		}
	}
</style>
