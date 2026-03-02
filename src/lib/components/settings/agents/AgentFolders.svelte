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
	import { invoke } from '@tauri-apps/api/core';
	import { open } from '@tauri-apps/plugin-dialog';
	import { i18n } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import { Button } from '$lib/components/ui';

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
			const selected = await open({
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
			const canonicalPath = await invoke<string>('validate_agent_folder', { path });

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
	{#if folders.length === 0}
		<div class="empty-state">
			<p>{$i18n('agents_folder_empty')}</p>
		</div>
	{:else}
		<ul class="folder-list" role="list">
			{#each folders as folder, index (folder)}
				<li class="folder-item">
					<span class="folder-path" title={folder}>{folder}</span>
					<Button
						variant="ghost"
						size="sm"
						onclick={() => removeFolder(index)}
						ariaLabel="{$i18n('agents_folder_remove')}: {folder}"
					>
						{$i18n('agents_folder_remove')}
					</Button>
				</li>
			{/each}
		</ul>
	{/if}

	<div class="folder-actions">
		<Button variant="secondary" size="sm" onclick={addFolder} disabled={adding}>
			{adding ? '...' : $i18n('agents_folder_add')}
		</Button>
	</div>

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

	.empty-state {
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.empty-state p {
		margin: 0;
	}

	.folder-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.folder-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-sm);
		padding: var(--spacing-xs) var(--spacing-sm);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
		border: 1px solid var(--color-border);
	}

	.folder-path {
		flex: 1;
		font-size: var(--font-size-sm);
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}

	.folder-actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.folder-error {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-error);
	}
</style>
