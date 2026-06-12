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
		<div class="splash-mark" aria-hidden="true"></div>
		<h1 class="splash-title">{$i18n('splash_app_name')}</h1>
		<Spinner size="md" label={$i18n('splash_step_loading')} />

		<div class="splash-ticker" aria-hidden="true">
			<div class="splash-ticker-track">
				{#each TICKER_KEYS as key (key)}
					<span class="splash-ticker-item"><span class="tick-dot"></span>{$i18n(key)}</span>
				{/each}
				{#each TICKER_KEYS as key (key)}
					<span class="splash-ticker-item"><span class="tick-dot"></span>{$i18n(key)}</span>
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
		/* Soft turquoise veil radiating from the brand mark, like the site */
		background:
			radial-gradient(620px 360px at 50% 38%, var(--color-accent-light), transparent 70%),
			linear-gradient(180deg, rgba(148, 239, 238, 0.07), transparent 50%), var(--color-bg-primary);
		color: var(--color-text-primary);
		font-family: var(--font-family);
	}

	:global([data-theme='dark']) .splash {
		background:
			radial-gradient(620px 360px at 50% 38%, rgba(148, 239, 238, 0.1), transparent 70%),
			linear-gradient(145deg, var(--color-bg-tertiary), var(--color-bg-primary));
	}

	/* Brand mark with a breathing glow */
	.splash-mark {
		width: 84px;
		height: 84px;
		border-radius: 24px;
		background: var(--gradient-brand);
		box-shadow: var(--glow-accent);
		animation: breathe 2.6s ease-in-out infinite;
	}

	@keyframes breathe {
		0%,
		100% {
			box-shadow:
				0 0 0 1px rgba(148, 239, 238, 0.4),
				0 10px 30px rgba(148, 239, 238, 0.2);
			transform: scale(1);
		}
		50% {
			box-shadow:
				0 0 0 1px rgba(148, 239, 238, 0.55),
				0 14px 44px rgba(148, 239, 238, 0.38);
			transform: scale(1.03);
		}
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
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-sm);
		margin-bottom: 0.5em;
		font-size: var(--font-size-sm);
		line-height: 1.4;
		color: var(--color-text-tertiary);
		font-variant-numeric: tabular-nums;
	}

	.tick-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--color-accent-hover);
		box-shadow: 0 0 6px var(--color-accent-hover);
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
		color: var(--color-text-tertiary);
		text-decoration: none;
		transition: color var(--transition-fast);
	}

	.splash-site:hover {
		color: var(--color-accent-deep);
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

		.splash-mark {
			animation: none;
		}
	}
</style>
