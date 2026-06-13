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
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

ImportExportSteps - Horizontal wizard progress indicator shared by the export
and import panels. A completed step shows a check on a soft success disc, the
current step a brand-gradient disc with a glow, and upcoming steps a plain
outlined number. Steps are joined by thin connector lines.
-->

<script lang="ts">
	import { Check } from '@lucide/svelte';

	/** One step of the wizard progress bar. */
	export interface WizardStepItem {
		/** Visible label for the step */
		label: string;
		/** Progress state of the step */
		state: 'done' | 'current' | 'upcoming';
	}

	interface Props {
		/** Ordered list of steps to render */
		steps: WizardStepItem[];
	}

	let { steps }: Props = $props();
</script>

<div class="steps">
	{#each steps as step, i (step.label)}
		{#if i > 0}
			<span class="lnk" aria-hidden="true"></span>
		{/if}
		<span class="step" class:done={step.state === 'done'} class:current={step.state === 'current'}>
			<span class="num">
				{#if step.state === 'done'}
					<Check size={14} aria-hidden="true" />
				{:else}
					{i + 1}
				{/if}
			</span>
			{step.label}
		</span>
	{/each}
</div>

<style>
	.steps {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
	}

	.step {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.num {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		border-radius: 50%;
		border: 1px solid var(--color-border-dark);
		font-weight: var(--font-weight-semibold);
		font-size: var(--font-size-2xs);
	}

	.step.done .num {
		background: var(--color-success-light);
		color: var(--color-success);
		border-color: var(--color-success-border);
	}

	.step.current {
		color: var(--color-text-primary);
		font-weight: var(--font-weight-semibold);
	}

	.step.current .num {
		background: var(--gradient-brand);
		color: var(--color-accent-text);
		border: none;
		box-shadow: var(--glow-accent-soft);
	}

	.lnk {
		flex: 0 0 26px;
		height: 1px;
		background: var(--color-border);
	}
</style>
