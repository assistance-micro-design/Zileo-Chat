<!--
  ConnectionTester Component
  Tests the connection to a LLM provider and displays the result.

  @example
  <ConnectionTester provider="mistral" />
  <ConnectionTester provider="ollama" />
-->
<script lang="ts">
	import { Button, Spinner, StatusIndicator } from '$lib/components/ui';
	import { testConnection } from '$lib/stores/llm';
	import type { ProviderType, ConnectionTestResult } from '$types/llm';

	/**
	 * ConnectionTester props
	 */
	interface Props {
		/** Provider to test connection for */
		provider: ProviderType;
		/** Whether the test button should be disabled */
		disabled?: boolean;
	}

	let { provider, disabled = false }: Props = $props();

	/** Whether a test is currently running */
	let testing = $state(false);
	/** Result of the last connection test */
	let result = $state<ConnectionTestResult | null>(null);

	/**
	 * Handles the connection test action.
	 * Calls the backend to test the provider connection.
	 */
	async function handleTest(): Promise<void> {
		testing = true;
		result = null;

		try {
			result = await testConnection(provider);
		} catch (err) {
			result = {
				provider,
				success: false,
				latency_ms: null,
				error_message: err instanceof Error ? err.message : String(err),
				model_tested: null
			};
		} finally {
			testing = false;
		}
	}

	/**
	 * Formats latency for display
	 */
	function formatLatency(ms: number | null): string {
		if (ms === null) return '';
		return `${ms}ms`;
	}
</script>

<div class="connection-tester">
	<Button variant="ghost" size="sm" onclick={handleTest} disabled={testing || disabled}>
		{#if testing}
			<span class="tester-loading">
				<Spinner size="sm" />
				<span>Testing...</span>
			</span>
		{:else}
			Test Connection
		{/if}
	</Button>

	{#if result}
		<div class="test-result" class:success={result.success} class:error={!result.success}>
			<StatusIndicator status={result.success ? 'completed' : 'error'} size="sm" />
			{#if result.success}
				<span class="result-text">Connected ({formatLatency(result.latency_ms)})</span>
			{:else}
				<span class="result-text">{result.error_message || 'Connection failed'}</span>
			{/if}
		</div>
	{/if}
</div>

<style>
	.connection-tester {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.tester-loading {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.test-result {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		padding: var(--spacing-xs) var(--spacing-sm);
		border-radius: var(--radius-sm);
		font-size: var(--font-size-sm);
	}

	.test-result.success {
		background-color: var(--color-success-bg);
		color: var(--color-success);
	}

	.test-result.error {
		background-color: var(--color-error-bg);
		color: var(--color-error);
	}

	.result-text {
		word-break: break-word;
	}
</style>
