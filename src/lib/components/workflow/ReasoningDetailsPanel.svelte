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
  ReasoningDetailsPanel Component
  Displays full plain-text reasoning content in an expandable panel.
  Styled consistently with ActivityItemDetails (border-left accent, slideDown).

  @example
  <ReasoningDetailsPanel content={activity.metadata.content} />
-->
<script lang="ts">
	interface Props {
		/** Full reasoning content text */
		content: string;
	}

	let { content }: Props = $props();
</script>

<div class="reasoning-details" role="region" aria-label="Reasoning content">
	<pre class="reasoning-content">{content}</pre>
</div>

<style>
	.reasoning-details {
		display: flex;
		flex-direction: column;
		margin-top: var(--spacing-sm);
		padding: var(--spacing-sm);
		background: var(--color-bg-tertiary);
		border-radius: var(--radius-sm);
		border-left: 2px solid var(--color-accent);
		animation: slideDown 150ms ease-out;
	}

	@keyframes slideDown {
		from {
			opacity: 0;
			transform: translateY(-4px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.reasoning-content {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		white-space: pre-wrap;
		word-break: break-word;
		margin: 0;
		max-height: 300px;
		overflow-y: auto;
		line-height: 1.5;
		font-family: inherit;
	}

	.reasoning-content::-webkit-scrollbar {
		width: 4px;
	}

	.reasoning-content::-webkit-scrollbar-track {
		background: transparent;
	}

	.reasoning-content::-webkit-scrollbar-thumb {
		background: var(--color-border);
		border-radius: var(--radius-full);
	}
</style>
