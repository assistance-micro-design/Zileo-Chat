<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';

  onMount(() => {
    goto('/agent');
  });
</script>

<div class="loading">
  <p>Loading...</p>
</div>

<style>
  .loading {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
  }
</style>
