<!--
  Copyright 2025 Assistance Micro Design

  VersionsHistoryModal — lists previous versions of a prompt or skill,
  previews their content, and restores them with a single click.

  Used by PromptForm and SkillForm in `edit` mode. The version snapshot is
  taken automatically on every update_prompt/update_skill on the backend, so
  the list reflects the full edit history.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { i18n } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import { Button, Badge } from '$lib/components/ui';
	import type { PromptVersion, PromptVersionSummary } from '$types/prompt_version';
	import type { SkillVersion, SkillVersionSummary } from '$types/skill_version';

	type AnyVersion = PromptVersion | SkillVersion;
	type AnyVersionSummary = PromptVersionSummary | SkillVersionSummary;

	interface Props {
		kind: 'prompt' | 'skill';
		/** The ID of the prompt or skill whose versions are listed. */
		resourceId: string;
		onclose: () => void;
		/** Called after a successful restore so the parent can refresh. */
		onrestored?: () => void;
	}

	let { kind, resourceId, onclose, onrestored }: Props = $props();

	let versions = $state.raw<AnyVersionSummary[]>([]);
	let preview = $state<AnyVersion | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let restoring = $state(false);

	const listCmd = $derived(kind === 'prompt' ? 'list_prompt_versions' : 'list_skill_versions');
	const getCmd = $derived(kind === 'prompt' ? 'get_prompt_version' : 'get_skill_version');
	const restoreCmd = $derived(
		kind === 'prompt' ? 'restore_prompt_version' : 'restore_skill_version'
	);
	const idParam = $derived(kind === 'prompt' ? 'promptId' : 'skillId');

	async function loadVersions() {
		loading = true;
		error = null;
		try {
			versions = await invoke<AnyVersionSummary[]>(listCmd, { [idParam]: resourceId });
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			loading = false;
		}
	}

	async function loadPreview(versionId: string) {
		try {
			preview = await invoke<AnyVersion>(getCmd, { versionId });
		} catch (e) {
			error = getErrorMessage(e);
		}
	}

	async function restoreVersion(versionId: string) {
		restoring = true;
		error = null;
		try {
			await invoke(restoreCmd, { [idParam]: resourceId, versionId, editedBy: 'user' });
			onrestored?.();
			onclose();
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			restoring = false;
		}
	}

	onMount(loadVersions);
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
	class="versions-modal-backdrop"
	role="dialog"
	aria-modal="true"
	aria-labelledby="versions-title"
	tabindex="-1"
	onclick={onclose}
>
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div class="versions-modal-panel" onclick={(e) => e.stopPropagation()} role="document">
		<header class="modal-head">
			<h3 id="versions-title">
				{kind === 'prompt'
					? $i18n('versions_history_prompt_title')
					: $i18n('versions_history_skill_title')}
			</h3>
			<button type="button" class="close" onclick={onclose} aria-label={$i18n('common_close')}>
				×
			</button>
		</header>

		<div class="modal-body">
			{#if loading}
				<p class="hint">{$i18n('versions_loading')}</p>
			{:else if error}
				<p class="error">{error}</p>
			{:else if versions.length === 0}
				<p class="hint">{$i18n('versions_empty')}</p>
			{:else}
				<div class="layout">
					<ul class="versions-list">
						{#each versions as v (v.id)}
							<li class="version-row">
								<div class="version-meta">
									<Badge variant="primary">v{v.version}</Badge>
									<span class="who">{v.edited_by}</span>
									<span class="when">{new Date(v.edited_at).toLocaleString()}</span>
								</div>
								{#if v.edit_summary}
									<p class="summary">{v.edit_summary}</p>
								{/if}
								<div class="actions">
									<Button type="button" variant="ghost" size="sm" onclick={() => loadPreview(v.id)}>
										{$i18n('versions_view')}
									</Button>
									<Button
										type="button"
										variant="primary"
										size="sm"
										disabled={restoring}
										onclick={() => restoreVersion(v.id)}
									>
										{$i18n('versions_restore')}
									</Button>
								</div>
							</li>
						{/each}
					</ul>
					{#if preview}
						<aside class="preview">
							<header class="preview-head">
								<span><strong>v{preview.version}</strong> — {preview.name}</span>
								<button
									type="button"
									class="close"
									onclick={() => (preview = null)}
									aria-label={$i18n('common_close')}>×</button
								>
							</header>
							<pre>{preview.content}</pre>
						</aside>
					{/if}
				</div>
			{/if}
		</div>
	</div>
</div>

<style>
	/* Stacked above the parent Modal's backdrop (--z-index-modal-backdrop = 1040)
	   and content (--z-index-modal = 1050) since this dialog is rendered nested
	   inside an already-open Modal (PromptSettings/SkillSettings). */
	.versions-modal-backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: var(--z-index-popover);
		padding: var(--spacing-lg);
	}
	.versions-modal-panel {
		background: var(--color-bg-primary);
		color: var(--color-text);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-xl);
		box-shadow: var(--shadow-xl);
		width: min(960px, 95vw);
		max-height: 85vh;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}
	.modal-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
	}
	.modal-head h3 {
		margin: 0;
		font-size: 1.05rem;
	}
	.close {
		background: none;
		border: none;
		color: inherit;
		font-size: 1.5rem;
		line-height: 1;
		cursor: pointer;
	}
	.modal-body {
		padding: 0.75rem 1rem;
		overflow: auto;
	}
	.hint {
		color: var(--color-text-muted);
	}
	.error {
		color: var(--color-error);
	}
	.layout {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr);
		gap: 1rem;
	}
	.versions-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	.version-row {
		border: 1px solid var(--color-border);
		border-radius: 6px;
		padding: 0.5rem 0.75rem;
	}
	.version-meta {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
	}
	.who {
		font-family: var(--font-mono);
		font-size: 0.85rem;
		color: var(--color-text-muted);
	}
	.when {
		font-size: 0.85rem;
		color: var(--color-text-muted);
	}
	.summary {
		margin: 0.25rem 0 0.5rem;
	}
	.actions {
		display: flex;
		gap: 0.5rem;
	}
	.preview {
		border: 1px solid var(--color-border);
		border-radius: 6px;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}
	.preview-head {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.5rem 0.75rem;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-surface-alt);
	}
	pre {
		margin: 0;
		padding: 0.75rem;
		white-space: pre-wrap;
		font-size: 0.85rem;
		overflow: auto;
	}
	@media (max-width: 700px) {
		.layout {
			grid-template-columns: 1fr;
		}
	}
</style>
