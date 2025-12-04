<script lang="ts">
	import { i18n } from '$lib/i18n';
</script>

<svelte:head>
  <meta http-equiv="refresh" content="0;url=/agent" />
</svelte:head>

<div class="loading">
  <p>{$i18n('page_redirecting')}</p>
</div>

<style>
  .loading {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    color: var(--color-text-secondary);
  }
</style>
