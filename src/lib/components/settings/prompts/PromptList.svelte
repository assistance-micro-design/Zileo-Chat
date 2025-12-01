<script lang="ts">
  import { Button, Card, Badge, Input, Select, Spinner } from '$lib/components/ui';
  import type { PromptSummary, PromptCategory } from '$types/prompt';
  import { PROMPT_CATEGORY_LABELS } from '$types/prompt';

  interface Props {
    prompts: PromptSummary[];
    loading?: boolean;
    oncreate?: () => void;
    onedit?: (id: string) => void;
    ondelete?: (id: string) => void;
  }

  let { prompts, loading = false, oncreate, onedit, ondelete }: Props = $props();

  // Filter state
  let searchQuery = $state('');
  let categoryFilter = $state<PromptCategory | ''>('');

  // Category options with "All" option
  const categoryOptions = [
    { value: '', label: 'All Categories' },
    ...Object.entries(PROMPT_CATEGORY_LABELS).map(([value, label]) => ({
      value: value as PromptCategory,
      label
    }))
  ];

  // Filtered prompts
  let filteredPrompts = $derived.by(() => {
    let result = prompts;

    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      result = result.filter(
        (p) =>
          p.name.toLowerCase().includes(query) ||
          p.description.toLowerCase().includes(query)
      );
    }

    if (categoryFilter) {
      result = result.filter((p) => p.category === categoryFilter);
    }

    return result;
  });

  function formatDate(dateStr: string): string {
    return new Date(dateStr).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric'
    });
  }

  function handleEdit(id: string) {
    onedit?.(id);
  }

  function handleDelete(id: string) {
    if (confirm('Are you sure you want to delete this prompt?')) {
      ondelete?.(id);
    }
  }
</script>

<div class="prompt-list">
  <div class="list-header">
    <div class="filters">
      <Input
        placeholder="Search prompts..."
        value={searchQuery}
        oninput={(e) => searchQuery = e.currentTarget.value}
      />
      <Select
        value={categoryFilter}
        onchange={(e) => categoryFilter = e.currentTarget.value as PromptCategory | ''}
        options={categoryOptions}
      />
    </div>
    <Button variant="primary" onclick={oncreate} disabled={loading}>
      Create Prompt
    </Button>
  </div>

  {#if loading}
    <div class="loading-state">
      <Spinner size="lg" />
      <span>Loading prompts...</span>
    </div>
  {:else if filteredPrompts.length === 0}
    <div class="empty-state">
      {#if prompts.length === 0}
        <p class="empty-title">No prompts yet</p>
        <p class="empty-description">Create your first prompt template to get started.</p>
        <Button variant="primary" onclick={oncreate}>Create Prompt</Button>
      {:else}
        <p class="empty-title">No matching prompts</p>
        <p class="empty-description">Try adjusting your search or filter criteria.</p>
      {/if}
    </div>
  {:else}
    <div class="prompts-grid">
      {#each filteredPrompts as prompt (prompt.id)}
        <Card>
          {#snippet header()}
            <div class="card-header">
              <h4 class="prompt-name">{prompt.name}</h4>
              <Badge variant="primary">{PROMPT_CATEGORY_LABELS[prompt.category]}</Badge>
            </div>
          {/snippet}

          {#snippet body()}
            <p class="prompt-description">{prompt.description || 'No description'}</p>
            <div class="prompt-meta">
              <span class="meta-item">
                {prompt.variables_count} variable{prompt.variables_count !== 1 ? 's' : ''}
              </span>
              <span class="meta-separator">|</span>
              <span class="meta-item">Updated {formatDate(prompt.updated_at)}</span>
            </div>
          {/snippet}

          {#snippet footer()}
            <div class="card-actions">
              <Button
                variant="ghost"
                size="sm"
                onclick={() => handleEdit(prompt.id)}
              >
                Edit
              </Button>
              <Button
                variant="danger"
                size="sm"
                onclick={() => handleDelete(prompt.id)}
              >
                Delete
              </Button>
            </div>
          {/snippet}
        </Card>
      {/each}
    </div>
  {/if}
</div>

<style>
  .prompt-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .list-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--space-4);
    flex-wrap: wrap;
  }

  .filters {
    display: flex;
    gap: var(--space-3);
    flex: 1;
    min-width: 200px;
    max-width: 500px;
  }

  .filters :global(.search-input) {
    flex: 2;
  }

  .filters :global(.category-filter) {
    flex: 1;
    min-width: 150px;
  }

  .loading-state,
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    padding: var(--space-8);
    text-align: center;
    color: var(--text-secondary);
  }

  .empty-title {
    font-size: var(--font-lg);
    font-weight: var(--font-medium);
    color: var(--text-primary);
    margin: 0;
  }

  .empty-description {
    font-size: var(--font-sm);
    margin: 0;
  }

  .prompts-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: var(--space-4);
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: var(--space-2);
  }

  .prompt-name {
    font-size: var(--font-md);
    font-weight: var(--font-semibold);
    color: var(--text-primary);
    margin: 0;
    word-break: break-word;
  }

  .prompt-description {
    font-size: var(--font-sm);
    color: var(--text-secondary);
    margin: 0;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .prompt-meta {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--font-xs);
    color: var(--text-tertiary);
    margin-top: var(--space-2);
  }

  .meta-separator {
    color: var(--border-primary);
  }

  .card-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
</style>
