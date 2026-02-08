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
  MarkdownRenderer Component
  Renders markdown content as sanitized HTML.
  Uses marked for parsing and DOMPurify for XSS protection.
  Intercepts link clicks with a confirmation popup.

  @example
  <MarkdownRenderer content={markdownString} />
-->
<script lang="ts">
	import { marked } from 'marked';
	import DOMPurify from 'dompurify';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { i18n } from '$lib/i18n';
	import { ExternalLink, Copy, X } from '@lucide/svelte';

	/**
	 * MarkdownRenderer props
	 */
	interface Props {
		/** Markdown content to render */
		content: string;
		/** Compact mode (reduced spacing) */
		compact?: boolean;
	}

	let { content, compact = false }: Props = $props();

	const html = $derived(DOMPurify.sanitize(marked.parse(content) as string));

	let linkPopup = $state<{ url: string; x: number; y: number } | null>(null);
	let copied = $state(false);

	function handleClick(event: MouseEvent): void {
		const target = event.target as HTMLElement;
		const anchor = target.closest('a');
		if (!anchor) return;

		event.preventDefault();
		event.stopPropagation();

		const url = anchor.getAttribute('href');
		if (!url) return;

		const rect = anchor.getBoundingClientRect();
		linkPopup = {
			url,
			x: rect.left,
			y: rect.bottom + 4
		};
		copied = false;
	}

	function closePopup(): void {
		linkPopup = null;
		copied = false;
	}

	async function handleOpenInBrowser(): Promise<void> {
		if (!linkPopup) return;
		await openUrl(linkPopup.url);
		closePopup();
	}

	async function handleCopyUrl(): Promise<void> {
		if (!linkPopup) return;
		await navigator.clipboard.writeText(linkPopup.url);
		copied = true;
		setTimeout(() => {
			closePopup();
		}, 1000);
	}
</script>

<svelte:window
	onclick={(e) => {
		if (linkPopup) {
			const target = e.target as HTMLElement;
			if (!target.closest('.link-popup')) {
				closePopup();
			}
		}
	}}
/>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="markdown-content" class:compact onclick={handleClick} onkeydown={(e) => {
	if (e.key === 'Enter' || e.key === ' ') {
		handleClick(e as unknown as MouseEvent);
	}
}}>
	<!-- eslint-disable-next-line svelte/no-at-html-tags -- Content sanitized via DOMPurify -->
	{@html html}
</div>

{#if linkPopup}
	<div
		class="link-popup"
		style="left: {linkPopup.x}px; top: {linkPopup.y}px;"
		role="dialog"
		aria-label={$i18n('link_popup_arialabel')}
	>
		<div class="link-popup-url" title={linkPopup.url}>{linkPopup.url}</div>
		<div class="link-popup-actions">
			<button class="link-popup-btn open" onclick={handleOpenInBrowser}>
				<ExternalLink size={14} />
				{$i18n('link_open_browser')}
			</button>
			<button class="link-popup-btn copy" onclick={handleCopyUrl}>
				<Copy size={14} />
				{copied ? $i18n('chat_copied') : $i18n('link_copy_url')}
			</button>
			<button class="link-popup-btn close" onclick={closePopup} aria-label={$i18n('common_cancel')}>
				<X size={14} />
			</button>
		</div>
	</div>
{/if}

<style>
	.markdown-content {
		line-height: var(--line-height-relaxed);
		word-break: break-word;
	}

	.markdown-content :global(h1) {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-bold);
		margin: var(--spacing-md) 0 var(--spacing-sm);
	}

	.markdown-content :global(h2) {
		font-size: var(--font-size-md);
		font-weight: var(--font-weight-semibold);
		margin: var(--spacing-md) 0 var(--spacing-sm);
	}

	.markdown-content :global(h3) {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		margin: var(--spacing-sm) 0 var(--spacing-xs);
	}

	.markdown-content :global(p) {
		margin: var(--spacing-sm) 0;
	}

	.markdown-content :global(p:first-child) {
		margin-top: 0;
	}

	.markdown-content :global(p:last-child) {
		margin-bottom: 0;
	}

	.markdown-content :global(code) {
		font-family: var(--font-mono);
		font-size: var(--font-size-xs);
		padding: 2px var(--spacing-xs);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-sm);
	}

	.markdown-content :global(pre) {
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-md);
		padding: var(--spacing-md);
		overflow-x: auto;
		margin: var(--spacing-sm) 0;
	}

	.markdown-content :global(pre code) {
		background: none;
		padding: 0;
		font-size: var(--font-size-xs);
	}

	.markdown-content :global(ul),
	.markdown-content :global(ol) {
		margin: var(--spacing-sm) 0;
		padding-left: var(--spacing-xl);
	}

	.markdown-content :global(li) {
		margin: var(--spacing-xs) 0;
	}

	.markdown-content :global(blockquote) {
		border-left: 3px solid var(--color-border);
		margin: var(--spacing-sm) 0;
		padding: var(--spacing-xs) var(--spacing-md);
		color: var(--color-text-secondary);
	}

	.markdown-content :global(table) {
		border-collapse: collapse;
		width: 100%;
		margin: var(--spacing-sm) 0;
		font-size: var(--font-size-xs);
	}

	.markdown-content :global(th),
	.markdown-content :global(td) {
		border: 1px solid var(--color-border);
		padding: var(--spacing-xs) var(--spacing-sm);
		text-align: left;
	}

	.markdown-content :global(th) {
		background: var(--color-bg-tertiary);
		font-weight: var(--font-weight-semibold);
	}

	.markdown-content :global(strong) {
		font-weight: var(--font-weight-semibold);
	}

	.markdown-content :global(a) {
		color: var(--color-accent);
		text-decoration: underline;
		cursor: pointer;
	}

	.markdown-content :global(hr) {
		border: none;
		border-top: 1px solid var(--color-border);
		margin: var(--spacing-md) 0;
	}

	/* Compact mode */
	.markdown-content.compact :global(h1) {
		font-size: var(--font-size-md);
		margin: var(--spacing-sm) 0 var(--spacing-xs);
	}

	.markdown-content.compact :global(h2) {
		font-size: var(--font-size-sm);
		margin: var(--spacing-sm) 0 var(--spacing-xs);
	}

	.markdown-content.compact :global(p) {
		margin: var(--spacing-xs) 0;
	}

	/* Link confirmation popup */
	.link-popup {
		position: fixed;
		z-index: 1000;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		box-shadow: var(--shadow-lg);
		padding: var(--spacing-sm);
		max-width: 400px;
		min-width: 240px;
	}

	.link-popup-url {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		word-break: break-all;
		padding: var(--spacing-xs);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-sm);
		margin-bottom: var(--spacing-sm);
		max-height: 60px;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.link-popup-actions {
		display: flex;
		gap: var(--spacing-xs);
		align-items: center;
	}

	.link-popup-btn {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-xs);
		padding: var(--spacing-xs) var(--spacing-sm);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-sm);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font-size: var(--font-size-xs);
		cursor: pointer;
		transition: background 0.15s;
		white-space: nowrap;
	}

	.link-popup-btn:hover {
		background: var(--color-bg-tertiary);
	}

	.link-popup-btn.open {
		background: var(--color-accent);
		color: white;
		border-color: var(--color-accent);
	}

	.link-popup-btn.open:hover {
		opacity: 0.9;
	}

	.link-popup-btn.close {
		padding: var(--spacing-xs);
		margin-left: auto;
	}
</style>
