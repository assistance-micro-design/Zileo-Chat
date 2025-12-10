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
Settings > Import/Export Page
Manages settings import and export functionality.
-->

<script lang="ts">
	import { ImportExportSettings } from '$lib/components/settings/import-export';

	/**
	 * Dispatches custom event to notify other settings pages to refresh.
	 * Used after import operation completes.
	 */
	function handleImportRefresh(): void {
		// Only dispatch events in browser context
		if (typeof window !== 'undefined') {
			window.dispatchEvent(new CustomEvent('settings:refresh'));
		}
	}
</script>

<section class="settings-section">
	<ImportExportSettings onRefreshNeeded={handleImportRefresh} />
</section>

<style>
	.settings-section {
		margin-bottom: var(--spacing-2xl);
		padding-bottom: var(--spacing-xl);
	}
</style>
