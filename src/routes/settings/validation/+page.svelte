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
Settings > Validation Page
Manages validation settings configuration.
-->

<script lang="ts">
	import { HelpButton } from '$lib/components/ui';
	import { ValidationSettings } from '$lib/components/settings/validation';
	import { i18n } from '$lib/i18n';
</script>

<section class="settings-section">
	<div class="section-title-row">
		<h2 class="section-title">{$i18n('settings_validation')}</h2>
		<HelpButton
			titleKey="help_validation_title"
			descriptionKey="help_validation_description"
			tutorialKey="help_validation_tutorial"
		/>
	</div>
	<p class="section-description">
		{$i18n('validation_description')}
	</p>
	<ValidationSettings />
</section>

<style>
	.settings-section {
		margin-bottom: var(--spacing-2xl);
		padding-bottom: var(--spacing-xl);
	}

	.section-title {
		font-size: var(--font-size-2xl);
		font-weight: var(--font-weight-semibold);
		margin-bottom: var(--spacing-lg);
	}

	.section-title-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-lg);
	}

	.section-title-row .section-title {
		margin-bottom: 0;
	}

	.section-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin-bottom: var(--spacing-lg);
		line-height: var(--line-height-relaxed);
	}
</style>
