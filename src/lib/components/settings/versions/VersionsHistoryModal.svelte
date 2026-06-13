<!--
  Copyright 2025 Assistance Micro Design

  VersionsHistoryModal — lists previous versions of a prompt or skill,
  previews their content, and restores them with a single click.

  Opened from the prompt/skill list rows. The version snapshot is taken
  automatically on every update_prompt/update_skill on the backend, so the
  list reflects the full edit history.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { tauriInvoke as invoke } from '$lib/tauri';
	import { i18n } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import { Button, Badge, DeleteConfirmModal } from '$lib/components/ui';
	import { Eye, RotateCcw, Trash2 } from '@lucide/svelte';
	import type { PromptVersion, PromptVersionSummary } from '$types/prompt_version';
	import type { SkillVersion, SkillVersionSummary } from '$types/skill_version';

	type AnyVersion = PromptVersion | SkillVersion;
	type AnyVersionSummary = PromptVersionSummary | SkillVersionSummary;

	interface Props {
		kind: 'prompt' | 'skill';
		/** The ID of the prompt or skill whose versions are listed. */
		resourceId: string;
		onclose: () => void;
		/**
		 * Called after a successful restore or delete so the parent can refresh
		 * (e.g. reload the list or the current content).
		 */
		onchanged?: () => void;
	}

	let { kind, resourceId, onclose, onchanged }: Props = $props();

	let versions = $state.raw<AnyVersionSummary[]>([]);
	let preview = $state<AnyVersion | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let restoring = $state(false);
	let deleting = $state(false);
	let pendingDelete = $state<{ id: string; version: number } | null>(null);

	const listCmd = $derived(kind === 'prompt' ? 'list_prompt_versions' : 'list_skill_versions');
	const getCmd = $derived(kind === 'prompt' ? 'get_prompt_version' : 'get_skill_version');
	const restoreCmd = $derived(
		kind === 'prompt' ? 'restore_prompt_version' : 'restore_skill_version'
	);
	const deleteCmd = $derived(kind === 'prompt' ? 'delete_prompt_version' : 'delete_skill_version');
	const idParam = $derived(kind === 'prompt' ? 'promptId' : 'skillId');

	/** Highest version number, badged on the brand color (older ones stay neutral). */
	const latestVersion = $derived(
		versions.length > 0 ? Math.max(...versions.map((v) => v.version)) : 0
	);

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
			onchanged?.();
			onclose();
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			restoring = false;
		}
	}

	function requestDelete(versionId: string, versionNumber: number) {
		pendingDelete = { id: versionId, version: versionNumber };
	}

	function cancelDelete() {
		if (!deleting) pendingDelete = null;
	}

	async function confirmDelete() {
		if (!pendingDelete) return;
		const { id: versionId } = pendingDelete;
		deleting = true;
		error = null;
		try {
			await invoke(deleteCmd, { versionId });
			if (preview && preview.id === versionId) {
				preview = null;
			}
			await loadVersions();
			onchanged?.();
			pendingDelete = null;
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			deleting = false;
		}
	}

	onMount(loadVersions);
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div
	class="versions-modal-backdrop"
	role="presentation"
	onclick={(e) => e.target === e.currentTarget && onclose()}
>
	<div
		class="versions-modal-panel"
		role="dialog"
		aria-modal="true"
		aria-labelledby="versions-title"
		tabindex="-1"
	>
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
					<ul class="versions-list" role="list">
						{#each versions as v (v.id)}
							<li class="entity-row">
								<Badge variant={v.version === latestVersion ? 'primary' : 'neutral'}>
									v{v.version}
								</Badge>
								<div class="entity-main">
									<strong class="entity-title">{v.edit_summary || `v${v.version}`}</strong>
									<span class="entity-meta">
										{new Date(v.edited_at).toLocaleString()} — {v.edited_by}
									</span>
								</div>
								<div class="entity-actions">
									<Button
										type="button"
										variant="ghost"
										size="sm"
										onclick={() => loadPreview(v.id)}
										ariaLabel="{$i18n('versions_view')} v{v.version}"
									>
										<Eye size={14} />
									</Button>
									<Button
										type="button"
										variant="ghost"
										size="sm"
										disabled={restoring || deleting}
										onclick={() => restoreVersion(v.id)}
										ariaLabel="{$i18n('versions_restore')} v{v.version}"
									>
										<RotateCcw size={14} />
									</Button>
									<Button
										type="button"
										variant="ghost"
										size="sm"
										disabled={deleting || restoring || versions.length <= 1}
										onclick={() => requestDelete(v.id, v.version)}
										ariaLabel={versions.length <= 1
											? $i18n('versions_delete_blocked_last')
											: `${$i18n('versions_delete')} v${v.version}`}
									>
										<Trash2 size={14} />
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

		<footer class="modal-foot">
			<Button type="button" variant="ghost" onclick={onclose}>
				{$i18n('common_close')}
			</Button>
		</footer>
	</div>
</div>

<DeleteConfirmModal
	open={pendingDelete !== null}
	titleKey="versions_delete_modal_title"
	confirmMessageKey="versions_confirm_delete_message"
	itemName={pendingDelete ? `v${pendingDelete.version}` : undefined}
	warningMessageKey="versions_confirm_delete_warning"
	{deleting}
	onConfirm={confirmDelete}
	onCancel={cancelDelete}
	elevated
/>

<style>
	/* Stacked above popover-level overlays so this dialog reads on top of the
	   settings page. The delete confirmation opened from here is nested one
	   level deeper again, so it uses the Modal `elevated` prop
	   (--z-index-modal-nested) to stack above this backdrop. */
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
		background: var(--surface-1);
		color: var(--color-text-primary);
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
		padding: var(--spacing-md) var(--spacing-lg);
		border-bottom: 1px solid var(--color-border-light);
	}
	.modal-head h3 {
		margin: 0;
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
	}
	.close {
		background: none;
		border: none;
		color: inherit;
		font-size: var(--font-size-2xl);
		line-height: 1;
		cursor: pointer;
	}
	.modal-body {
		padding: var(--spacing-md) var(--spacing-lg);
		overflow: auto;
		flex: 1;
	}
	.modal-foot {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-sm);
		padding: var(--spacing-md) var(--spacing-lg);
		border-top: 1px solid var(--color-border-light);
		background: var(--surface-2);
	}
	.hint {
		color: var(--color-text-secondary);
	}
	.error {
		color: var(--color-error);
	}
	.layout {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr);
		gap: var(--spacing-md);
	}
	.versions-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
	}
	.entity-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-md) 0;
		border-bottom: 1px solid var(--color-border-light);
	}
	.entity-row:last-child {
		border-bottom: none;
	}
	.entity-main {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}
	.entity-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
	}
	.entity-meta {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}
	.entity-actions {
		display: flex;
		gap: var(--spacing-xs);
		flex-shrink: 0;
	}
	.preview {
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}
	.preview-head {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: var(--spacing-sm) var(--spacing-md);
		border-bottom: 1px solid var(--color-border-light);
		background: var(--surface-2);
	}
	pre {
		margin: 0;
		padding: var(--spacing-md);
		white-space: pre-wrap;
		font-size: var(--font-size-sm);
		overflow: auto;
	}
	@media (max-width: 700px) {
		.layout {
			grid-template-columns: 1fr;
		}
	}
</style>
