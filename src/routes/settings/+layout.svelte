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
Settings Layout - Route-based navigation with sidebar
Each section is now a separate route for better performance and UX.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { Sidebar } from '$lib/components/layout';

	import {
		Globe,
		Palette,
		Plug,
		ShieldCheck,
		Brain,
		Bot,
		Settings,
		BookOpen,
		BookMarked,
		FolderSync,
		ScrollText,
		Mic,
		KanbanSquare
	} from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { pauseOnScroll } from '$lib/actions/pauseOnScroll';
	import { loadAllLLMData, testConnection } from '$lib/stores/llm';

	/** Props from +layout.ts */
	interface Props {
		data: { pathname: string };
		children: import('svelte').Snippet;
	}
	let { data, children }: Props = $props();

	/** UI state */
	let sidebarCollapsed = $state(false);

	/** Navigation sections with routes */
	const sectionDefs = [
		{ id: 'providers', route: '/settings/providers', labelKey: 'settings_providers', icon: Globe },
		{ id: 'agents', route: '/settings/agents', labelKey: 'settings_agents', icon: Bot },
		{ id: 'kanban', route: '/settings/kanban', labelKey: 'settings_kanban', icon: KanbanSquare },
		{ id: 'mcp', route: '/settings/mcp', labelKey: 'settings_mcp_servers', icon: Plug },
		{ id: 'memory', route: '/settings/memory', labelKey: 'settings_memory', icon: Brain },
		{
			id: 'validation',
			route: '/settings/validation',
			labelKey: 'settings_validation',
			icon: ShieldCheck
		},
		{
			id: 'audit-log',
			route: '/settings/audit-log',
			labelKey: 'settings_audit_log',
			icon: ScrollText
		},
		{ id: 'prompts', route: '/settings/prompts', labelKey: 'settings_prompts', icon: BookOpen },
		{ id: 'skills', route: '/settings/skills', labelKey: 'settings_skills', icon: BookMarked },
		{
			id: 'import-export',
			route: '/settings/import-export',
			labelKey: 'settings_import_export',
			icon: FolderSync
		},
		{
			id: 'speech-to-text',
			route: '/settings/speech-to-text',
			labelKey: 'settings_speech_to_text',
			icon: Mic
		},
		{ id: 'theme', route: '/settings/theme', labelKey: 'settings_theme', icon: Palette }
	] as const;

	/**
	 * Determine active section from current URL
	 */
	let activeSection = $derived.by(() => {
		const pathname = data.pathname;
		const section = sectionDefs.find((s) => pathname.startsWith(s.route));
		return section?.id ?? 'providers';
	});

	/** Connectivity state shown for each provider in the sidebar footer. */
	type ProviderState = 'unconfigured' | 'checking' | 'functional' | 'offline';

	interface ProviderStatus {
		id: string;
		displayName: string;
		state: ProviderState;
	}

	let providerStatuses = $state<ProviderStatus[]>([]);
	let providersLoading = $state(true);

	/** Maps a connectivity state to its localized label. */
	function providerStateLabel(state: ProviderState): string {
		switch (state) {
			case 'functional':
				return $i18n('settings_provider_state_functional');
			case 'offline':
				return $i18n('settings_provider_state_offline');
			case 'checking':
				return $i18n('settings_provider_state_checking');
			default:
				return $i18n('llm_provider_not_configured');
		}
	}

	/** Replaces one provider's state immutably so the list stays reactive. */
	function setProviderState(id: string, state: ProviderState): void {
		providerStatuses = providerStatuses.map((p) => (p.id === id ? { ...p, state } : p));
	}

	// Load the provider list once when entering Settings (the layout persists
	// across sub-route navigations, so this runs a single time). Configured
	// providers are then probed concurrently and their dot upgrades to
	// functional/offline as each result lands.
	onMount(() => {
		let active = true;

		void (async () => {
			try {
				const data = await loadAllLLMData();
				if (!active) return;
				providerStatuses = data.providerList
					.filter((p) => p.enabled)
					.map((p): ProviderStatus => {
						const configured =
							!p.requiresApiKey || data.settings[p.id]?.api_key_configured === true;
						return {
							id: p.id,
							displayName: p.displayName,
							state: configured ? 'checking' : 'unconfigured'
						};
					});

				// Failures are swallowed into 'offline' so one unreachable provider
				// never rejects the whole batch.
				await Promise.all(
					providerStatuses
						.filter((p) => p.state === 'checking')
						.map(async ({ id }) => {
							try {
								const result = await testConnection(id);
								if (active) setProviderState(id, result.success ? 'functional' : 'offline');
							} catch {
								if (active) setProviderState(id, 'offline');
							}
						})
				);
			} catch {
				// Leave the panel empty on a hard load failure.
			} finally {
				if (active) providersLoading = false;
			}
		})();

		return () => {
			active = false;
		};
	});
</script>

<div class="settings-page">
	<!-- Settings Sidebar -->
	<Sidebar bind:collapsed={sidebarCollapsed}>
		{#snippet header()}
			{#if sidebarCollapsed}
				<div class="sidebar-icon-collapsed" title={$i18n('settings_title')}>
					<Settings size={24} />
				</div>
			{:else}
				<h2 class="sidebar-title">{$i18n('settings_title')}</h2>
			{/if}
		{/snippet}

		{#snippet nav()}
			{#if !sidebarCollapsed}
				<div class="nav-items">
					{#each sectionDefs as section (section.id)}
						{@const Icon = section.icon}
						<a
							href={section.route}
							class="nav-button"
							class:active={activeSection === section.id}
							aria-current={activeSection === section.id ? 'page' : undefined}
						>
							<Icon size={20} />
							<span class="nav-text">{$i18n(section.labelKey)}</span>
						</a>
					{/each}
				</div>
			{:else}
				<div class="nav-items-collapsed">
					{#each sectionDefs as section (section.id)}
						{@const Icon = section.icon}
						<a
							href={section.route}
							class="nav-button-icon"
							class:active={activeSection === section.id}
							aria-current={activeSection === section.id ? 'page' : undefined}
							title={$i18n(section.labelKey)}
							aria-label={$i18n(section.labelKey)}
						>
							<Icon size={20} />
						</a>
					{/each}
				</div>
			{/if}
		{/snippet}

		{#snippet footer()}
			{#if sidebarCollapsed}
				<div class="providers-status providers-status-collapsed">
					{#each providerStatuses as provider (provider.id)}
						<span
							class="provider-dot"
							data-state={provider.state}
							title={`${provider.displayName} — ${providerStateLabel(provider.state)}`}
						></span>
					{/each}
				</div>
			{:else}
				<div class="providers-status">
					<span class="providers-status-title">{$i18n('settings_providers')}</span>
					{#if providersLoading && providerStatuses.length === 0}
						<span class="providers-status-empty">{$i18n('providers_loading')}</span>
					{:else if providerStatuses.length === 0}
						<span class="providers-status-empty">{$i18n('settings_providers_none')}</span>
					{:else}
						<ul class="providers-status-list">
							{#each providerStatuses as provider (provider.id)}
								<li class="provider-status-item">
									<span class="provider-dot" data-state={provider.state}></span>
									<span class="provider-status-name">{provider.displayName}</span>
									<span class="provider-status-state" data-state={provider.state}>
										{providerStateLabel(provider.state)}
									</span>
								</li>
							{/each}
						</ul>
					{/if}
				</div>
			{/if}
		{/snippet}
	</Sidebar>

	<!-- Settings Content -->
	<main class="content-area" {@attach pauseOnScroll()}>
		{@render children()}
	</main>
</div>

<style>
	.settings-page {
		display: flex;
		flex: 1;
		min-width: 0;
		min-height: 0;
		overflow: hidden;
		/* Detach the sidebar card from the viewport edges with a gutter + gap,
		   matching the floating sidebar of the agent page. */
		gap: var(--spacing-md);
		padding: 0 var(--spacing-md) var(--spacing-md);
	}

	/* Sidebar */
	.sidebar-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
	}

	.sidebar-icon-collapsed {
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--color-accent-deep);
		padding: var(--spacing-xs);
	}

	.nav-items {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.nav-items-collapsed {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.nav-button {
		position: relative;
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		background: transparent;
		border: none;
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast);
		width: 100%;
		text-align: left;
		text-decoration: none;
	}

	.nav-button:hover {
		background: var(--color-bg-hover);
		color: var(--color-text-primary);
	}

	.nav-button.active {
		background: var(--color-accent-light);
		color: var(--color-accent-deep);
		font-weight: var(--font-weight-semibold);
	}

	/* Glowing gradient bar marking the active section */
	.nav-button.active::before {
		content: '';
		position: absolute;
		left: -8px;
		top: 20%;
		bottom: 20%;
		width: 3px;
		border-radius: var(--border-radius-full);
		background: var(--gradient-brand);
		box-shadow: var(--glow-accent-soft);
	}

	.nav-button-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-sm);
		color: var(--color-text-secondary);
		background: transparent;
		border: none;
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast);
		text-decoration: none;
	}

	.nav-button-icon:hover {
		background: var(--color-bg-hover);
		color: var(--color-text-primary);
	}

	.nav-button-icon.active {
		background: var(--color-accent-light);
		color: var(--color-accent-deep);
	}

	/* Provider connectivity panel (sidebar footer) */
	.providers-status {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
		width: 100%;
	}

	.providers-status-collapsed {
		align-items: center;
		gap: var(--spacing-sm);
	}

	.providers-status-title {
		font-size: var(--font-size-2xs);
		font-weight: var(--font-weight-semibold);
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--color-text-tertiary);
	}

	.providers-status-empty {
		font-size: var(--font-size-xs);
		font-style: italic;
		color: var(--color-text-tertiary);
	}

	.providers-status-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2xs);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.provider-status-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		font-size: var(--font-size-xs);
	}

	.provider-status-name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--color-text-secondary);
	}

	.provider-status-state {
		flex-shrink: 0;
		font-size: var(--font-size-2xs);
		color: var(--color-text-tertiary);
	}

	.provider-status-state[data-state='functional'] {
		color: var(--color-success);
	}

	.provider-status-state[data-state='offline'] {
		color: var(--color-danger);
	}

	/* Glowing connectivity dot */
	.provider-dot {
		width: 8px;
		height: 8px;
		flex-shrink: 0;
		border-radius: var(--border-radius-full);
		background: var(--color-text-tertiary);
	}

	.provider-dot[data-state='unconfigured'] {
		background: var(--color-text-tertiary);
		opacity: 0.55;
	}

	.provider-dot[data-state='checking'] {
		background: var(--color-accent-deep);
	}

	.provider-dot[data-state='functional'] {
		background: var(--color-success);
		box-shadow: 0 0 6px color-mix(in srgb, var(--color-success) 55%, transparent);
	}

	.provider-dot[data-state='offline'] {
		background: var(--color-danger);
		box-shadow: 0 0 6px color-mix(in srgb, var(--color-danger) 45%, transparent);
	}

	@media (prefers-reduced-motion: no-preference) {
		.provider-dot[data-state='checking'] {
			animation: provider-dot-pulse 1.3s ease-in-out infinite;
		}
	}

	@keyframes provider-dot-pulse {
		0%,
		100% {
			opacity: 0.35;
		}
		50% {
			opacity: 1;
		}
	}

	/* Shared settings page styles (scoped to content area) */
	.content-area :global(.settings-section) {
		margin-bottom: var(--spacing-2xl);
		padding-bottom: var(--spacing-xl);
	}

	.content-area :global(.lazy-loading) {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-xl);
	}

	/* Content Area */
	.content-area {
		flex: 1;
		min-height: 0;
		min-width: 0;
		overflow-y: auto;
		padding: var(--spacing-xl);
		-webkit-overflow-scrolling: touch;
		/* Opaque background: a transparent scroll container over the body
		   gradient forces WebKitGTK to repaint the whole visible area on
		   every scrolled frame (it cannot blit the moving pixels), which made
		   the settings pages scroll visibly slower than the chat or kanban
		   surfaces, whose scroll containers are opaque. */
		background: var(--color-bg-primary);
		/* Promote the scroller for accelerated scrolling. Unlike the chat
		   (whose animated rail nodes force composited layers, pulling its
		   scroller onto the async path), settings pages have no composited
		   descendant, so WebKitGTK keeps them on main-thread scrolling where
		   every frame repaints shadowed cards. scroll-position is the one
		   will-change value that does NOT create a containing block, so the
		   fixed-position modal backdrops rendered inside the pages are safe. */
		will-change: scroll-position;
	}

	/* Disable pointer events on the CHILDREN during scroll to avoid expensive
	   hover-state recalculations in WebKitGTK (Twitter/X technique). The
	   is-scrolling class is toggled at runtime by the pauseOnScroll
	   attachment, so it must be :global() or the compiler would prune the
	   selector as unused.

	   NEVER put pointer-events: none on the scroll container itself: it makes
	   the whole subtree transparent to hit-testing, so the wheel events that
	   follow the first scrolled notch target the overflow:hidden ancestors
	   and nothing scrolls until the 250 ms idle timer clears the class --
	   the page then only advances one notch per ~250 ms window ("stuck,
	   point-by-point" mouse-wheel scrolling). */
	.content-area:global(.is-scrolling) > :global(*) {
		pointer-events: none;
	}
</style>
