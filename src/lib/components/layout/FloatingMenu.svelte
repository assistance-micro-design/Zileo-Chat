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
  FloatingMenu Component
  A floating, detached dock pill fixed near the top of the viewport, with
  translucent blurred background (styles in global.css). Brand mark on the
  left, primary action + segmented navigation in the center, language and
  theme controls on the right.

  Layout: Brand + dictation (left) | New task + segmented nav (center) | Language/Theme (right)

  @example
  <FloatingMenu />
-->
<script lang="ts">
	import { Sun, Moon, Settings, Bot, KanbanSquare, Plus, Minus } from '@lucide/svelte';
	import { page } from '$app/state';
	import { theme } from '$lib/stores/theme';
	import { uiZoom, formatZoomPercent, MIN_ZOOM, MAX_ZOOM } from '$lib/stores/ui-zoom';
	import { i18n } from '$lib/i18n';
	import { cardCreatorStore } from '$lib/stores/card-creator';
	import LanguageSelector from '$lib/components/ui/LanguageSelector.svelte';
	import MicButton from '$lib/components/ui/MicButton.svelte';

	/**
	 * FloatingMenu props
	 */
	interface Props {
		/** Application title */
		title?: string;
	}

	let { title = 'Zileo Chat' }: Props = $props();

	/** Whether the current route is inside the settings area. */
	const isSettings = $derived(page.url.pathname.startsWith('/settings'));
	/** Whether the current route is the agent workspace. */
	const isAgent = $derived(page.url.pathname.startsWith('/agent'));
	/** Whether the current route is the Kanban board. */
	const isKanban = $derived(page.url.pathname.startsWith('/kanban'));

	/** Current zoom rendered as a whole-percentage label (e.g. "130 %"). */
	const zoomLabel = $derived(formatZoomPercent($uiZoom));
	/** Disable the controls once a bound is reached so the UI can't break. */
	const canZoomOut = $derived($uiZoom > MIN_ZOOM);
	const canZoomIn = $derived($uiZoom < MAX_ZOOM);

	/**
	 * Toggle theme between light and dark
	 */
	function toggleTheme(): void {
		theme.toggle();
	}
</script>

<header class="floating-menu">
	<!-- Left: Brand mark + dictation FAB -->
	<div class="menu-brand">
		<span class="brand-dot" aria-hidden="true"></span>
		<h1>{title}</h1>
	</div>
	<MicButton />

	<!-- Center: primary action + segmented navigation -->
	<div class="menu-center">
		<button type="button" class="btn btn-primary" onclick={() => cardCreatorStore.open()}>
			<Plus size={16} />
			<span class="floating-menu-link-text">{$i18n('kanban_new_card')}</span>
		</button>

		<nav class="nav-seg" aria-label={$i18n('layout_main_navigation')}>
			<a href="/kanban" class:active={isKanban} aria-current={isKanban ? 'page' : undefined}>
				<KanbanSquare size={16} />
				<span class="floating-menu-link-text">{$i18n('layout_kanban')}</span>
			</a>

			<a
				href="/agent"
				class="seg-agent"
				class:active={isAgent}
				aria-current={isAgent ? 'page' : undefined}
			>
				<Bot size={16} />
				<span class="floating-menu-link-text">{$i18n('layout_agent')}</span>
			</a>

			<a href="/settings" class:active={isSettings} aria-current={isSettings ? 'page' : undefined}>
				<Settings size={16} />
				<span class="floating-menu-link-text">{$i18n('layout_configuration')}</span>
			</a>
		</nav>
	</div>

	<!-- Right: Zoom, Language & Theme -->
	<div class="menu-right">
		<!--
		  Zoom control (native webview zoom). The percentage is reactive (driven by
		  the uiZoom store), so it tracks both these buttons and the global
		  Ctrl +/-/0 keyboard shortcuts. Shortcuts are exposed to assistive tech via
		  aria-keyshortcuts on each control; the hover/focus infobox is the sighted
		  equivalent (aria-hidden, no duplicate announcement). Clicking the
		  percentage resets to 100 %.
		-->
		<div class="zoom-control" role="group" aria-label={$i18n('layout_zoom')}>
			<button
				type="button"
				class="btn btn-ghost btn-icon zoom-btn"
				onclick={() => uiZoom.decrease()}
				disabled={!canZoomOut}
				aria-label={$i18n('layout_zoom_out')}
				aria-keyshortcuts="Control+- Meta+-"
			>
				<Minus size={15} />
			</button>

			<button
				type="button"
				class="zoom-value"
				onclick={() => uiZoom.reset()}
				aria-label={$i18n('layout_zoom_reset')}
				aria-keyshortcuts="Control+0 Meta+0"
			>
				{zoomLabel}
			</button>

			<button
				type="button"
				class="btn btn-ghost btn-icon zoom-btn"
				onclick={() => uiZoom.increase()}
				disabled={!canZoomIn}
				aria-label={$i18n('layout_zoom_in')}
				aria-keyshortcuts="Control+= Meta+="
			>
				<Plus size={15} />
			</button>

			<div class="zoom-tip" aria-hidden="true">
				<span class="zoom-tip-row"><span>{$i18n('layout_zoom_in')}</span><kbd>Ctrl +</kbd></span>
				<span class="zoom-tip-row"><span>{$i18n('layout_zoom_out')}</span><kbd>Ctrl -</kbd></span>
				<span class="zoom-tip-row"><span>{$i18n('layout_zoom_reset')}</span><kbd>Ctrl 0</kbd></span>
			</div>
		</div>

		<LanguageSelector />

		<button
			type="button"
			class="btn btn-ghost btn-icon"
			onclick={toggleTheme}
			aria-label={$theme === 'light'
				? $i18n('layout_switch_to_dark_mode')
				: $i18n('layout_switch_to_light_mode')}
		>
			{#if $theme === 'light'}
				<Moon size={18} />
			{:else}
				<Sun size={18} />
			{/if}
		</button>
	</div>
</header>

<style>
	.floating-menu-link-text {
		display: inline;
	}

	@media (max-width: 640px) {
		.floating-menu-link-text {
			display: none;
		}
	}

	/* Zoom control: a compact segmented pill (− value +) with a hover/focus
	   infobox listing the keyboard shortcuts. */
	.zoom-control {
		position: relative;
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2xs);
		padding: 2px;
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-full);
		background: var(--surface-2);
	}

	.zoom-btn {
		padding: var(--spacing-2xs);
		min-height: 0;
	}

	.zoom-value {
		min-width: 3.25rem;
		padding: 0 var(--spacing-2xs);
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		font-family: var(--font-family);
		font-size: var(--font-size-xs);
		font-variant-numeric: tabular-nums;
		text-align: center;
		cursor: pointer;
		border-radius: var(--border-radius-sm);
		transition: background 0.15s ease;
	}

	.zoom-value:hover {
		color: var(--color-text-primary);
		background: var(--color-bg-hover);
	}

	.zoom-tip {
		position: absolute;
		top: calc(100% + 10px);
		right: 0;
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2xs);
		min-width: 12rem;
		padding: var(--spacing-sm) var(--spacing-md);
		background: var(--surface-overlay);
		backdrop-filter: blur(14px);
		-webkit-backdrop-filter: blur(14px);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		box-shadow: var(--shadow-lg);
		z-index: var(--z-index-tooltip);
		opacity: 0;
		visibility: hidden;
		transform: translateY(-4px);
		transition:
			opacity 0.15s ease,
			transform 0.15s ease;
		pointer-events: none;
	}

	.zoom-control:hover .zoom-tip,
	.zoom-control:focus-within .zoom-tip {
		opacity: 1;
		visibility: visible;
		transform: translateY(0);
	}

	.zoom-tip-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-lg);
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.zoom-tip kbd {
		font-family: var(--font-mono);
		font-size: var(--font-size-2xs);
		color: var(--color-text-tertiary);
		background: var(--color-bg-tertiary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-sm);
		padding: 1px 6px;
	}

	@media (prefers-reduced-motion: reduce) {
		.zoom-tip {
			transition: none;
			transform: none;
		}

		.zoom-control:hover .zoom-tip,
		.zoom-control:focus-within .zoom-tip {
			transform: none;
		}
	}

	@media (max-width: 640px) {
		.zoom-control {
			display: none;
		}
	}
</style>
