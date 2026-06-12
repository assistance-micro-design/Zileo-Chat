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
