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
  AppContainer Component
  Root layout container that provides the main app structure.
  Uses a flex column layout to organize FloatingMenu and main content.

  @example
  <AppContainer>
    <FloatingMenu />
    <div class="main-content">
      <Sidebar />
      <main class="content-area">Content</main>
    </div>
  </AppContainer>
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	/**
	 * AppContainer props
	 */
	interface Props {
		/** Main content slot */
		children: Snippet;
	}

	let { children }: Props = $props();
</script>

<div class="app-container">
	{@render children()}
</div>
