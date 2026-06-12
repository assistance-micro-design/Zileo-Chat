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
MCPServerTester Component
Displays MCP server connection test results: status badge, discovered tools
as monospace badges on the MCP channel, resources, and the error output in a
monospace panel on failure. The retry action lives in the host modal footer.

@example
<MCPServerTester
  result={testResult}
  loading={isTestRunning}
  error={connectError}
/>
-->
<script lang="ts">
	import type { MCPTestResult } from '$types/mcp';
	import { Badge, Spinner } from '$lib/components/ui';
	import { CircleCheck, CircleX, Clock } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * MCPServerTester props
	 */
	interface Props {
		/** Test result data (null if no test run yet) */
		result: MCPTestResult | null;
		/** Whether a test is currently running */
		loading?: boolean;
		/** Error message if test failed before getting result */
		error?: string | null;
	}

	let { result, loading = false, error = null }: Props = $props();

	/**
	 * Formats latency in a human-readable way
	 */
	function formatLatency(ms: number): string {
		if (ms < 1000) {
			return `${ms}ms`;
		}
		return `${(ms / 1000).toFixed(2)}s`;
	}
</script>

<div class="tester-container">
	{#if loading}
		<div class="tester-loading">
			<Spinner size="md" />
			<span class="loading-text">{$i18n('mcp_tester_loading')}</span>
		</div>
	{:else if error}
		<div class="tester-result">
			<div class="result-header">
				<Badge variant="error">
					<CircleX size={12} />
					{$i18n('mcp_tester_failed')}
				</Badge>
			</div>
			<div class="code-panel error-text">{error}</div>
		</div>
	{:else if result}
		<div class="tester-result">
			<div class="result-header">
				{#if result.success}
					<Badge variant="success">
						<CircleCheck size={12} />
						{$i18n('mcp_tester_success')}
					</Badge>
				{:else}
					<Badge variant="error">
						<CircleX size={12} />
						{$i18n('mcp_tester_failure')}
					</Badge>
				{/if}
				<Badge variant="neutral">
					<Clock size={12} />
					{formatLatency(result.latency_ms)}
				</Badge>
			</div>

			{#if result.success}
				<div class="detail-section">
					<span class="section-label">
						{$i18n('mcp_tester_tools')} ({result.tools.length})
					</span>
					{#if result.tools.length === 0}
						<p class="empty-list">{$i18n('mcp_tester_tools_empty')}</p>
					{:else}
						<div class="badge-wrap">
							{#each result.tools as tool (tool.name)}
								<span class="badge badge-mcp mono-badge" title={tool.description}>{tool.name}</span>
							{/each}
						</div>
					{/if}
				</div>

				<div class="detail-section">
					<span class="section-label">
						{$i18n('mcp_tester_resources')} ({result.resources.length})
					</span>
					{#if result.resources.length === 0}
						<p class="empty-list">{$i18n('mcp_tester_resources_empty')}</p>
					{:else}
						<div class="badge-wrap">
							{#each result.resources as resource (resource.uri)}
								<span class="badge badge-mcp mono-badge" title={resource.uri}>{resource.name}</span>
							{/each}
						</div>
					{/if}
				</div>
			{:else}
				<div class="code-panel error-text">{result.message}</div>
			{/if}
		</div>
	{:else}
		<div class="tester-empty">
			<p class="empty-text">{$i18n('mcp_tester_empty')}</p>
		</div>
	{/if}
</div>

<style>
	.tester-loading {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-xl);
		gap: var(--spacing-md);
	}

	.loading-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.tester-result {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.result-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.detail-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.section-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.badge-wrap {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-xs);
	}

	.mono-badge {
		font-family: var(--font-mono);
		font-weight: var(--font-weight-medium);
	}

	.empty-list {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin: 0;
	}

	.code-panel {
		padding: var(--spacing-md);
		font-family: var(--font-mono);
		font-size: var(--font-size-xs);
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		white-space: pre-wrap;
		word-break: break-word;
	}

	.code-panel.error-text {
		color: var(--color-error);
		border-color: var(--color-error-border);
	}

	.tester-empty {
		padding: var(--spacing-xl);
		text-align: center;
	}

	.empty-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
	}
</style>
