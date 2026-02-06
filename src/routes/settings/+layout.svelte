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
Settings Layout - Route-based navigation with sidebar (OPT-SCROLL-ROUTES)
Each section is now a separate route for better performance and UX.
-->

<script lang="ts">
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
		FolderSync
	} from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	/** Props from +layout.ts */
	interface Props {
		data: { pathname: string };
		children: import('svelte').Snippet;
	}
	let { data, children }: Props = $props();

	/** UI state */
	let sidebarCollapsed = $state(false);
	let contentAreaRef: HTMLElement | null = $state(null);
	let isScrolling = $state(false);

	/**
	 * OPT-SCROLL-FIX: Disable pointer events during scroll to prevent
	 * expensive hover state recalculations in WebKit2GTK
	 */
	let scrollTimeout: ReturnType<typeof setTimeout>;

	function handleScroll(): void {
		if (!isScrolling) {
			isScrolling = true;
		}
		clearTimeout(scrollTimeout);
		scrollTimeout = setTimeout(() => {
			isScrolling = false;
		}, 100);
	}

	$effect(() => {
		const el = contentAreaRef;
		if (!el) return;

		el.addEventListener('scroll', handleScroll, { passive: true });

		return () => {
			el.removeEventListener('scroll', handleScroll);
			clearTimeout(scrollTimeout);
		};
	});

	/** Navigation sections with routes */
	const sectionDefs = [
		{ id: 'providers', route: '/settings/providers', labelKey: 'settings_providers', icon: Globe },
		{ id: 'agents', route: '/settings/agents', labelKey: 'settings_agents', icon: Bot },
		{ id: 'mcp', route: '/settings/mcp', labelKey: 'settings_mcp_servers', icon: Plug },
		{ id: 'memory', route: '/settings/memory', labelKey: 'settings_memory', icon: Brain },
		{ id: 'validation', route: '/settings/validation', labelKey: 'settings_validation', icon: ShieldCheck },
		{ id: 'prompts', route: '/settings/prompts', labelKey: 'settings_prompts', icon: BookOpen },
		{ id: 'import-export', route: '/settings/import-export', labelKey: 'settings_import_export', icon: FolderSync },
		{ id: 'theme', route: '/settings/theme', labelKey: 'settings_theme', icon: Palette }
	] as const;

	/**
	 * Determine active section from current URL
	 */
	let activeSection = $derived.by(() => {
		const pathname = data.pathname;
		const section = sectionDefs.find(s => pathname.startsWith(s.route));
		return section?.id ?? 'providers';
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
							title={$i18n(section.labelKey)}
						>
							<Icon size={20} />
						</a>
					{/each}
				</div>
			{/if}
		{/snippet}

		{#snippet footer()}
			{#if sidebarCollapsed}
				<div class="security-badge-collapsed" title={$i18n('settings_security_badge')}>
					<ShieldCheck size={20} />
				</div>
			{:else}
				<div class="security-badge">
					<ShieldCheck size={16} />
					<span class="security-text">{$i18n('settings_security_badge')}</span>
				</div>
			{/if}
		{/snippet}
	</Sidebar>

	<!-- Settings Content -->
	<main
		bind:this={contentAreaRef}
		class="content-area"
		class:is-scrolling={isScrolling}
	>
		{@render children()}
	</main>
</div>

<style>
	.settings-page {
		display: flex;
		height: 100%;
		flex: 1;
		min-width: 0;
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
		color: var(--color-accent);
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
		transition: background-color var(--transition-fast), color var(--transition-fast);
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
		color: var(--color-accent);
		font-weight: var(--font-weight-medium);
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
		transition: background-color var(--transition-fast), color var(--transition-fast);
		text-decoration: none;
	}

	.nav-button-icon:hover {
		background: var(--color-bg-hover);
		color: var(--color-text-primary);
	}

	.nav-button-icon.active {
		background: var(--color-accent-light);
		color: var(--color-accent);
	}

	.security-badge {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-sm);
		background: var(--color-success-light);
		border-radius: var(--border-radius-md);
		color: var(--color-success);
	}

	.security-badge-collapsed {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-sm);
		background: var(--color-success-light);
		border-radius: var(--border-radius-md);
		color: var(--color-success);
	}

	.security-text {
		font-size: var(--font-size-xs);
	}

	/* Content Area */
	.content-area {
		flex: 1;
		min-height: 0;
		min-width: 0;
		overflow-y: auto;
		padding: var(--spacing-xl);
		-webkit-overflow-scrolling: touch;
		/* OPT-SCROLL-FIX: contain: content instead of will-change */
		/* will-change: scroll-position causes GPU overhead in WebKit2GTK */
		contain: content;
	}

	/**
	 * OPT-SCROLL-FIX: Disable pointer events during scroll
	 * This prevents expensive hover state recalculations in WebKit2GTK
	 * The technique is used by major apps like Twitter/X for smooth scrolling
	 */
	.content-area.is-scrolling {
		pointer-events: none;
	}

	.content-area.is-scrolling :global(*) {
		pointer-events: none !important;
	}
</style>
