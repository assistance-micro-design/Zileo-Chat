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
  ToastContainer Component
  Renders toast notifications in a fixed overlay at the bottom-right corner.
  Toasts stack vertically with the most recent at the bottom.
-->

<script lang="ts">
	import { visibleToasts, toastStore } from '$lib/stores/toast';
	import ToastItem from './ToastItem.svelte';

	function handleNavigate(workflowId: string): void {
		toastStore.requestNavigation(workflowId);
		toastStore.dismissForWorkflow(workflowId);
	}
</script>

{#if $visibleToasts.length > 0}
	<div class="toast-container" aria-live="polite" role="status">
		{#each $visibleToasts as toast (toast.id)}
			<ToastItem {toast} onnavigate={handleNavigate} />
		{/each}
	</div>
{/if}

<style>
	.toast-container {
		position: fixed;
		bottom: var(--spacing-lg);
		right: var(--spacing-lg);
		display: flex;
		flex-direction: column-reverse;
		gap: var(--spacing-sm);
		z-index: 9999;
		pointer-events: none;
		max-width: 400px;
	}
</style>
