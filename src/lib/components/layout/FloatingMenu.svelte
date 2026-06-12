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
	import { Sun, Moon, Settings, Bot, KanbanSquare, Plus } from '@lucide/svelte';
	import { page } from '$app/state';
	import { theme } from '$lib/stores/theme';
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

	<!-- Right: Language & Theme -->
	<div class="menu-right">
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
</style>
