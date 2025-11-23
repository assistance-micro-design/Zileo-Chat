<script lang="ts">
  import '../styles/global.css';

  let { children } = $props();
</script>

<div class="app-container">
  <nav class="floating-menu">
    <a href="/agent">Agent</a>
    <a href="/settings">Settings</a>
  </nav>

  <main>
    {@render children()}
  </main>
</div>

<style>
  :global(body) {
    margin: 0;
    font-family: 'Inter', system-ui, sans-serif;
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
  }

  .app-container {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .floating-menu {
    display: flex;
    gap: 1rem;
    padding: 1rem;
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
  }

  .floating-menu a {
    padding: 0.5rem 1rem;
    text-decoration: none;
    color: var(--color-text-primary);
    border-radius: 0.375rem;
    transition: background 0.2s;
  }

  .floating-menu a:hover {
    background: var(--color-bg-hover);
  }

  main {
    flex: 1;
    overflow: auto;
  }
</style>
