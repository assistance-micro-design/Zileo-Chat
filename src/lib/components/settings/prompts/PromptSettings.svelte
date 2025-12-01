<script lang="ts">
  import { onMount } from 'svelte';
  import { promptStore, prompts, promptLoading, promptError, promptFormMode, editingPrompt } from '$lib/stores/prompts';
  import type { PromptCreate } from '$types/prompt';
  import PromptList from './PromptList.svelte';
  import PromptForm from './PromptForm.svelte';

  let saving = $state(false);

  onMount(() => {
    promptStore.loadPrompts();
  });

  async function handleCreate(data: PromptCreate) {
    saving = true;
    try {
      await promptStore.createPrompt(data);
    } catch (e) {
      console.error('Failed to create prompt:', e);
    } finally {
      saving = false;
    }
  }

  async function handleUpdate(data: PromptCreate) {
    if (!$editingPrompt) return;
    saving = true;
    try {
      await promptStore.updatePrompt($editingPrompt.id, data);
    } catch (e) {
      console.error('Failed to update prompt:', e);
    } finally {
      saving = false;
    }
  }

  async function handleDelete(id: string) {
    try {
      await promptStore.deletePrompt(id);
    } catch (e) {
      console.error('Failed to delete prompt:', e);
    }
  }

  function handleSave(data: PromptCreate) {
    if ($promptFormMode === 'create') {
      handleCreate(data);
    } else {
      handleUpdate(data);
    }
  }
</script>

<div class="prompt-settings">
  {#if $promptError}
    <div class="error-banner">
      <span>{$promptError}</span>
      <button type="button" onclick={() => promptStore.clearError()}>Dismiss</button>
    </div>
  {/if}

  {#if $promptFormMode}
    <PromptForm
      mode={$promptFormMode}
      prompt={$editingPrompt}
      {saving}
      onsave={handleSave}
      oncancel={() => promptStore.closeForm()}
    />
  {:else}
    <PromptList
      prompts={$prompts}
      loading={$promptLoading}
      oncreate={() => promptStore.openCreateForm()}
      onedit={(id) => promptStore.openEditForm(id)}
      ondelete={handleDelete}
    />
  {/if}
</div>

<style>
  .prompt-settings {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .error-banner {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--space-3) var(--space-4);
    background: var(--color-error-subtle);
    border: 1px solid var(--color-error);
    border-radius: var(--radius-md);
    color: var(--color-error);
  }

  .error-banner button {
    background: none;
    border: none;
    color: inherit;
    cursor: pointer;
    font-size: var(--font-sm);
    text-decoration: underline;
  }

  .error-banner button:hover {
    opacity: 0.8;
  }
</style>
