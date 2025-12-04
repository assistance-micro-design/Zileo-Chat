<script lang="ts">
	/**
	 * Step 3: Welcome message
	 * Introduction to Zileo Chat
	 */
	import { i18n } from '$lib/i18n';

	interface Props {
		onNext?: () => void;
	}

	let { onNext: _onNext }: Props = $props();
</script>

<div class="step-welcome" data-step="welcome">
	<div class="logo-container">
		<div class="logo">Z</div>
	</div>

	<h1 class="step-title">{$i18n('onboarding_welcome_title')}</h1>
	<p class="step-description">{$i18n('onboarding_welcome_description')}</p>
</div>

<style>
	.step-welcome {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-lg);
	}

	.logo-container {
		margin-bottom: var(--spacing-md);
	}

	.logo {
		width: 80px;
		height: 80px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: linear-gradient(135deg, var(--color-primary) 0%, var(--color-primary-dark, #4f46e5) 100%);
		border-radius: var(--radius-xl);
		font-size: 2.5rem;
		font-weight: var(--font-weight-bold);
		color: white;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
	}

	.step-title {
		font-size: var(--font-size-2xl);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.step-description {
		font-size: var(--font-size-md);
		color: var(--color-text-secondary);
		margin: 0;
		max-width: 400px;
	}
</style>
