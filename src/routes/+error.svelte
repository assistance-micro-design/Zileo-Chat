<!--
  Copyright 2025 Assistance Micro Design

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0
-->

<!--
  Global error boundary. SvelteKit renders this when a `+page.ts` `load`
  throws or when a route imports fail. Without it, the user sees the
  default SvelteKit error template (no theme, no i18n, no opener plugin).

  Note: this only catches load/route errors. Per-component errors still
  propagate up; reactive `$effect` exceptions need their own try/catch.
-->
<script lang="ts">
	import { page } from '$app/state';
	import { i18n } from '$lib/i18n';
	import { Button } from '$lib/components/ui';
	import { goto } from '$app/navigation';

	function handleRetry(): void {
		// Reload the current route. Equivalent to F5 but stays in-app.
		window.location.reload();
	}

	function handleHome(): void {
		void goto('/');
	}
</script>

<div class="error-page">
	<div class="error-card">
		<div class="error-status">
			<span class="error-code">{page.status}</span>
		</div>
		<h1 class="error-title">{$i18n('error_page_title')}</h1>
		<p class="error-message">{page.error?.message ?? $i18n('error_page_unknown')}</p>
		<div class="error-actions">
			<Button variant="ghost" onclick={handleRetry}>
				{$i18n('error_page_retry')}
			</Button>
			<Button variant="primary" onclick={handleHome}>
				{$i18n('error_page_home')}
			</Button>
		</div>
	</div>
</div>

<style>
	.error-page {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 100vh;
		padding: var(--spacing-xl);
		background: var(--color-bg-primary);
	}

	.error-card {
		max-width: 480px;
		padding: var(--spacing-xl);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-lg);
		box-shadow: var(--shadow-lg);
		text-align: center;
	}

	.error-status {
		font-size: 4rem;
		font-weight: var(--font-weight-bold);
		color: var(--color-text-tertiary);
		line-height: 1;
		margin-bottom: var(--spacing-lg);
	}

	.error-code {
		font-variant-numeric: tabular-nums;
	}

	.error-title {
		font-size: var(--font-size-xl);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0 0 var(--spacing-md);
	}

	.error-message {
		color: var(--color-text-secondary);
		font-size: var(--font-size-base);
		line-height: 1.5;
		margin: 0 0 var(--spacing-xl);
	}

	.error-actions {
		display: flex;
		justify-content: center;
		gap: var(--spacing-md);
	}
</style>
