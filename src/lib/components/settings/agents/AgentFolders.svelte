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
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

AgentFolders - Manages the list of authorized folders for an agent.
Uses Tauri's native file dialog to pick folders, validates via backend command.
-->

<script lang="ts">
	import { tauriInvoke, openDialog, createTauriUnavailableError, isTauriRuntime } from '$lib/tauri';
	import { i18n } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import { Button } from '$lib/components/ui';
	import { Folder, FolderPlus, X } from '@lucide/svelte';

	/**
	 * Component props
	 */
	interface Props {
		/** List of authorized folder paths */
		folders: string[];
		/** Callback when folders list changes */
		onchange: (folders: string[]) => void;
	}

	let { folders, onchange }: Props = $props();

	/** Whether the folder picker dialog is currently open */
	let adding = $state(false);

	/** Error message from the last add attempt */
	let error = $state<string | null>(null);

	/**
	 * Opens the native folder picker and validates the selected folder.
	 * Checks for duplicates before and after backend validation (canonical path).
	 */
	async function addFolder(): Promise<void> {
		adding = true;
		error = null;
		try {
			if (!isTauriRuntime()) {
				throw createTauriUnavailableError('Tauri open dialog');
			}

			const selected = await openDialog({
				directory: true,
				multiple: false,
				title: $i18n('agents_folder_select_title')
			});

			if (!selected) {
				adding = false;
				return;
			}

			const path = typeof selected === 'string' ? selected : selected[0];
			if (!path) {
				adding = false;
				return;
			}

			// Check for duplicates before backend validation
			if (folders.includes(path)) {
				error = $i18n('agents_folder_duplicate');
				adding = false;
				return;
			}

			// Validate via backend (returns canonical path)
			const canonicalPath = await tauriInvoke<string>('validate_agent_folder', { path });

			// Check canonical path for duplicates too
			if (folders.includes(canonicalPath)) {
				error = $i18n('agents_folder_duplicate');
				adding = false;
				return;
			}

			onchange([...folders, canonicalPath]);
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			adding = false;
		}
	}

	/**
	 * Removes a folder from the list by index.
	 * @param index - Index of the folder to remove
	 */
	function removeFolder(index: number): void {
		const updated = folders.filter((_, i) => i !== index);
		onchange(updated);
	}
</script>

<div class="agent-folders">
	<div class="folders-card">
		{#if folders.length === 0}
			<p class="folder-empty">{$i18n('agents_folder_empty')}</p>
		{:else}
			<ul class="folder-list" role="list">
				{#each folders as folder, index (folder)}
					<li class="entity-row">
						<Folder size={16} class="folder-icon" />
						<span class="folder-path" title={folder}>{folder}</span>
						<div class="entity-actions">
							<Button
								variant="ghost"
								size="sm"
								onclick={() => removeFolder(index)}
								ariaLabel="{$i18n('agents_folder_remove')}: {folder}"
							>
								<X size={14} />
							</Button>
						</div>
					</li>
				{/each}
			</ul>
		{/if}

		<div class="folders-footer">
			<Button variant="outline" size="sm" onclick={addFolder} disabled={adding}>
				<FolderPlus size={14} />
				<span>{adding ? '...' : $i18n('agents_folder_add')}</span>
			</Button>
		</div>
	</div>

	<p class="folder-rules">{$i18n('agents_folder_rules')}</p>

	{#if error}
		<p class="folder-error" role="alert">{error}</p>
	{/if}
</div>

<style>
	.agent-folders {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.folders-card {
		background: var(--surface-1);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		overflow: hidden;
	}

	.folder-empty {
		margin: 0;
		padding: var(--spacing-md);
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.folder-list {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.entity-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		border-bottom: 1px solid var(--color-border-light);
	}

	.entity-row :global(.folder-icon) {
		color: var(--color-text-secondary);
		flex-shrink: 0;
	}

	.folder-path {
		flex: 1;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}

	.entity-actions {
		flex-shrink: 0;
	}

	.folders-footer {
		display: flex;
		justify-content: flex-start;
		align-items: center;
		padding: var(--spacing-md) var(--spacing-lg);
		border-top: 1px solid var(--color-border-light);
		background: var(--surface-2);
	}

	.folders-footer :global(button),
	.entity-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.folder-rules {
		margin: 0;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.folder-error {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-error);
	}
</style>
