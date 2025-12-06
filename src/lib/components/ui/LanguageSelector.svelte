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
  LanguageSelector Component
  Compact language dropdown with flag display.
  Integrates with locale store for persistence.

  @example
  <LanguageSelector />
-->
<script lang="ts">
	import { i18n } from '$lib/i18n';
	import { localeStore, localeInfo } from '$lib/stores/locale';
	import { LOCALES, SUPPORTED_LOCALES, type Locale } from '$types/i18n';

	/**
	 * Dropdown open state
	 */
	let isOpen = $state(false);

	/**
	 * Current locale info (reactive)
	 */
	let currentInfo = $state(LOCALES.en);

	/**
	 * Subscribe to locale changes
	 */
	localeInfo.subscribe((info) => {
		currentInfo = info;
	});

	/**
	 * Toggle dropdown visibility
	 */
	function toggleDropdown(): void {
		isOpen = !isOpen;
	}

	/**
	 * Select a locale
	 */
	function selectLocale(locale: Locale): void {
		localeStore.setLocale(locale);
		isOpen = false;
	}

	/**
	 * Close dropdown on outside click
	 */
	function handleClickOutside(event: MouseEvent): void {
		const target = event.target as Element;
		if (!target.closest('.language-selector')) {
			isOpen = false;
		}
	}

	/**
	 * Handle keyboard navigation
	 */
	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape' && isOpen) {
			isOpen = false;
		}
	}

	/**
	 * Get flag display for locale
	 */
	function getFlagDisplay(countryCode: string): string {
		const flags: Record<string, string> = {
			GB: 'EN',
			FR: 'FR'
		};
		return flags[countryCode] || countryCode;
	}
</script>

<svelte:window onclick={handleClickOutside} onkeydown={handleKeydown} />

<div class="language-selector">
	<button
		type="button"
		class="btn btn-ghost btn-icon language-btn"
		onclick={toggleDropdown}
		aria-label={$i18n('ui_language_select')}
		aria-expanded={isOpen}
		aria-haspopup="listbox"
	>
		<span class="flag-display">{getFlagDisplay(currentInfo.countryCode)}</span>
	</button>

	{#if isOpen}
		<ul class="dropdown" role="listbox" aria-label={$i18n('ui_language_available')}>
			{#each SUPPORTED_LOCALES as loc}
				{@const info = LOCALES[loc]}
				<li role="option" aria-selected={loc === currentInfo.id}>
					<button
						type="button"
						class="dropdown-item"
						class:active={loc === currentInfo.id}
						onclick={() => selectLocale(loc)}
					>
						<span class="flag-display">{getFlagDisplay(info.countryCode)}</span>
						<span class="locale-name">{info.nativeName}</span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.language-selector {
		position: relative;
		display: inline-block;
	}

	.language-btn {
		min-width: 2.5rem;
	}

	.flag-display {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		line-height: 1;
	}

	.dropdown {
		position: absolute;
		top: 100%;
		right: 0;
		margin-top: var(--spacing-xs);
		padding: var(--spacing-xs);
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-lg);
		min-width: 140px;
		z-index: var(--z-dropdown);
		list-style: none;
	}

	.dropdown-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		width: 100%;
		padding: var(--spacing-sm) var(--spacing-md);
		background: transparent;
		border: none;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
		transition: background-color var(--transition-fast);
	}

	.dropdown-item:hover {
		background: var(--color-bg-tertiary);
	}

	.dropdown-item.active {
		background: var(--color-accent);
		color: var(--color-text-on-accent);
	}

	.locale-name {
		flex: 1;
		text-align: left;
	}
</style>
