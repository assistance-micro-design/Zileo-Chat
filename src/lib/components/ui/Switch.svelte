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
  Switch Component
  Accessible on/off toggle for boolean settings ("enable X") and per-agent
  grants (tools, MCP servers, skills), replacing bare checkboxes on settings
  surfaces. Built on a native button with role="switch" + aria-checked, so
  Space/Enter activation comes for free — no custom keyboard state machine.
  Multi-selection pickers (export entity pickers) stay checkboxes by design.

  @example
  <Switch checked={enabled} onchange={(v) => (enabled = v)} ariaLabel="Enable dictation" />
-->
<script lang="ts">
	interface Props {
		/** Current on/off state */
		checked: boolean;
		/** Called with the requested new state when the user toggles */
		onchange?: (checked: boolean) => void;
		/** Disables interaction */
		disabled?: boolean;
		/** Accessible label (use when no visible label references the switch) */
		ariaLabel?: string;
		/** id of the visible element labelling the switch */
		labelledBy?: string;
	}

	let { checked, onchange, disabled = false, ariaLabel, labelledBy }: Props = $props();
</script>

<button
	type="button"
	class="switch"
	class:on={checked}
	role="switch"
	aria-checked={checked}
	aria-label={ariaLabel}
	aria-labelledby={labelledBy}
	{disabled}
	onclick={() => onchange?.(!checked)}
></button>

<style>
	.switch {
		position: relative;
		display: inline-flex;
		flex-shrink: 0;
		width: 38px;
		height: 22px;
		padding: 0;
		background: var(--color-bg-active);
		border-radius: var(--border-radius-full);
		border: 1px solid var(--color-border);
		cursor: pointer;
		transition:
			background-color var(--transition-fast),
			box-shadow var(--transition-fast);
	}

	.switch::after {
		content: '';
		position: absolute;
		top: 2px;
		left: 2px;
		width: 16px;
		height: 16px;
		border-radius: 50%;
		background: var(--surface-1);
		box-shadow: var(--shadow-xs);
		transition: transform var(--transition-fast);
	}

	.switch.on {
		background: var(--gradient-brand);
		box-shadow: var(--glow-accent-soft);
		border-color: transparent;
	}

	.switch.on::after {
		transform: translateX(16px);
	}

	.switch:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.switch:focus-visible {
		outline: none;
		box-shadow:
			0 0 0 3px var(--color-accent-light),
			0 0 0 1px var(--color-accent-deep);
	}
</style>
