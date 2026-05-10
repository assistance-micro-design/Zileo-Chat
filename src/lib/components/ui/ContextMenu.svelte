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
  ContextMenu Component
  Reusable context menu with keyboard navigation, positioning, and accessibility.

  @example
  <ContextMenu items={menuItems} x={100} y={200} onselect={handleAction} onclose={handleClose} />
-->
<script lang="ts">
	import type { ContextMenuItem } from '$types/sidebar';
	import { adjustMenuPosition, getNextFocusableIndex } from '$lib/utils/contextMenu';
	import { i18n } from '$lib/i18n';
	import { tick } from 'svelte';

	interface Props {
		/** Menu items to display */
		items: ContextMenuItem[];
		/** X position in pixels */
		x: number;
		/** Y position in pixels */
		y: number;
		/** Handler when an item is selected */
		onselect: (itemId: string) => void;
		/** Handler when the menu is closed */
		onclose: () => void;
	}

	let { items, x, y, onselect, onclose }: Props = $props();

	let menuRef = $state<HTMLDivElement | null>(null);
	let focusedIndex = $state(-1);

	const enabledItems = $derived(items.filter((item) => !item.disabled));

	/** Adjusted position to keep menu within viewport */
	let adjustedX = $state(0);
	let adjustedY = $state(0);

	$effect(() => {
		if (menuRef) {
			const rect = menuRef.getBoundingClientRect();
			const pos = adjustMenuPosition(
				x,
				y,
				rect.width,
				rect.height,
				window.innerWidth,
				window.innerHeight
			);
			adjustedX = pos.x;
			adjustedY = pos.y;
		}
	});

	$effect(() => {
		tick().then(() => menuRef?.focus());
	});

	/**
	 * Handle click outside to close the menu
	 */
	function handlePointerDown(event: PointerEvent): void {
		if (menuRef && !menuRef.contains(event.target as Node)) {
			onclose();
		}
	}

	$effect(() => {
		document.addEventListener('pointerdown', handlePointerDown, true);
		return () => document.removeEventListener('pointerdown', handlePointerDown, true);
	});

	/**
	 * Handle keyboard navigation
	 */
	function handleKeydown(event: KeyboardEvent): void {
		switch (event.key) {
			case 'ArrowDown':
				event.preventDefault();
				focusedIndex = getNextFocusableIndex(items, focusedIndex, 1);
				break;
			case 'ArrowUp':
				event.preventDefault();
				focusedIndex = getNextFocusableIndex(items, focusedIndex, -1);
				break;
			case 'Enter':
			case ' ': {
				event.preventDefault();
				const focused = items[focusedIndex];
				if (focused && !focused.disabled) {
					onselect(focused.id);
				}
				break;
			}
			case 'Escape':
				event.preventDefault();
				onclose();
				break;
			case 'Home': {
				event.preventDefault();
				const first = enabledItems[0];
				focusedIndex = first ? items.indexOf(first) : 0;
				break;
			}
			case 'End': {
				event.preventDefault();
				const last = enabledItems[enabledItems.length - 1];
				focusedIndex = last ? items.indexOf(last) : items.length - 1;
				break;
			}
		}
	}

	/**
	 * Handle item click
	 */
	function handleItemClick(item: ContextMenuItem): void {
		if (!item.disabled) {
			onselect(item.id);
		}
	}
</script>

<div
	bind:this={menuRef}
	class="context-menu"
	role="menu"
	tabindex="-1"
	style:left="{adjustedX}px"
	style:top="{adjustedY}px"
	onkeydown={handleKeydown}
>
	{#each items as item, index (item.id)}
		{#if item.separator}
			<div class="separator" role="separator"></div>
		{/if}
		<button
			type="button"
			class="menu-item"
			class:danger={item.variant === 'danger'}
			class:focused={index === focusedIndex}
			role="menuitem"
			disabled={item.disabled}
			aria-disabled={item.disabled}
			tabindex="-1"
			onclick={() => handleItemClick(item)}
			onpointerenter={() => (focusedIndex = index)}
		>
			{#if item.icon}
				<span class="menu-item-icon">
					<item.icon size={14} />
				</span>
			{/if}
			<span class="menu-item-label">{item.label ?? $i18n(item.labelKey ?? '')}</span>
		</button>
	{/each}
</div>

<style>
	.context-menu {
		position: fixed;
		z-index: 1000;
		min-width: 180px;
		max-width: 280px;
		padding: var(--spacing-xs) 0;
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		box-shadow:
			0 4px 12px rgba(0, 0, 0, 0.15),
			0 1px 3px rgba(0, 0, 0, 0.1);
		outline: none;
	}

	.separator {
		height: 1px;
		margin: var(--spacing-xs) 0;
		background: var(--color-border);
	}

	.menu-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		width: 100%;
		padding: var(--spacing-sm) var(--spacing-md);
		font-size: var(--font-size-sm);
		font-family: var(--font-family);
		color: var(--color-text-primary);
		background: transparent;
		border: none;
		cursor: pointer;
		text-align: left;
		transition: background var(--transition-fast);
	}

	.menu-item:hover:not(:disabled),
	.menu-item.focused:not(:disabled) {
		background: var(--color-bg-hover);
	}

	.menu-item:disabled {
		color: var(--color-text-tertiary);
		cursor: not-allowed;
		opacity: 0.5;
	}

	.menu-item.danger {
		color: var(--color-error);
	}

	.menu-item.danger:hover:not(:disabled),
	.menu-item.danger.focused:not(:disabled) {
		background: var(--color-error-light);
	}

	.menu-item-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.menu-item-label {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
