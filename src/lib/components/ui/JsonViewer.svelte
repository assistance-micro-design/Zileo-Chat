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
  JsonViewer Component
  Recursive collapsible JSON tree viewer with type-colored values.
  Supports collapse/expand per node and truncation of long strings.

  @example
  <JsonViewer data={jsonObject} maxDepth={3} collapsed={true} />
-->
<script lang="ts">
	import { ChevronRight, ChevronDown } from '@lucide/svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import JsonViewerSelf from './JsonViewer.svelte';

	interface Props {
		/** JSON data to display */
		data: unknown;
		/** Maximum nesting depth (default: 3) */
		maxDepth?: number;
		/** Whether nodes start collapsed (default: true) */
		collapsed?: boolean;
		/** Current depth (internal, do not set) */
		depth?: number;
	}

	let {
		data,
		maxDepth = 3,
		collapsed = true,
		depth = 0
	}: Props = $props();

	/** Track expanded state per key */
	let expandedKeys = new SvelteSet<string>();

	/** Max string length before truncation */
	const MAX_STRING_LENGTH = 200;

	function toggleKey(key: string): void {
		if (expandedKeys.has(key)) {
			expandedKeys.delete(key);
		} else {
			expandedKeys.add(key);
		}
	}

	function getType(value: unknown): string {
		if (value === null) return 'null';
		if (Array.isArray(value)) return 'array';
		return typeof value;
	}

	function isExpandable(value: unknown): boolean {
		return (typeof value === 'object' && value !== null);
	}

	function getEntries(value: unknown): [string, unknown][] {
		if (Array.isArray(value)) {
			return value.map((v, i) => [String(i), v]);
		}
		if (typeof value === 'object' && value !== null) {
			return Object.entries(value);
		}
		return [];
	}

	function getPreview(value: unknown): string {
		if (Array.isArray(value)) {
			return `[${value.length}]`;
		}
		if (typeof value === 'object' && value !== null) {
			const keys = Object.keys(value);
			return `{${keys.length}}`;
		}
		return '';
	}

	function formatValue(value: unknown): string {
		if (value === null) return 'null';
		if (typeof value === 'string') {
			if (value.length > MAX_STRING_LENGTH) {
				return `"${value.slice(0, MAX_STRING_LENGTH)}..."`;
			}
			return `"${value}"`;
		}
		if (typeof value === 'boolean') return String(value);
		if (typeof value === 'number') return String(value);
		return String(value);
	}
</script>

<div class="json-viewer" class:root={depth === 0}>
	{#if isExpandable(data)}
		{@const entries = getEntries(data)}
		{#if depth === 0 && entries.length === 0}
			<span class="json-empty">{Array.isArray(data) ? '[]' : '{}'}</span>
		{:else}
			{#each entries as [key, value] (key)}
				{@const isExp = isExpandable(value)}
				{@const isOpen = !collapsed ? !expandedKeys.has(key) : expandedKeys.has(key)}
				<div class="json-entry">
					{#if isExp && depth < maxDepth}
						<button class="json-toggle" onclick={() => toggleKey(key)}>
							{#if isOpen}
								<ChevronDown size={12} />
							{:else}
								<ChevronRight size={12} />
							{/if}
						</button>
					{:else}
						<span class="json-spacer"></span>
					{/if}
					<span class="json-key">{key}</span>
					<span class="json-colon">:</span>
					{#if isExp}
						{#if depth >= maxDepth}
							<span class="json-preview">{getPreview(value)}</span>
						{:else if isOpen}
							<span class="json-preview">{getPreview(value)}</span>
							<div class="json-children">
								<JsonViewerSelf data={value} {maxDepth} {collapsed} depth={depth + 1} />
							</div>
						{:else}
							<span class="json-preview">{getPreview(value)}</span>
						{/if}
					{:else}
						<span class="json-value json-type-{getType(value)}">{formatValue(value)}</span>
					{/if}
				</div>
			{/each}
		{/if}
	{:else}
		<span class="json-value json-type-{getType(data)}">{formatValue(data)}</span>
	{/if}
</div>

<style>
	.json-viewer {
		font-family: var(--font-mono);
		font-size: var(--font-size-xs);
		line-height: 1.6;
	}

	.json-viewer.root {
		padding: var(--spacing-xs) 0;
	}

	.json-entry {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		gap: 2px;
		padding-left: var(--spacing-sm);
	}

	.json-toggle {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0;
		border: none;
		background: transparent;
		cursor: pointer;
		color: var(--color-text-tertiary);
		width: 14px;
		height: 14px;
		flex-shrink: 0;
		margin-top: 3px;
	}

	.json-toggle:hover {
		color: var(--color-accent);
	}

	.json-spacer {
		width: 14px;
		flex-shrink: 0;
	}

	.json-key {
		color: var(--color-text-primary);
		font-weight: 500;
		flex-shrink: 0;
	}

	.json-colon {
		color: var(--color-text-tertiary);
		margin-right: 4px;
		flex-shrink: 0;
	}

	.json-preview {
		color: var(--color-text-tertiary);
		font-style: italic;
	}

	.json-value {
		word-break: break-all;
	}

	.json-type-string {
		color: var(--color-success);
	}

	.json-type-number {
		color: var(--color-accent);
	}

	.json-type-boolean {
		color: var(--color-warning);
	}

	.json-type-null {
		color: var(--color-text-tertiary);
		font-style: italic;
	}

	.json-children {
		width: 100%;
	}

	.json-empty {
		color: var(--color-text-tertiary);
		font-style: italic;
	}
</style>
