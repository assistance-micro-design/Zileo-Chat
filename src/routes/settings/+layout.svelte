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
	}

	/**
	 * Disable pointer events during scroll
	 * This prevents expensive hover state recalculations in WebKit2GTK
	 * The technique is used by major apps like Twitter/X for smooth scrolling
	 * Removed :global(*) selector - parent is sufficient
	 */
	/* The is-scrolling class is toggled at runtime by the pauseOnScroll
	   attachment, so it must be :global() or the compiler would prune the
	   selector as unused. Disabling pointer events during scroll avoids
	   expensive hover-state recalculations in WebKitGTK. */
	.content-area:global(.is-scrolling) {
		pointer-events: none;
	}
</style>
