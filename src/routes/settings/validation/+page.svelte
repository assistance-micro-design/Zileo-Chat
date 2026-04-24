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
	import { ValidationSettings } from '$lib/components/settings/validation';
	import SettingsSectionHeader from '$lib/components/settings/SettingsSectionHeader.svelte';
	import { validationSettingsStore } from '$lib/stores/validation-settings';
	import { onSettingsRefresh } from '$lib/utils/settings-refresh';

	onSettingsRefresh(() => validationSettingsStore.loadSettings());
</script>

<section class="settings-section">
	<SettingsSectionHeader
		titleKey="settings_validation"
		descriptionKey="validation_description"
		helpTitleKey="help_validation_title"
		helpDescriptionKey="help_validation_description"
		helpTutorialKey="help_validation_tutorial"
	/>
	<ValidationSettings />
</section>
