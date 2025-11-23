# Frontend Specifications

> **Stack**: SvelteKit 2.49.0 | Svelte 5.43.14 | Tauri 2.9.4
> **Target**: Desktop/Laptop uniquement | Fullscreen mode
> **Architecture**: Multi-workflow simultané avec indicateurs temps réel

## Vue d'Ensemble

```
┌─────────────────────────────────────────────────────────────┐
│  Menu Flottant (Top)                                        │
│  [Configuration] [Agent]                                    │
└─────────────────────────────────────────────────────────────┘

Page Settings                    Page Agent
┌──────┬──────────┐            ┌──────┬───────────────────┐
│      │          │            │      │                   │
│ Side │ Content  │            │ Work │  Agent Interface  │
│ bar  │ Section  │            │ flow │  + Tools Display  │
│      │          │            │      │                   │
└──────┴──────────┘            └──────┴───────────────────┘
```

### Workflow Interaction Flow

```
User Input
    ↓
┌───────────────────────────────────────┐
│ Workflow Running?                     │
├───────────────┬───────────────────────┤
│ NO            │ YES                   │
│ ↓             │ ↓                     │
│ Process       │ Add to Queue          │
│ Immediately   │ [Queue: 1, 2, 3...]   │
└───────┬───────┴───────────────────────┘
        ↓
┌───────────────────────────────────────┐
│ Validation Required?                  │
├─────────────┬─────────────────────────┤
│ Auto Mode   │ Manual/Selective        │
│ ↓           │ ↓                       │
│ Execute     │ Pause → Request → Wait  │
│             │         User Decision   │
│             │         (Approve/Reject)│
└─────────────┴─────────────────────────┘
        ↓
┌───────────────────────────────────────┐
│ Execute Operation                     │
│ • Tool Call                           │
│ • Sub-Agent Spawn                     │
│ • MCP Server Call                     │
│ • File/DB Operation                   │
└───────────────────────────────────────┘
        ↓
┌───────────────────────────────────────┐
│ Stream Results → UI                   │
│ • Token count updates                 │
│ • Tool status updates                 │
│ • Reasoning steps (if supported)      │
└───────────────────────────────────────┘
        ↓
┌───────────────────────────────────────┐
│ Workflow Complete                     │
│ Process Queue (if any)                │
└───────────────────────────────────────┘
```

## 1. Menu Principal Flottant

### Position & Comportement
- **Position**: Top de page, flottant (fixed)
- **Z-index**: Élevé pour rester au-dessus du contenu
- **Transparence**: Légère (backdrop-filter blur) pour effet moderne
- **Responsive**: Adapte largeur selon contenu, max-width conteneur

### Navigation
```svelte
<nav class="floating-menu">
  <button on:click={() => goto('/settings')}>Configuration</button>
  <button on:click={() => goto('/agent')}>Agent</button>
</nav>
```

**Pattern Recommandé**: [Navigation Best Practices](https://www.nngroup.com/articles/vertical-nav/)
- Maximum 2 niveaux de sous-menus pour éviter surcharge cognitive
- Icons + labels pour améliorer usabilité
- Support navigation clavier pour accessibilité

## 2. Page Settings

### Architecture Sidebar
```
┌────────────────┬─────────────────────────────┐
│                │                             │
│  Providers     │  Content: Provider Config   │
│  Models        │  - API Keys                 │
│  Theme         │  - Endpoints                │
│  Agents        │  - Rate limits              │
│  Prompts       │                             │
│  MCP           │                             │
│  Memory        │                             │
│  Directories   │                             │
│                │                             │
│ [◀] Collapse   │                             │
└────────────────┴─────────────────────────────┘
```

### Sidebar Rétractable

**State Management** (Svelte 5 runes)
```svelte
<script lang="ts">
  let collapsed = $state(false);
  let activeSection = $state('providers');
</script>

<aside class:collapsed>
  <nav>
    {#each sections as section}
      <button
        on:click={() => activeSection = section.id}
        class:active={activeSection === section.id}
      >
        {section.label}
      </button>
    {/each}
  </nav>
  <button on:click={() => collapsed = !collapsed}>
    {collapsed ? '▶' : '◀'}
  </button>
</aside>
```

**Animation**: Transition smooth (200-300ms) selon [UX Best Practices](https://uiuxdesigntrends.com/best-ux-practices-for-sidebar-menu-in-2025/)

### Sections Détaillées

#### Providers
- Liste providers disponibles (OpenAI, Anthropic, Gemini, Ollama)
- Configuration par provider :
  - API Key (input type="password")
  - Endpoint URL
  - Rate limits (requests/min)
  - Timeout (seconds)
- Toggle enable/disable
- Test connection (button + status indicator)

#### Models
- Sélection model par provider
- Affichage capacités :
  - Context window (tokens)
  - Output max tokens
  - Pricing (input/output par 1M tokens)
  - Features (vision, function calling, streaming)
- Configuration par défaut :
  - Temperature (slider 0-2)
  - Top P (slider 0-1)
  - Frequency penalty (slider -2 à 2)
  - Presence penalty (slider -2 à 2)

#### Theme
- Sélection thème : Light | Dark | Auto (system)
- Color scheme customization :
  - Primary color (color picker)
  - Accent color
  - Background variants
- Font settings :
  - Font family (select)
  - Font size (slider 12-20px)
  - Line height (slider 1.2-2)
- Preview en temps réel

#### Agents
- Liste agents permanents + temporaires
- CRUD complet :
  - Create: Modal avec formulaire
  - Read: Affichage configuration
  - Update: Édition inline ou modal
  - Delete: Confirmation requise
- Tri & filtrage :
  - Par nom (alphabétique)
  - Par type (permanent/temporaire)
  - Par dernière utilisation
  - Search bar (filter par nom/description)
- Import/Export configuration (JSON/TOML)

#### Modèle de Prompt
- Bibliothèque prompts enregistrés
- Structure :
  - Nom (unique)
  - Description
  - Catégorie (tag)
  - Contenu (textarea avec syntax highlighting)
  - Variables (placeholders détectés automatiquement)
- Actions :
  - Duplicate
  - Export (markdown)
  - Versioning (historique modifications)
- Preview avec variables remplies

#### MCP
- Liste MCP servers disponibles
- Configuration par server :
  - Enable/Disable toggle
  - Connection settings (stdio, docker, HTTP, SSE)
  - Capabilities list (read-only)
  - Tools disponibles (expandable list)
- Status monitoring :
  - Connection status (●online/●offline)
  - Latency moyenne (ms)
  - Erreurs récentes (collapsible)
- Logs (dernières 50 entrées, filtrable)

#### Memory Tool Settings
- **Modèle Embedding**
  - Sélection provider (selon providers activés)
  - Sélection modèle embedding si disponible pour provider :
    - OpenAI : text-embedding-3-small (1536D), text-embedding-3-large (3072D)
    - Ollama : nomic-embed-text (768D), mxbai-embed-large (1024D)
    - Mistral : mistral-embed (1024D), codestral-embed (256-1024D variable)
  - Dimensions embedding (slider ou select selon modèle)
    - 768D (BERT/Ollama léger)
    - 1024D (Mistral/Ollama équilibré)
    - 1536D (OpenAI standard)
    - 3072D (OpenAI haute précision)
  - Chunking settings :
    - Chunk size (slider 100-2000 caractères, défaut: 512)
    - Overlap (slider 0-500 caractères, défaut: 50)
    - Stratégie : Fixed | Semantic | Recursive
  - Test embedding (input + bouton "Test" → affiche vecteur preview)

- **Liste Mémoires**
  - Table avec colonnes :
    - Type (user_pref | context | knowledge | decision)
    - Contenu (preview 100 chars, expandable)
    - Source (agent créateur)
    - Date création
    - Tags
    - Actions (View | Edit | Delete)
  - Filtres :
    - Par type
    - Par agent source
    - Par date range
    - Search sémantique (input → recherche vectorielle)
  - Tri :
    - Date (récent/ancien)
    - Type
    - Pertinence (si recherche active)
  - Pagination (50 entrées par page)

- **Ajout Mémoire Manuel**
  - Modal formulaire :
    - Type (select : user_pref, context, knowledge, decision)
    - Contenu (textarea, max 5000 chars)
    - Tags (multi-input, suggestions auto)
    - Priority (slider 0.0-1.0)
    - Workflow association (select, optionnel)
  - Preview embedding (affiche vecteur généré avant sauvegarde)
  - Button "Save" → génère embedding + enregistre dans SurrealDB

- **Actions Globales**
  - Export toutes mémoires (JSON/CSV)
  - Import mémoires (JSON avec validation schéma)
  - Purge par critères :
    - Date (supprimer >X jours)
    - Type
    - Agent source
  - Statistiques :
    - Total mémoires
    - Distribution par type (pie chart)
    - Utilisation espace vectoriel

#### Directory Management
- **Répertoire Racine**
  - Affichage path : `appDataDir()/reports/` (Tauri)
  - Button "Open in Explorer" → ouvre explorateur système

- **Arbre de Fichiers**
  - Vue hiérarchique (tree view)
  - Icônes par type :
    - 📁 Dossier
    - 📄 Markdown (.md)
    - 📊 JSON (.json)
    - 📋 Texte (.txt)
    - ❓ Autres
  - Affichage infos :
    - Nom fichier/dossier
    - Taille (KB/MB)
    - Date modification
    - Actions (hover)

- **Actions Fichiers**
  - View : Ouvre preview dans modal (markdown rendered, JSON formaté)
  - Download : Télécharge fichier
  - Rename : Input inline édition
  - Delete : Confirmation modal (⚠️ "Are you sure?")
  - Move : Drag & drop ou select destination

- **Actions Répertoires**
  - Create New : Modal avec input nom + path parent
  - Rename : Input inline édition
  - Delete : Confirmation recursive (affiche nombre fichiers impactés)
  - Move : Drag & drop ou select destination

- **Filtres & Recherche**
  - Search bar (recherche nom fichier/dossier)
  - Filtres :
    - Type fichier (checkbox multi-select)
    - Date range (date picker)
    - Taille (slider min-max)
  - Tri :
    - Nom (A-Z, Z-A)
    - Date (récent/ancien)
    - Taille (petit/grand)
    - Type

- **Upload Fichiers**
  - Drag & drop zone
  - Button "Upload Files"
  - Multi-upload supporté
  - Progress bar par fichier
  - Validation :
    - Max size : 10MB par fichier
    - Types autorisés : .md, .txt, .json, .csv
    - Scan anti-malware (optionnel)

- **Scope & Sécurité**
  - Scope Tauri configuré : `["$APPDATA/reports/*"]`
  - Path traversal bloqué (validation backend)
  - Confirmation pour suppression définitive
  - Logs d'opérations (audit trail)

#### Validation (Global Settings)
- Mode par défaut : Auto | Manual | Selective
- Configuration selective globale :
  - Tools validation (ON/OFF)
  - Sub-agents validation (ON/OFF)
  - MCP calls validation (ON/OFF)
  - File operations validation (ON/OFF)
  - Database operations validation (ON/OFF)
- Risk level thresholds :
  - Auto-approve LOW risk (checkbox)
  - Always confirm HIGH risk (checkbox, disabled par défaut)
- Timeout validation request :
  - Délai avant auto-reject (slider 30s - 5min)
  - Comportement timeout : Reject | Approve | Ask Again
- Audit settings :
  - Enable validation logging (checkbox)
  - Log retention (days, slider 7-90)
  - Export logs (button → CSV/JSON)

## 3. Page Agent

### Layout Multi-Workflow

```
┌──────────┬─────────────────────────────────────────┐
│          │  ┌─────────────────────────────────┐    │
│ Workflow │  │ Input Area                      │    │
│   List   │  │ [📎 Prompt] [Send]              │    │
│          │  └─────────────────────────────────┘    │
│ • Task 1 │                                          │
│ ◆ Task 2 │  ┌─────────────────────────────────┐    │
│ • Task 3 │  │ Output Stream                   │    │
│          │  │ [Agent response here...]        │    │
│ + New    │  └─────────────────────────────────┘    │
│          │                                          │
│          │  ┌─────────────────────────────────┐    │
│ [◀]      │  │ Metrics & Tools                 │    │
│          │  │ Tokens: 1.2K/4K | Tools: 3      │    │
│          │  └─────────────────────────────────┘    │
└──────────┴─────────────────────────────────────────┘
```

### Sidebar Workflows (Gauche)

**Structure**
```svelte
<script lang="ts">
  type Workflow = {
    id: string;
    name: string;
    status: 'idle' | 'running' | 'completed' | 'error';
    agent_id: string;
    created_at: Date;
  };

  let workflows = $state<Workflow[]>([]);
  let activeWorkflow = $state<string | null>(null);
</script>

<aside class="workflows">
  <div class="toolbar">
    <input type="search" placeholder="Filter workflows..." />
    <button on:click={createWorkflow}>+ New</button>
  </div>

  <ul>
    {#each sortedWorkflows as workflow}
      <li
        class:active={activeWorkflow === workflow.id}
        on:click={() => selectWorkflow(workflow.id)}
      >
        <StatusIcon status={workflow.status} />
        <span class="name" contenteditable>{workflow.name}</span>
        <button on:click={() => deleteWorkflow(workflow.id)}>×</button>
      </li>
    {/each}
  </ul>

  <button class="collapse">◀</button>
</aside>
```

**Fonctionnalités**
- Tri dynamique :
  - Par statut (running → idle → completed)
  - Par date (récent → ancien)
  - Par nom (A-Z)
- Édition nom : Click inline edit (contenteditable)
- Status visuel :
  - ● Running (animation pulse)
  - ○ Idle
  - ✓ Completed (fade green)
  - ✗ Error (fade red)
- Navigation : Click switch workflow instantané
- CRUD :
  - Create: Modal sélection agent + prompt
  - Delete: Confirmation si running
  - Duplicate: Copy workflow + rename

### Zone Input

**Composant Principal**
```svelte
<div class="input-area">
  <textarea
    bind:value={userInput}
    placeholder="Enter your message..."
    on:keydown={handleKeydown}
  />

  <div class="actions">
    <button on:click={openPromptSelector}>
      📎 Prompt
    </button>
    <button on:click={sendMessage} disabled={!userInput.trim()}>
      Send
    </button>
  </div>
</div>
```

**Prompt Selector**
- Modal overlay avec liste prompts enregistrés
- Preview prompt au hover
- Variables auto-détectées → formulaire dynamique
- Insertion variables dans textarea

### Message Queue System (User-in-the-Loop)

**Contexte**: L'utilisateur peut envoyer des messages pendant l'exécution d'un workflow.

**Architecture Queue**
```svelte
<script lang="ts">
  type QueuedMessage = {
    id: string;
    content: string;
    timestamp: Date;
    status: 'pending' | 'processing' | 'processed';
  };

  let messageQueue = $state<QueuedMessage[]>([]);
  let isWorkflowRunning = $state(false);

  async function sendMessage() {
    const message: QueuedMessage = {
      id: crypto.randomUUID(),
      content: userInput,
      timestamp: new Date(),
      status: isWorkflowRunning ? 'pending' : 'processing'
    };

    if (isWorkflowRunning) {
      messageQueue.push(message);
      showQueueNotification(messageQueue.length);
    } else {
      await processMessage(message);
    }

    userInput = '';
  }

  // Process queue after workflow completes
  async function onWorkflowComplete() {
    isWorkflowRunning = false;

    while (messageQueue.length > 0) {
      const message = messageQueue.shift()!;
      message.status = 'processing';
      await processMessage(message);
      message.status = 'processed';
    }
  }
</script>
```

**UI Queue Indicator**
```svelte
{#if messageQueue.length > 0}
  <div class="message-queue-indicator">
    <span class="badge">{messageQueue.length}</span>
    <span class="text">messages in queue</span>
    <button on:click={viewQueue}>View</button>
  </div>
{/if}

<!-- Queue Modal -->
<dialog open={showQueueModal}>
  <h3>Message Queue ({messageQueue.length})</h3>
  <ul>
    {#each messageQueue as msg, i}
      <li>
        <span class="position">#{i + 1}</span>
        <div class="content">{msg.content}</div>
        <StatusBadge status={msg.status} />
        <button on:click={() => removeFromQueue(msg.id)}>×</button>
      </li>
    {/each}
  </ul>
  <div class="actions">
    <button on:click={clearQueue}>Clear All</button>
    <button on:click={() => showQueueModal = false}>Close</button>
  </div>
</dialog>
```

**Comportement Input**
- Input toujours actif (même pendant workflow running)
- Visual feedback si message mis en queue :
  - Badge compteur visible
  - Toast notification : "Message added to queue (position #3)"
  - Input border couleur différente (queue mode)
- Réorganisation queue : Drag & drop pour changer ordre
- Édition queue : Click pour modifier message avant traitement

### Validation System (Human-in-the-Loop)

**Modes de Validation**

```ts
type ValidationMode = 'auto' | 'manual' | 'selective';

type ValidationConfig = {
  mode: ValidationMode;
  selective?: {
    tools: boolean;      // Valider tools usage
    subAgents: boolean;  // Valider spawn sub-agents
    mcp: boolean;        // Valider MCP calls
    fileOps: boolean;    // Valider opérations fichiers
    dbOps: boolean;      // Valider opérations DB
  };
};
```

**Configuration UI**
```svelte
<section class="validation-settings">
  <h3>Validation Mode</h3>

  <label>
    <input type="radio" bind:group={validationMode} value="auto" />
    <div>
      <strong>Auto-validate All</strong>
      <p>Execute all operations without confirmation</p>
    </div>
  </label>

  <label>
    <input type="radio" bind:group={validationMode} value="manual" />
    <div>
      <strong>Manual Validation</strong>
      <p>Request confirmation for every operation</p>
    </div>
  </label>

  <label>
    <input type="radio" bind:group={validationMode} value="selective" />
    <div>
      <strong>Selective Validation</strong>
      <p>Choose which operations require confirmation</p>
    </div>
  </label>

  {#if validationMode === 'selective'}
    <div class="selective-options">
      <label>
        <input type="checkbox" bind:checked={selectiveConfig.tools} />
        Validate Tool Usage
      </label>
      <label>
        <input type="checkbox" bind:checked={selectiveConfig.subAgents} />
        Validate Sub-Agent Spawn
      </label>
      <label>
        <input type="checkbox" bind:checked={selectiveConfig.mcp} />
        Validate MCP Calls
      </label>
      <label>
        <input type="checkbox" bind:checked={selectiveConfig.fileOps} />
        Validate File Operations
      </label>
      <label>
        <input type="checkbox" bind:checked={selectiveConfig.dbOps} />
        Validate Database Operations
      </label>
    </div>
  {/if}
</section>
```

**Validation Request UI**
```svelte
<script lang="ts">
  type ValidationRequest = {
    id: string;
    type: 'tool' | 'sub_agent' | 'mcp' | 'file_op' | 'db_op';
    operation: string;
    details: Record<string, any>;
    risk_level: 'low' | 'medium' | 'high';
  };

  let pendingValidations = $state<ValidationRequest[]>([]);
</script>

<!-- Validation Modal -->
<dialog open={pendingValidations.length > 0}>
  <div class="validation-request">
    {#each pendingValidations as request}
      <div class="request-card" class:high-risk={request.risk_level === 'high'}>
        <div class="header">
          <h3>Validation Required</h3>
          <span class="risk-badge" class:high={request.risk_level === 'high'}>
            {request.risk_level} risk
          </span>
        </div>

        <div class="operation">
          <strong>{request.type.toUpperCase()}</strong>
          <code>{request.operation}</code>
        </div>

        <div class="details">
          <h4>Details</h4>
          <pre>{JSON.stringify(request.details, null, 2)}</pre>
        </div>

        <div class="actions">
          <button
            class="approve"
            on:click={() => approveValidation(request.id)}
          >
            ✓ Approve
          </button>
          <button
            class="reject"
            on:click={() => rejectValidation(request.id)}
          >
            ✗ Reject
          </button>
          <button
            class="approve-all"
            on:click={approveAllPending}
          >
            Approve All Pending
          </button>
        </div>
      </div>
    {/each}
  </div>
</dialog>

<!-- Validation Indicator in Workflow -->
<div class="workflow-status">
  {#if pendingValidations.length > 0}
    <div class="waiting-validation">
      <span class="icon">⏸️</span>
      <span class="text">Waiting for validation</span>
      <span class="badge">{pendingValidations.length}</span>
    </div>
  {/if}
</div>
```

**Backend Integration**
```rust
// src-tauri/src/commands/validation.rs
#[tauri::command]
async fn request_validation(
    app_handle: AppHandle,
    validation_config: ValidationConfig,
    operation: Operation,
) -> Result<ValidationResponse, String> {
    match validation_config.mode {
        ValidationMode::Auto => {
            // Auto-approve
            Ok(ValidationResponse::Approved)
        }
        ValidationMode::Manual => {
            // Pause workflow, request user input
            let request = ValidationRequest {
                id: Uuid::new_v4().to_string(),
                type_: operation.operation_type(),
                operation: operation.name(),
                details: operation.details(),
                risk_level: assess_risk(&operation),
            };

            // Emit to frontend
            app_handle.emit_all("validation_request", &request)?;

            // Wait for user response (async channel)
            wait_for_user_response(request.id).await
        }
        ValidationMode::Selective => {
            // Check if this operation type needs validation
            if should_validate(&validation_config, &operation) {
                // Same as Manual
                // ...
            } else {
                Ok(ValidationResponse::Approved)
            }
        }
    }
}

fn assess_risk(operation: &Operation) -> RiskLevel {
    match operation.operation_type() {
        OperationType::FileDelete => RiskLevel::High,
        OperationType::DbDelete => RiskLevel::High,
        OperationType::ToolExecution => RiskLevel::Low,
        OperationType::McpCall => RiskLevel::Medium,
        OperationType::SubAgentSpawn => RiskLevel::Medium,
    }
}
```

**Flow Validation**
```
1. Agent détecte opération nécessitant validation
   ↓
2. Backend vérifie ValidationConfig
   ↓
3a. Mode Auto → Execute immédiatement
3b. Mode Manual/Selective → Pause workflow
   ↓
4. Emit validation_request → Frontend
   ↓
5. UI affiche modal validation
   ↓
6. User: Approve | Reject | Approve All
   ↓
7. Frontend send response → Backend
   ↓
8a. Approved → Resume workflow, execute operation
8b. Rejected → Skip operation, continue workflow
```

**Persistence Préférences**
```ts
// Sauvegarder config validation par agent
async function saveValidationConfig(agentId: string, config: ValidationConfig) {
  await invoke('save_agent_validation_config', {
    agentId,
    config
  });
}

// Charger config au démarrage workflow
async function loadValidationConfig(agentId: string): Promise<ValidationConfig> {
  return await invoke('load_agent_validation_config', { agentId });
}
```

**Shortcuts Validation**
- `Ctrl+Enter`: Approve current validation
- `Ctrl+Shift+Enter`: Approve all pending
- `Esc`: Reject current validation
- `Ctrl+D`: Toggle validation mode (auto ↔ manual)

**Audit Trail**
```markdown
# Validation Log: workflow_123
[2025-11-23 10:32] Tool: SurrealDBTool → DELETE query
  Risk: HIGH | User: APPROVED | Duration: 2.3s

[2025-11-23 10:33] MCP: serena::replace_content
  Risk: MEDIUM | User: APPROVED | Duration: 0.8s

[2025-11-23 10:34] Sub-Agent: migration_agent
  Risk: MEDIUM | Mode: AUTO | Duration: 15.2s
```

### Calcul Tokens Temps Réel

**Pattern Recommandé**: [Open WebUI Token Counter](https://github.com/open-webui/open-webui/discussions/5455)

**Display Format**
```
[current_tokens] / [max_tokens]  •  [tokens/s]
   1,234        /    4,096       •    45 tk/s
```

**Implementation**
```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  let tokenStats = $state({
    input: 0,
    output: 0,
    total: 0,
    max: 4096,
    speed: 0 // tokens/s
  });

  // Real-time update via streaming
  async function trackTokens(text: string) {
    tokenStats.input = await invoke('count_tokens', { text });
    tokenStats.total = tokenStats.input + tokenStats.output;
  }

  $effect(() => {
    trackTokens(userInput);
  });
</script>

<div class="token-display">
  <span class:warning={tokenStats.total > tokenStats.max * 0.8}>
    {tokenStats.total.toLocaleString()} / {tokenStats.max.toLocaleString()}
  </span>
  {#if isStreaming}
    <span class="speed">• {tokenStats.speed} tk/s</span>
  {/if}
  <progress value={tokenStats.total} max={tokenStats.max} />
</div>
```

**Warning States**
- 0-75%: Normal (green)
- 75-90%: Warning (orange)
- 90-100%: Critical (red)
- 100%+: Error (message truncation)

### Affichage Tools & MCP

**Panel Tools Actifs**
```svelte
<div class="tools-panel">
  <h3>Active Tools ({activatedTools.length})</h3>
  <ul>
    {#each activatedTools as tool}
      <li class:executing={tool.status === 'executing'}>
        <span class="name">{tool.name}</span>
        <span class="duration">{tool.duration}ms</span>
        <StatusBadge status={tool.status} />
      </li>
    {/each}
  </ul>

  <h3>MCP Servers ({mcpServers.length})</h3>
  <ul>
    {#each mcpServers as server}
      <li>
        <span class="name">{server.name}</span>
        <span class="calls">{server.callCount} calls</span>
        <span class="latency">{server.avgLatency}ms avg</span>
      </li>
    {/each}
  </ul>
</div>
```

**Real-time Updates**
- SSE (Server-Sent Events) depuis Rust backend
- Update status tools en temps réel
- Animation pulse pendant exécution
- Historique tools utilisés (collapsible)

### Sous-Agents en Cours

**Visualization**
```svelte
<div class="sub-agents">
  <h3>Sub-Agents ({runningAgents.length})</h3>
  {#each runningAgents as agent}
    <div class="agent-card">
      <div class="header">
        <span class="name">{agent.name}</span>
        <StatusBadge status={agent.status} />
      </div>
      <div class="task">
        {agent.currentTask}
      </div>
      <div class="progress">
        <progress value={agent.progress} max="100" />
        <span>{agent.progress}%</span>
      </div>
      {#if agent.tools.length}
        <details>
          <summary>Tools ({agent.tools.length})</summary>
          <ul>
            {#each agent.tools as tool}
              <li>{tool}</li>
            {/each}
          </ul>
        </details>
      {/if}
    </div>
  {/each}
</div>
```

**Pattern**: [Multi-Workflow Task Manager](https://www.guru99.com/workflow-management-software-tool.html)
- Kanban-style cards pour agents
- Progress bars pour tâches longues
- Expandable details (tools, MCP calls)
- token use

### Reasoning Display

**Condition**: Si modèle supporte reasoning (future capability)

```svelte
{#if model.supportsReasoning}
  <div class="reasoning-panel">
    <h3>
      Reasoning
      <button on:click={() => showReasoning = !showReasoning}>
        {showReasoning ? '▼' : '▶'}
      </button>
    </h3>

    {#if showReasoning}
      <div class="reasoning-content">
        {#each reasoningSteps as step, i}
          <div class="step">
            <span class="index">{i + 1}</span>
            <div class="content">{step.content}</div>
            <div class="meta">
              <span class="time">{step.duration}ms</span>
              <span class="tokens">{step.tokens} tokens</span>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/if}
```

**Streaming Reasoning**
- Update temps réel pendant génération
- Auto-scroll vers dernière étape
- Syntax highlighting pour code/JSON
- Collapse/expand par défaut (user preference)

### Indicateurs Visuels Tâches

**Status Indicators**
```css
.status-indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.status-idle { background: #666; }
.status-running {
  background: #3b82f6;
  animation: pulse 2s infinite;
}
.status-completed { background: #10b981; }
.status-error { background: #ef4444; }

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}
```

**Progress Components**
- Spinner pour tâches indéterminées
- Progress bar pour tâches déterminées (avec %)
- Estimated time remaining (si calculable)
- Toast notifications pour events majeurs :
  - Workflow started
  - Workflow completed
  - Error occurred
  - User confirmation required

### Settings Agent Spécifiques

**Modal Configuration Agent**
```svelte
<dialog open={showAgentSettings}>
  <h2>Agent Settings: {selectedAgent?.name}</h2>

  <section>
    <h3>Model Selection</h3>
    <select bind:value={agentConfig.model}>
      {#each availableModels as model}
        <option value={model.id}>{model.name}</option>
      {/each}
    </select>
  </section>

  <section>
    <h3>Parameters</h3>
    <label>
      Temperature
      <input type="range" min="0" max="2" step="0.1"
             bind:value={agentConfig.temperature} />
      <output>{agentConfig.temperature}</output>
    </label>

    <label>
      Max Tokens
      <input type="number" bind:value={agentConfig.maxTokens} />
    </label>
  </section>

  <section>
    <h3>System Prompt</h3>
    <textarea bind:value={agentConfig.systemPrompt} />
  </section>

  <section>
    <h3>Tools</h3>
    {#each availableTools as tool}
      <label>
        <input type="checkbox" bind:checked={tool.enabled} />
        {tool.name}
      </label>
    {/each}
  </section>

  <section>
    <h3>MCP Servers</h3>
    {#each mcpServers as server}
      <label>
        <input type="checkbox" bind:checked={server.enabled} />
        {server.name}
      </label>
    {/each}
  </section>

  <div class="actions">
    <button on:click={saveAgentConfig}>Save</button>
    <button on:click={() => showAgentSettings = false}>Cancel</button>
  </div>
</dialog>
```

### Création Agent Custom

**Wizard Multi-Step**
```svelte
<script lang="ts">
  let step = $state(1);
  let newAgent = $state({
    name: '',
    description: '',
    lifecycle: 'permanent',
    provider: 'Claude',
    model: 'claude-sonnet-4.5',
    systemPrompt: '',
    tools: [],
    mcpServers: []
  });
</script>

<div class="agent-wizard">
  <div class="steps">
    <span class:active={step === 1}>1. Basic Info</span>
    <span class:active={step === 2}>2. Model</span>
    <span class:active={step === 3}>3. Capabilities</span>
    <span class:active={step === 4}>4. Review</span>
  </div>

  {#if step === 1}
    <StepBasicInfo bind:agent={newAgent} />
  {:else if step === 2}
    <StepModelSelection bind:agent={newAgent} />
  {:else if step === 3}
    <StepCapabilities bind:agent={newAgent} />
  {:else if step === 4}
    <StepReview agent={newAgent} />
  {/if}

  <div class="navigation">
    {#if step > 1}
      <button on:click={() => step--}>Previous</button>
    {/if}
    {#if step < 4}
      <button on:click={() => step++}>Next</button>
    {:else}
      <button on:click={createAgent}>Create Agent</button>
    {/if}
  </div>
</div>
```

## 4. Multi-Workflow Simultané

### State Management

**Store Global Workflows**
```ts
// stores/workflows.ts
import { writable } from 'svelte/store';

export type WorkflowState = {
  id: string;
  name: string;
  agent_id: string;
  status: 'idle' | 'running' | 'completed' | 'error';
  messages: Message[];
  tools: ToolExecution[];
  subAgents: SubAgent[];
  metrics: WorkflowMetrics;
};

export const workflows = writable<Map<string, WorkflowState>>(new Map());

export function createWorkflow(agentId: string, name: string) {
  const id = crypto.randomUUID();
  workflows.update(map => {
    map.set(id, {
      id,
      name,
      agent_id: agentId,
      status: 'idle',
      messages: [],
      tools: [],
      subAgents: [],
      metrics: { tokens: 0, duration: 0, cost: 0 }
    });
    return map;
  });
  return id;
}
```

### Navigation Inter-Workflows

**Tabs ou List** (Pattern recommandé: Tabs pour ≤5, List pour >5)
```svelte
<nav class="workflow-tabs">
  {#each Array.from($workflows.values()) as workflow}
    <button
      class:active={$activeWorkflowId === workflow.id}
      on:click={() => switchWorkflow(workflow.id)}
    >
      <StatusIcon status={workflow.status} />
      {workflow.name}
      <button on:click|stopPropagation={() => closeWorkflow(workflow.id)}>
        ×
      </button>
    </button>
  {/each}
  <button on:click={createNewWorkflow}>+</button>
</nav>
```

**Keyboard Shortcuts**
- `Ctrl+Tab`: Next workflow
- `Ctrl+Shift+Tab`: Previous workflow
- `Ctrl+T`: New workflow
- `Ctrl+W`: Close current workflow
- `Ctrl+1-9`: Jump to workflow N

### Persistence

**Auto-save** (SurrealDB via Tauri)
```rust
// src-tauri/src/commands/workflow.rs
#[tauri::command]
async fn save_workflow_state(id: String, state: WorkflowState) -> Result<(), String> {
    let db = get_db_connection().await?;

    db.query("
        UPDATE workflow SET
            name = $name,
            status = $status,
            messages = $messages,
            updated_at = time::now()
        WHERE id = $id
    ")
    .bind(("id", id))
    .bind(("name", state.name))
    .bind(("status", state.status))
    .bind(("messages", state.messages))
    .await?;

    Ok(())
}
```

**Load on Startup**
- Récupérer workflows non-terminés
- Restaurer état exact (messages, metrics)
- Demander si reprendre workflows running (crash recovery)

## 5. Architecture Composants Réutilisables

### Component Library

```
src/lib/components/
├─ ui/
│  ├─ Button.svelte
│  ├─ Input.svelte
│  ├─ Select.svelte
│  ├─ Textarea.svelte
│  ├─ Modal.svelte
│  ├─ Toast.svelte
│  └─ Progress.svelte
├─ layout/
│  ├─ Sidebar.svelte
│  ├─ FloatingMenu.svelte
│  └─ Panel.svelte
├─ workflow/
│  ├─ WorkflowList.svelte
│  ├─ WorkflowCard.svelte
│  ├─ MessageStream.svelte
│  └─ InputArea.svelte
├─ agent/
│  ├─ AgentSelector.svelte
│  ├─ AgentSettings.svelte
│  ├─ AgentWizard.svelte
│  └─ SubAgentCard.svelte
├─ monitoring/
│  ├─ TokenDisplay.svelte
│  ├─ ToolsPanel.svelte
│  ├─ MCPStatus.svelte
│  └─ ReasoningPanel.svelte
└─ settings/
   ├─ ProviderConfig.svelte
   ├─ ModelConfig.svelte
   ├─ ThemeSelector.svelte
   └─ PromptLibrary.svelte
```

### Props Pattern (TypeScript)

```svelte
<script lang="ts">
  interface Props {
    workflow: WorkflowState;
    onStatusChange?: (status: WorkflowStatus) => void;
    readonly?: boolean;
  }

  let { workflow, onStatusChange, readonly = false }: Props = $props();
</script>
```

## 6. Communication Frontend ↔ Backend

### Tauri Commands

**Invoke Pattern**
```ts
// Frontend
import { invoke } from '@tauri-apps/api/core';

const result = await invoke<WorkflowResult>('execute_workflow', {
  workflowId: '123',
  message: 'User input',
  agentId: 'db_agent'
});
```

```rust
// Backend
#[tauri::command]
async fn execute_workflow(
    workflow_id: String,
    message: String,
    agent_id: String
) -> Result<WorkflowResult, String> {
    // Execute agent workflow
    let agent = AgentRegistry::get(&agent_id)?;
    let report = agent.execute(Task::new(message)).await?;

    Ok(WorkflowResult {
        report,
        metrics: /* ... */
    })
}
```

### Streaming Responses (SSE)

**Event Listener**
```ts
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen<StreamChunk>('workflow_stream', (event) => {
  const chunk = event.payload;

  switch (chunk.type) {
    case 'token':
      appendToken(chunk.content);
      updateTokenCount();
      break;
    case 'tool_start':
      markToolExecuting(chunk.tool);
      break;
    case 'tool_end':
      markToolCompleted(chunk.tool, chunk.duration);
      break;
    case 'reasoning':
      addReasoningStep(chunk.content);
      break;
  }
});
```

**Backend Emitter**
```rust
use tauri::Manager;

async fn stream_workflow(app_handle: &AppHandle, workflow_id: String) {
    // Stream tokens
    app_handle.emit_all("workflow_stream", StreamChunk {
        workflow_id: workflow_id.clone(),
        type: "token",
        content: "Hello",
    }).unwrap();

    // Stream tool execution
    app_handle.emit_all("workflow_stream", StreamChunk {
        workflow_id,
        type: "tool_start",
        tool: "SurrealDBTool",
    }).unwrap();
}
```

## 7. Accessibilité (WCAG AA)

### Patterns Requis

**Keyboard Navigation**
- `Tab`: Focus suivant
- `Shift+Tab`: Focus précédent
- `Enter/Space`: Activer button/link
- `Esc`: Fermer modal/dropdown
- `Arrow keys`: Navigation lists/menus

**ARIA Labels**
```svelte
<button
  aria-label="Create new workflow"
  aria-pressed={isActive}
>
  +
</button>

<div role="status" aria-live="polite">
  {statusMessage}
</div>

<progress
  value={current}
  max={total}
  aria-label="Token usage: {current} of {total}"
/>
```

**Focus Management**
```svelte
<script lang="ts">
  let modalOpen = $state(false);
  let firstFocusable: HTMLElement;

  $effect(() => {
    if (modalOpen) {
      firstFocusable?.focus();
    }
  });
</script>

<dialog open={modalOpen}>
  <button bind:this={firstFocusable}>First</button>
  <!-- content -->
</dialog>
```

**Color Contrast**: Ratio minimum 4.5:1 (texte normal), 3:1 (large text)

## 8. Performance

### Optimization Strategies

**Virtual Scrolling** (listes >100 items)
```svelte
<script lang="ts">
  import VirtualList from '@sveltejs/svelte-virtual-list';
</script>

<VirtualList items={messages} let:item>
  <MessageCard message={item} />
</VirtualList>
```

**Lazy Loading Components**
```ts
const AgentSettings = lazy(() => import('$lib/components/agent/AgentSettings.svelte'));
```

**Debounce Input**
```svelte
<script lang="ts">
  import { debounce } from '$lib/utils';

  const debouncedTokenCount = debounce(async (text: string) => {
    tokenCount = await invoke('count_tokens', { text });
  }, 300);

  $effect(() => {
    debouncedTokenCount(userInput);
  });
</script>
```

**Memoization** (Svelte 5 $derived)
```svelte
<script lang="ts">
  let workflows = $state<Workflow[]>([]);

  let sortedWorkflows = $derived(
    workflows.sort((a, b) =>
      statusPriority[a.status] - statusPriority[b.status]
    )
  );
</script>
```

## 9. Styling Architecture

### CSS Variables (Theme System)

```css
:root {
  /* Colors */
  --color-bg-primary: #ffffff;
  --color-bg-secondary: #f3f4f6;
  --color-text-primary: #111827;
  --color-text-secondary: #6b7280;
  --color-accent: #3b82f6;
  --color-success: #10b981;
  --color-warning: #f59e0b;
  --color-error: #ef4444;

  /* Spacing */
  --spacing-xs: 0.25rem;
  --spacing-sm: 0.5rem;
  --spacing-md: 1rem;
  --spacing-lg: 1.5rem;
  --spacing-xl: 2rem;

  /* Typography */
  --font-family: 'Inter', system-ui, sans-serif;
  --font-size-sm: 0.875rem;
  --font-size-base: 1rem;
  --font-size-lg: 1.125rem;
  --font-size-xl: 1.25rem;

  /* Shadows */
  --shadow-sm: 0 1px 2px 0 rgb(0 0 0 / 0.05);
  --shadow-md: 0 4px 6px -1px rgb(0 0 0 / 0.1);
  --shadow-lg: 0 10px 15px -3px rgb(0 0 0 / 0.1);

  /* Transitions */
  --transition-fast: 150ms cubic-bezier(0.4, 0, 0.2, 1);
  --transition-base: 200ms cubic-bezier(0.4, 0, 0.2, 1);
  --transition-slow: 300ms cubic-bezier(0.4, 0, 0.2, 1);
}

[data-theme="dark"] {
  --color-bg-primary: #111827;
  --color-bg-secondary: #1f2937;
  --color-text-primary: #f9fafb;
  --color-text-secondary: #9ca3af;
}
```

### Component Scoped Styles

```svelte
<style>
  .workflow-card {
    background: var(--color-bg-primary);
    border-radius: 0.5rem;
    padding: var(--spacing-md);
    box-shadow: var(--shadow-md);
    transition: transform var(--transition-fast);
  }

  .workflow-card:hover {
    transform: translateY(-2px);
  }

  .workflow-card.active {
    border: 2px solid var(--color-accent);
  }
</style>
```

## 10. Testing Strategy

### Unit Tests (Vitest)
```ts
import { render } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import WorkflowCard from '$lib/components/workflow/WorkflowCard.svelte';

describe('WorkflowCard', () => {
  it('renders workflow name', () => {
    const { getByText } = render(WorkflowCard, {
      workflow: { id: '1', name: 'Test Workflow', status: 'idle' }
    });
    expect(getByText('Test Workflow')).toBeInTheDocument();
  });
});
```

### E2E Tests (Playwright via MCP)
```ts
import { test, expect } from '@playwright/test';

test('create and execute workflow', async ({ page }) => {
  await page.goto('http://localhost:5173/agent');

  // Create workflow
  await page.click('button:has-text("+ New")');
  await page.fill('input[name="workflow-name"]', 'E2E Test');
  await page.click('button:has-text("Create")');

  // Send message
  await page.fill('textarea', 'Query users from database');
  await page.click('button:has-text("Send")');

  // Verify execution
  await expect(page.locator('.status-running')).toBeVisible();
});
```

## Références

### Documentation Officielle
- **SvelteKit**: https://kit.svelte.dev/docs
- **Svelte 5 Runes**: https://svelte.dev/docs/svelte/what-are-runes
- **Tauri IPC**: https://v2.tauri.app/develop/calling-rust/

### UX/UI Best Practices
- [Sidebar Navigation Design](https://www.nngroup.com/articles/vertical-nav/)
- [UX Best Practices 2025](https://uiuxdesigntrends.com/best-ux-practices-for-sidebar-menu-in-2025/)
- [Multi-Workflow Task Management](https://www.guru99.com/workflow-management-software-tool.html)

### Performance
- [Token Counter Patterns](https://github.com/open-webui/open-webui/discussions/5455)
- [SaaS UI Workflows](https://gist.github.com/mpaiva-cc/d4ef3a652872cb5a91aa529db98d62dd)
