# Spec: TodoTasksBlock - Affichage des taches dans la conversation

**Date**: 2026-02-24
**Branch**: security/audit-remediation-tdd
**Scope**: Frontend + Backend minimal

## Objectif

Afficher les taches TodoTool dans la conversation comme une section dediee, visible en temps
reel pendant l'execution et persistee apres. Le widget montre toutes les taches avec leur nom,
statut, priorite, description et agent assigne. Taches groupees par agent/sous-agent.

## Architecture

### Approche: Section independante APRES le spinner

Le `TodoTasksBlock` est une **section separee** positionnee APRES le ExecutionSpinner
(pas un ChatBlockType dans le flux des blocs). Les taches sont groupees par agent/sous-agent.

```
+-----------------------------------------------------------+
| Messages Area                                              |
| [Messages + persisted blocks...]                           |
|                                                            |
| [Execution Blocks zone]                                    |
|   - ThinkingBlock                                          |
|   - ToolCallBlock (TodoTool)                               |
|   - SubAgentBlock                                          |
|                                                            |
|   [Execution Spinner] (if executing)                       |
|                                                            |
|   [TodoTasksBlock] <--- ICI, apres le spinner              |
|     Agent: "Planner"                                       |
|       [ ] Task 1 - pending (P3)                            |
|       [>] Task 2 - in_progress (P1)                        |
|     Sub-Agent: "Analyzer"                                  |
|       [v] Task 3 - completed (P2) - 1.5s                  |
|     [=========----] 1/3 completed                          |
|                                                            |
+-----------------------------------------------------------+
| Chat Input                                                 |
+-----------------------------------------------------------+
```

**Persiste**: Meme positionnement apres les blocs du dernier message assistant.

### Flux de donnees

```
[Real-time]
Rust TodoTool -> StreamChunk (task_*) -> executionBlocksStore handlers
  -> tasks: TodoTaskDisplay[] -> ChatContainer -> TodoTasksBlock

[Persisted]
SurrealDB task table -> list_workflow_tasks (Tauri command)
  -> +page.svelte -> ChatContainer -> TodoTasksBlock
```

## Changements par fichier

### Phase 2: Backend (Rust) - Minimal

#### 2.1 `src-tauri/src/models/streaming.rs`
- Ajouter champ `task_agent_name: Option<String>` a `StreamChunk`
- Mettre a jour `StreamChunk::task_create` pour accepter `agent_name: impl Into<String>`
- Mettre a jour `StreamChunk::task_update` et `task_complete` pour accepter `agent_name`
- Tests

#### 2.2 `src-tauri/src/tools/todo/tool.rs`
- Passer le nom de l'agent lors de l'emission des evenements task_*
- L'agent_id est deja disponible dans le contexte du tool

#### 2.3 `src-tauri/src/commands/task.rs` (optionnel)
- Ajouter `list_tasks_by_status` params: status filter  (deja existant - verification)

### Phase 3: Frontend (Svelte/TypeScript)

#### 3.1 `src/types/streaming.ts`
- Ajouter `task_agent_name?: string` a `StreamChunk`

#### 3.2 `src/types/chat-block.ts`
- Ajouter interface `TodoTaskDisplay`:
```typescript
export interface TodoTaskDisplay {
  id: string;
  name: string;
  description?: string;
  status: 'pending' | 'in_progress' | 'completed' | 'blocked';
  priority: number;
  agent_name?: string;
  duration_ms?: number;
}
```

#### 3.3 `src/lib/stores/executionBlocks.ts`
- Ajouter `tasks: TodoTaskDisplay[]` au `ExecutionBlocksState`
- Ajouter 3 handlers: `handleTaskCreate`, `handleTaskUpdate`, `handleTaskComplete`
- Les handlers mettent a jour le tableau `tasks` (pas les `blocks`)
- Registrer dans `chunkHandlers`
- Ajouter derived store: `executionTasks`
- Reset tasks dans `start()` et `reset()`

#### 3.4 `src/lib/components/chat/TodoTasksBlock.svelte` (NOUVEAU)
Composant avec:
- Props: `tasks: TodoTaskDisplay[]`
- Collapsible par defaut (ouvert)
- Header: icone ListTodo + "Tasks" + compteur "X/Y completed" + chevron
- Corps: taches groupees par `agent_name`:
  - Header groupe: nom de l'agent (ou "Main Agent" par defaut)
  - Liste de taches par agent:
    - Icone de statut coloree
    - Nom de la tache
    - Badge de priorite (1-5)
    - Description (si disponible, tronquee)
    - Duree (si completed)
- Barre de progression globale
- Animation fadeIn + respect prefers-reduced-motion
- Design: border-left info/accent, --color-bg-secondary

#### 3.5 `src/lib/components/agent/ChatContainer.svelte`
- Ajouter prop `tasks?: TodoTaskDisplay[]`
- Rendre TodoTasksBlock apres le spinner dans la zone execution-blocks
- Rendre TodoTasksBlock apres les persisted-blocks du dernier message assistant

#### 3.6 `src/routes/agent/+page.svelte`
- Importer `executionTasks` du store
- Charger les taches persistees via `list_workflow_tasks` dans `loadWorkflowData`
- Passer `tasks` a ChatContainer (real-time ou persistees)

#### 3.7 `src/lib/stores/utils/chunkProcessor.ts`
- Ajouter `agentName` a `ActiveTask` interface (deja dans streaming.ts)
- Mettre a jour `handleTaskCreate` pour capturer `task_agent_name`

## Tests

### Backend (Rust)
- `test_stream_chunk_task_create_with_agent_name`: agent_name inclus dans chunk
- `test_stream_chunk_task_create_agent_name_serialization`: JSON correct

### Frontend (TypeScript - Vitest)
- `test handleTaskCreate adds task to state`
- `test handleTaskCreate with agent name`
- `test handleTaskUpdate updates task status`
- `test handleTaskComplete marks task completed`
- `test multiple tasks from different agents`
- `test reset clears tasks`

## Positionnement (confirme)

| Contexte | Position | Source de donnees |
|----------|----------|-------------------|
| Real-time (executing) | Apres ExecutionSpinner | executionBlocksStore.tasks |
| Real-time (done) | Apres execution blocks | executionBlocksStore.tasks |
| Persiste (reload) | Apres persisted blocks du dernier msg | list_workflow_tasks depuis DB |

## Hors scope
- Interactivite (cliquer pour modifier)
- Drag-and-drop
- Filtrage/tri
- Graphe de dependances

## Ordre d'implementation
1. Types TS (streaming.ts, chat-block.ts) - TodoTaskDisplay
2. Tests executionBlocksStore handlers (RED)
3. Handlers executionBlocksStore (GREEN)
4. Backend streaming.rs: task_agent_name field + tests
5. Backend todo/tool.rs: passer agent_name
6. Composant TodoTasksBlock.svelte
7. Integration ChatContainer.svelte
8. Integration +page.svelte (real-time + persiste)
9. Validation complete (lint + check + test + clippy)
