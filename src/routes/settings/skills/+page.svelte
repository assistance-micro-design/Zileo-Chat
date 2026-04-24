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
Settings > Skills Page
Manages skill library configuration.
-->

<script lang="ts">
	import { SkillSettings } from '$lib/components/settings/skills';
	import { skillStore } from '$lib/stores/skills';
	import { onSettingsRefresh } from '$lib/utils/settings-refresh';

	onSettingsRefresh(() => skillStore.loadSkills());
</script>

<section class="settings-section">
	<SkillSettings />
</section>
