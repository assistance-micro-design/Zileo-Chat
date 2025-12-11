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

MCPServerTester Component
Displays MCP server connection test results including tools, resources, and latency.

@example
<MCPServerTester
  result={testResult}
  loading={isTestRunning}
  onRetry={handleRetry}
/>
-->
<script lang="ts">
	import type { MCPTestResult } from '$types/mcp';
	import { Button, Badge, Spinner } from '$lib/components/ui';
	import { CheckCircle2, XCircle, RefreshCw, Wrench, FileText, Clock } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * MCPServerTester props
	 */
	interface Props {
		/** Test result data (null if no test run yet) */
		result: MCPTestResult | null;
		/** Whether a test is currently running */
		loading?: boolean;
		/** Handler for retry action */
		onRetry?: () => void;
		/** Error message if test failed before getting result */
		error?: string | null;
	}

	let {
		result,
		loading = false,
		onRetry,
		error = null
	}: Props = $props();

	/**
	 * Formats latency in a human-readable way
	 */
	function formatLatency(ms: number): string {
		if (ms < 1000) {
			return `${ms}ms`;
		}
		return `${(ms / 1000).toFixed(2)}s`;
	}

	/**
	 * Truncates tool description for display
	 */
	function truncateDescription(desc: string, maxLength: number = 80): string {
		if (desc.length <= maxLength) {
			return desc;
		}
		return desc.slice(0, maxLength - 3) + '...';
	}
</script>

<div class="tester-container">
	{#if loading}
		<div class="tester-loading">
			<Spinner size="md" />
			<span class="loading-text">{$i18n('mcp_tester_loading')}</span>
		</div>
	{:else if error}
		<div class="tester-error">
			<div class="error-header">
				<XCircle size={24} class="error-icon" />
				<span class="error-title">{$i18n('mcp_tester_failed')}</span>
			</div>
			<p class="error-message">{error}</p>
			{#if onRetry}
				<Button variant="ghost" size="sm" onclick={onRetry}>
					<RefreshCw size={16} />
					<span>{$i18n('mcp_tester_retry')}</span>
				</Button>
			{/if}
		</div>
	{:else if result}
		<div class="tester-result" class:success={result.success} class:failure={!result.success}>
			<div class="result-header">
				{#if result.success}
					<CheckCircle2 size={24} class="success-icon" />
					<span class="result-title">{$i18n('mcp_tester_success')}</span>
				{:else}
					<XCircle size={24} class="error-icon" />
					<span class="result-title">{$i18n('mcp_tester_failure')}</span>
				{/if}
				<Badge variant={result.success ? 'success' : 'error'}>
					<Clock size={12} />
					{formatLatency(result.latency_ms)}
				</Badge>
			</div>

			<p class="result-message">{result.message}</p>

			{#if result.success}
				<div class="result-details">
					<!-- Tools Section -->
					<div class="detail-section">
						<div class="section-header">
							<Wrench size={16} />
							<span class="section-title">{$i18n('mcp_tester_tools')} ({result.tools.length})</span>
						</div>
						{#if result.tools.length === 0}
							<p class="empty-list">{$i18n('mcp_tester_tools_empty')}</p>
						{:else}
							<ul class="tool-list">
								{#each result.tools as tool}
									<li class="tool-item">
										<span class="tool-name">{tool.name}</span>
										<span class="tool-description">
											{truncateDescription(tool.description)}
										</span>
									</li>
								{/each}
							</ul>
						{/if}
					</div>

					<!-- Resources Section -->
					<div class="detail-section">
						<div class="section-header">
							<FileText size={16} />
							<span class="section-title">{$i18n('mcp_tester_resources')} ({result.resources.length})</span>
						</div>
						{#if result.resources.length === 0}
							<p class="empty-list">{$i18n('mcp_tester_resources_empty')}</p>
						{:else}
							<ul class="resource-list">
								{#each result.resources as resource}
									<li class="resource-item">
										<span class="resource-name">{resource.name}</span>
										<span class="resource-uri">{resource.uri}</span>
									</li>
								{/each}
							</ul>
						{/if}
					</div>
				</div>
			{/if}

			{#if onRetry}
				<div class="result-actions">
					<Button variant="ghost" size="sm" onclick={onRetry}>
						<RefreshCw size={16} />
						<span>{$i18n('mcp_tester_test_again')}</span>
					</Button>
				</div>
			{/if}
		</div>
	{:else}
		<div class="tester-empty">
			<p class="empty-text">{$i18n('mcp_tester_empty')}</p>
		</div>
	{/if}
</div>

<style>
	.tester-container {
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		overflow: hidden;
	}

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

	.tester-error,
	.tester-result {
		padding: var(--spacing-lg);
	}

	.tester-error {
		background: var(--color-error-light);
	}

	.error-header,
	.result-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-md);
	}

	.error-header :global(.error-icon),
	.result-header :global(.error-icon) {
		color: var(--color-error);
	}

	.result-header :global(.success-icon) {
		color: var(--color-success);
	}

	.error-title,
	.result-title {
		font-weight: var(--font-weight-semibold);
		flex: 1;
	}

	.error-message,
	.result-message {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin-bottom: var(--spacing-md);
	}

	.tester-error :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.tester-result.success {
		background: var(--color-success-light);
	}

	.tester-result.failure {
		background: var(--color-error-light);
	}

	.result-details {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
		margin-top: var(--spacing-lg);
		padding-top: var(--spacing-lg);
		border-top: 1px solid var(--color-border);
	}

	.detail-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.section-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		color: var(--color-text-secondary);
	}

	.section-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.empty-list {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.tool-list,
	.resource-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.tool-item,
	.resource-item {
		display: flex;
		flex-direction: column;
		padding: var(--spacing-sm);
		background: var(--color-bg-primary);
		border-radius: var(--border-radius-sm);
	}

	.tool-name,
	.resource-name {
		font-family: var(--font-family-mono);
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
	}

	.tool-description,
	.resource-uri {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.resource-uri {
		font-family: var(--font-family-mono);
	}

	.result-actions {
		margin-top: var(--spacing-md);
		padding-top: var(--spacing-md);
		border-top: 1px solid var(--color-border);
	}

	.result-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.tester-empty {
		padding: var(--spacing-xl);
		text-align: center;
	}

	.empty-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}
</style>
