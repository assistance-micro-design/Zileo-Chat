<script lang="ts">
  import { onMount } from 'svelte';
  import { Sun, Moon, Settings, Bot } from 'lucide-svelte';
  import '../styles/global.css';
  import { theme } from '$lib/stores/theme';

  let { children } = $props();

  /**
   * Current theme value for reactive UI updates
   */
  let currentTheme = $state<'light' | 'dark'>('light');

  /**
   * Subscribe to theme changes
   */
  const unsubscribe = theme.subscribe((value) => {
    currentTheme = value;
  });

  onMount(() => {
    theme.init();
    return unsubscribe;
  });

  /**
   * Toggle theme between light and dark
   */
  function toggleTheme(): void {
    theme.toggle();
  }
</script>

<svelte:head>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
  <link
    href="https://fonts.googleapis.com/css2?family=Signika:wght@400;500;600;700&family=JetBrains+Mono&display=swap"
    rel="stylesheet"
  />
</svelte:head>

<div class="app-container">
  <nav class="floating-menu">
    <div class="flex items-center gap-md flex-1">
      <h1 class="app-title">Zileo Chat 3</h1>
    </div>
    <div class="flex items-center gap-md">
      <button
        type="button"
        class="btn btn-ghost btn-icon"
        onclick={toggleTheme}
        aria-label={currentTheme === 'light' ? 'Switch to dark mode' : 'Switch to light mode'}
      >
        {#if currentTheme === 'light'}
          <Moon size={18} />
        {:else}
          <Sun size={18} />
        {/if}
      </button>
      <a href="/settings" class="btn btn-secondary">
        <Settings size={16} />
        Configuration
      </a>
      <a href="/agent" class="btn btn-primary">
        <Bot size={16} />
        Agent
      </a>
    </div>
  </nav>

  <div class="main-content">
    {@render children()}
  </div>
</div>

<style>
  .app-title {
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }
</style>
