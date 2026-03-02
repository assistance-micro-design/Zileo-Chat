# Frontend Specifications

> **Stack**: SvelteKit 2.49.1 | Svelte 5.53.6 | Vite 7.2.6 | Tauri 2
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

### Architecture Route-Based

> **Refactoring Dec 2025**: Migration d'une architecture scroll-based vers route-based pour améliorer les performances et l'expérience utilisateur.

```
/settings
  +layout.svelte   (navigation sidebar)
  +layout.ts       (pathname data)
  +page.svelte     (redirect → /settings/providers)
  /providers/+page.svelte    → LLMSection + APIKeysSection
  /agents/+page.svelte       → AgentSettings (lazy)
  /mcp/+page.svelte          → MCPSection
  /memory/+page.svelte       → MemorySettings + MemoryList (lazy)
  /validation/+page.svelte   → ValidationSettings
  /prompts/+page.svelte      → PromptSettings
  /skills/+page.svelte       → SkillSettings
  /import-export/+page.svelte → ImportExportSettings
  /theme/+page.svelte        → Theme selection + Security info
```

**Avantages Route-Based**:
- **Performance**: Code splitting par section (chargement uniquement de la section demandée)
- **Navigation**: URLs partageables, historique browser natif, Back/Forward fonctionnels
- **SEO/A11y**: Routes sémantiques, navigation clavier native
- **Maintenabilité**: Fichiers plus petits et spécialisés (~50-100 lignes vs 798 lignes)

### Sidebar Layout

```
┌────────────────┬─────────────────────────────┐
│                │                             │
│  Providers     │  Content: Section Page      │
│  Agents        │  (loaded via SvelteKit      │
│  MCP           │   route)                    │
│  Memory        │                             │
│  Validation    │                             │
│  Prompts       │                             │
│  Skills        │                             │
│  Import/Export │                             │
│  Theme         │                             │
│                │                             │
│ [◀] Collapse   │  [Security Badge]           │
└────────────────┴─────────────────────────────┘
```

### Navigation Implementation

**Layout with Route-Based Active Section** (Svelte 5 runes)
```svelte
<!-- src/routes/settings/+layout.svelte -->
<script lang="ts">
  import { Sidebar } from '$lib/components/layout';
  import { Globe, Bot, Plug, Brain, ShieldCheck, BookOpen, FolderSync, Palette } from '@lucide/svelte';

  let { data, children } = $props();

  const sectionDefs = [
    { id: 'providers', route: '/settings/providers', labelKey: 'settings_providers', icon: Globe },
    { id: 'agents', route: '/settings/agents', labelKey: 'settings_agents', icon: Bot },
    { id: 'mcp', route: '/settings/mcp', labelKey: 'settings_mcp_servers', icon: Plug },
    { id: 'memory', route: '/settings/memory', labelKey: 'settings_memory', icon: Brain },
    { id: 'validation', route: '/settings/validation', labelKey: 'settings_validation', icon: ShieldCheck },
    { id: 'prompts', route: '/settings/prompts', labelKey: 'settings_prompts', icon: BookOpen },
    { id: 'skills', route: '/settings/skills', labelKey: 'settings_skills', icon: GraduationCap },
    { id: 'import-export', route: '/settings/import-export', labelKey: 'settings_import_export', icon: FolderSync },
    { id: 'theme', route: '/settings/theme', labelKey: 'settings_theme', icon: Palette }
  ];

  // URL-driven active section (derived from pathname)
  let activeSection = $derived.by(() => {
    const section = sectionDefs.find(s => data.pathname.startsWith(s.route));
    return section?.id ?? 'providers';
  });
</script>
```

**Cross-Page Communication** (Event-based refresh):
```typescript
// After import, dispatch refresh event
window.dispatchEvent(new CustomEvent('settings:refresh'));

// All section pages listen and reload
onMount(() => {
  window.addEventListener('settings:refresh', handleRefresh);
  return () => window.removeEventListener('settings:refresh', handleRefresh);
});
```

**Animation**: Transition smooth (200-300ms) selon [UX Best Practices](https://uiuxdesigntrends.com/best-ux-practices-for-sidebar-menu-in-2025/)

### Sections Détaillées

#### Providers
- Liste providers disponibles (Mistral, Ollama + custom OpenAI-compatible)
- Builtin providers : Mistral (cloud), Ollama (local)
- Custom providers : OpenAI-compatible (RouterLab, OpenRouter, Together AI, etc.)
  - Bouton "Add Provider" ouvre CustomProviderForm modal
  - Champs : Provider ID (auto-generated), Display Name, Base URL, API Key
- Configuration par provider :
  - API Key (input type="password")
  - Endpoint URL (custom + Ollama)
- Toggle enable/disable
- Test connection (button + status indicator)
- Delete custom provider (with confirmation)

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

**Implementation Status**: Complete (v0.9.1)

**Mode de validation** (radio buttons) :
- **Auto** : Execute sans confirmation (affiche liste des outils/MCP avec badge "Auto-approved")
- **Manual** : Demande confirmation pour tout (affiche liste avec badge "Requires approval")
- **Selective** : Configuration granulaire par type d'operation

**Configuration selective** (checkboxes, visible en mode Selective) :
- Local Tools validation (ON/OFF) - MemoryTool, TodoTool, CalculatorTool, etc.
- Sub-agents validation (ON/OFF) - SpawnAgentTool, DelegateTaskTool, ParallelTasksTool
- MCP calls validation (ON/OFF) - Tous les appels aux serveurs MCP externes
- File operations validation (ON/OFF) - Reserve pour futur
- Database operations validation (ON/OFF) - Reserve pour futur

**Seuils de risque** (overrides appliques par-dessus le mode) :
- Auto-approve LOW risk (checkbox) : En mode Manual, ignore validation pour Low risk
- Always confirm HIGH risk (checkbox) : En mode Auto, force validation pour High risk

**Affichage dynamique** :
- Chaque mode affiche la liste des outils locaux et serveurs MCP disponibles
- Badges visuels indiquant le statut : "Auto-approved" (vert) ou "Requires approval" (orange)
- En mode Selective, les badges refletent la configuration des toggles

**Futur (non implemente)** :
- Timeout validation request (slider 30s - 5min)
- Comportement timeout : Reject | Approve | Ask Again
- Audit settings (logging, retention, export)

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
- Toast notifications (via `toastStore` + `ToastContainer`/`ToastItem`) :
  - Background workflow completed (auto-dismiss 5s)
  - Background workflow failed (auto-dismiss 5s)
  - User question pending on background workflow (persistent until dismissed)
  - Concurrent workflow limit reached (warning)
  - "Go to workflow" action button for navigation

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

### Concurrent Execution Limits

| Validation Mode | Max Concurrent | Behavior |
|----------------|----------------|----------|
| Auto | 3 | Multiple workflows run in background |
| Manual | 1 | Single workflow at a time |
| Selective | 1 | Single workflow at a time |

**Enforcement**: Frontend (`backgroundWorkflowsStore.canStart()`) + Backend safety net (`streaming_cancellations.len() >= 3`).

When limit is reached, a warning toast is shown to the user. Background workflows fire toast notifications for completion and user questions when not currently viewed.

See `WORKFLOW_ORCHESTRATION.md > Background Workflow Execution` for full architecture.

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

### Component Library (96 Total Components)

```
src/lib/components/
├─ ui/                  # 15 atomic UI components
│  ├─ Button.svelte
│  ├─ Badge.svelte
│  ├─ Card.svelte
│  ├─ Input.svelte
│  ├─ Select.svelte
│  ├─ Textarea.svelte
│  ├─ Modal.svelte
│  ├─ Spinner.svelte
│  ├─ ProgressBar.svelte
│  ├─ StatusIndicator.svelte
│  ├─ Skeleton.svelte
│  ├─ LanguageSelector.svelte
│  ├─ ToastContainer.svelte
│  └─ ToastItem.svelte
├─ layout/              # 4 layout containers
│  ├─ AppContainer.svelte
│  ├─ Sidebar.svelte
│  ├─ RightSidebar.svelte
│  └─ FloatingMenu.svelte
├─ navigation/          # 1 navigation element
│  └─ NavItem.svelte
├─ agent/               # 3 agent page sections (2-column layout)
│  ├─ AgentHeader.svelte
│  ├─ ChatContainer.svelte
│  └─ WorkflowSidebar.svelte
├─ chat/                # 8 chat components
│  ├─ ChatInput.svelte
│  ├─ MessageBubble.svelte
│  ├─ MessageList.svelte
│  ├─ MessageListSkeleton.svelte
│  ├─ PromptSelectorModal.svelte
│  ├─ ReasoningStep.svelte
│  ├─ StreamingMessage.svelte
│  └─ ToolExecution.svelte
├─ workflow/            # 16 workflow components
│  ├─ ActivityFeed.svelte
│  ├─ ActivityItem.svelte
│  ├─ ActivityItemDetails.svelte
│  ├─ AgentSelector.svelte
│  ├─ ConfirmDeleteModal.svelte
│  ├─ MetricsBar.svelte
│  ├─ NewWorkflowModal.svelte
│  ├─ ReasoningPanel.svelte
│  ├─ SubAgentActivity.svelte
│  ├─ TokenDisplay.svelte
│  ├─ ToolExecutionPanel.svelte
│  ├─ UserQuestionModal.svelte
│  ├─ ValidationModal.svelte
│  ├─ WorkflowItem.svelte
│  ├─ WorkflowItemCompact.svelte
│  └─ WorkflowList.svelte
├─ settings/            # Includes CustomProviderForm.svelte
│  └─ CustomProviderForm.svelte  # Modal form for adding OpenAI-compatible providers
├─ mcp/                 # 3 MCP management components
│  ├─ MCPServerCard.svelte
│  ├─ MCPServerForm.svelte
│  └─ MCPServerTester.svelte
├─ llm/                 # 4 LLM management components
│  ├─ ConnectionTester.svelte
│  ├─ ModelCard.svelte
│  ├─ ModelForm.svelte
│  └─ ProviderCard.svelte
├─ settings/            # 24 settings components
│  ├─ agents/           # Agent CRUD (3)
│  │  ├─ AgentSettings.svelte
│  │  ├─ AgentList.svelte
│  │  └─ AgentForm.svelte
│  ├─ memory/           # Memory CRUD (3)
│  │  ├─ MemorySettings.svelte
│  │  ├─ MemoryList.svelte
│  │  └─ MemoryForm.svelte
│  ├─ prompts/          # Prompt CRUD (3)
│  │  ├─ PromptSettings.svelte
│  │  ├─ PromptList.svelte
│  │  └─ PromptForm.svelte
│  ├─ skills/           # Skill CRUD (3)
│  │  ├─ SkillSettings.svelte
│  │  ├─ SkillList.svelte
│  │  └─ SkillForm.svelte
│  ├─ validation/       # Validation config (1)
│  │  └─ ValidationSettings.svelte
│  └─ import-export/    # Data portability (9)
│     ├─ ImportExportSettings.svelte
│     ├─ ExportPanel.svelte
│     ├─ ImportPanel.svelte
│     ├─ EntitySelector.svelte
│     ├─ ExportPreview.svelte
│     ├─ ImportPreview.svelte
│     ├─ ConflictResolver.svelte
│     ├─ MCPFieldEditor.svelte
│     └─ MCPEnvEditor.svelte
└─ onboarding/          # 9 first-launch wizard components
   ├─ OnboardingModal.svelte
   ├─ OnboardingProgress.svelte
   └─ steps/
      ├─ StepWelcome.svelte
      ├─ StepLanguage.svelte
      ├─ StepTheme.svelte
      ├─ StepApiKey.svelte
      ├─ StepValues.svelte
      ├─ StepImport.svelte
      └─ StepComplete.svelte
```

### Stores (18 Total)

| Store | Type | Key Exports | Description |
|-------|------|-------------|-------------|
| `theme` | custom | `theme`, `setTheme()`, `toggle()`, `init()` | Light/dark mode with localStorage persistence |
| `agents` | custom | `agentStore`, `agents`, `selectedAgent`, `isLoading`, `hasAgents`, `agentCount` | Agent CRUD with reactive state |
| `workflows` | custom | `workflowStore`, `workflows`, `selectedWorkflow`, `filteredWorkflows` | Workflow management (pure functions + reactive store) |
| `locale` | custom | `localeStore`, `locale`, `localeInfo` | i18n language management |
| `llm` | pure functions | `createInitialLLMState()`, `loadModels()`, `updateProviderSettings()`, `listProviders()`, `createCustomProvider()`, `deleteCustomProvider()`, `loadAllLLMData()` | LLM provider/model state + custom provider CRUD |
| `mcp` | pure functions | `createInitialMCPState()`, `loadServers()`, `testServer()`, `callTool()` | MCP server state |
| `streaming` | custom | `streamingStore`, `isStreaming`, `streamContent`, `activeTools`, `reasoningSteps` | Real-time workflow execution |
| `activity` | custom | `activityStore`, `historicalActivities`, `allActivities`, `filteredActivities` | Workflow activity tracking |
| `prompts` | custom | `promptStore`, `prompts`, `selectedPrompt`, `hasPrompts` | Prompt library management |
| `skills` | custom | `skillStore`, `skills`, `selectedSkill`, `hasSkills`, `enabledSkills` | Skill CRUD (createCRUDStore factory) |
| `validation` | custom | `validationStore`, `hasPendingValidation`, `pendingValidation` | Human-in-the-loop requests |
| `tokens` | custom | `tokenStore`, `tokenDisplayData`, `streamingTokens`, `cumulativeTokens` | Token usage/cost tracking |
| `validation-settings` | custom | N/A | Validation configuration |
| `onboarding` | custom | N/A | First-launch wizard state |
| `backgroundWorkflows` | custom | `backgroundWorkflowsStore`, `runningWorkflows`, `canStartNew`, `runningWorkflowIds`, `questionPendingIds` | Central dispatch for concurrent background workflow execution (PAT_STORE_004) |
| `toast` | custom | `toastStore`, `toasts`, `visibleToasts`, `hasToasts`, `navigationTarget` | Toast notifications for background workflow events |
| `index` | barrel | All stores | Re-exports all stores |

### Types (26 Modules in src/types/)

| Module | Key Types | Description |
|--------|-----------|-------------|
| `agent.ts` | `Agent`, `AgentConfig`, `AgentConfigCreate`, `AgentSummary`, `LLMConfig` | Agent configuration |
| `workflow.ts` | `Workflow`, `WorkflowResult`, `WorkflowMetrics`, `WorkflowFullState` | Workflow execution |
| `llm.ts` | `LLMModel`, `ProviderSettings`, `ConnectionTestResult`, `LLMState`, `ProviderType` | LLM providers (ProviderType = BuiltinProvider \| string) |
| `customProvider.ts` | `ProviderInfo`, `CreateCustomProviderRequest` | Custom OpenAI-compatible provider types |
| `mcp.ts` | `MCPServer`, `MCPServerConfig`, `MCPTool`, `MCPTestResult` | MCP servers |
| `streaming.ts` | `StreamChunk`, `WorkflowComplete`, `ChunkType` | Streaming events |
| `message.ts` | `Message` | Chat messages |
| `task.ts` | `Task` | Todo/task items |
| `tool.ts` | `ToolExecution`, `WorkflowToolExecution` | Tool execution |
| `thinking.ts` | `ThinkingStep` | Reasoning steps |
| `sub-agent.ts` | `SubAgentExecution`, `ValidationRequiredEvent` | Sub-agent execution |
| `validation.ts` | `ValidationRequest`, `ValidationType`, `RiskLevel` | Validation requests |
| `prompt.ts` | `Prompt`, `PromptCreate`, `PromptSummary`, `PromptCategory` | Prompt library |
| `skill.ts` | `Skill`, `SkillCreate`, `SkillUpdate`, `SkillSummary`, `SkillCategory`, `SKILL_CATEGORY_LABELS` | Skill documents |
| `activity.ts` | `WorkflowActivityEvent`, `ActivityFilter` | Activity events |
| `background-workflow.ts` | `BackgroundWorkflowStatus`, `WorkflowStreamState`, `Toast`, `ToastType` | Background workflow execution and toast notifications |
| `memory.ts` | `Memory`, `MemoryType` | Memory/RAG |
| `embedding.ts` | Embedding config types | Vector embeddings |
| `services.ts` | `ModalState` | Service layer |
| `security.ts` | `LLMProvider` | Security/credentials |
| `function_calling.ts` | Function calling schemas | LLM function calling |
| `importExport.ts` | Import/export structures | Backup/restore |
| `i18n.ts` | `Locale`, `LocaleInfo`, `LOCALES` | Internationalization |
| `onboarding.ts` | Onboarding state types | First-launch wizard |
| `index.ts` | All types | Barrel export |

### Utilities (src/lib/utils/)

| Module | Key Exports | Description |
|--------|-------------|-------------|
| `modal.svelte.ts` | `createModalController<T>()`, `ModalController`, `ModalMode` | Factory for modal state management (show/mode/editing) using Svelte 5 runes |
| `async.ts` | `createAsyncHandler()`, `createAsyncHandlerWithEvent()`, `withLoadingState()` | Async operation wrappers with loading/error handling |
| `error.ts` | `getErrorMessage()`, `formatErrorForDisplay()` | Error extraction and formatting utilities |
| `activity.ts` | `combineActivities()`, `filterActivities()`, `countActivitiesByType()` | Activity feed helpers |
| `activity-icons.ts` | `ACTIVITY_TYPE_ICONS`, `getActivityIcon()` | Consolidated activity type icon mapping |
| `duration.ts` | `formatDuration()` | Duration formatting utility |
| `debounce.ts` | `debounce()` | Debounce function wrapper |
| `index.ts` | All utilities | Barrel export |

### Services (src/lib/services/)

| Module | Key Exports | Description |
|--------|-------------|-------------|
| `message.service.ts` | `MessageService.load()`, `MessageService.save()` | Message CRUD with error handling (returns `{ messages, error? }`) |
| `workflow.service.ts` | `WorkflowService.execute()`, `WorkflowService.cancel()` | Workflow execution management |
| `workflowExecutor.service.ts` | `WorkflowExecutorService.execute()` | 8-step workflow orchestration with concurrency check and view-aware guards |
| `localStorage.service.ts` | `LocalStorage.get()`, `LocalStorage.set()`, `STORAGE_KEYS` | Typed localStorage access |
| `index.ts` | All services | Barrel export |

**WorkflowExecutorService Pattern**:
```typescript
// Extracted 8-step orchestration from handleSend
await WorkflowExecutorService.execute(
  {
    workflowId: 'wf-123',
    message: 'User message',
    agentId: 'agent-456',
    locale: 'en'
  },
  {
    onUserMessage: (msg) => pageState.messages.push(msg),
    onAssistantMessage: (msg) => pageState.messages.push(msg),
    onError: (msg) => pageState.messages.push(msg)
  }
);
```

**localStorage Service Pattern**:
```typescript
import { LocalStorage, STORAGE_KEYS } from '$lib/services';

// Type-safe access with defaults
const collapsed = LocalStorage.get(STORAGE_KEYS.RIGHT_SIDEBAR_COLLAPSED, false);
LocalStorage.set(STORAGE_KEYS.SELECTED_AGENT_ID, agentId);
```

### PageState Pattern

Aggregate page state into a single reactive object instead of multiple `$state()` variables:

```typescript
interface PageState {
  leftSidebarCollapsed: boolean;
  rightSidebarCollapsed: boolean;
  selectedWorkflowId: string | null;
  selectedAgentId: string | null;
  currentMaxIterations: number;
  currentContextWindow: number;
  messages: Message[];
  messagesLoading: boolean;
}

const initialPageState: PageState = { /* defaults */ };
let pageState = $state<PageState>(initialPageState);

// Usage
pageState.messages = [...pageState.messages, newMessage];
pageState.selectedWorkflowId = workflow.id;
```

### Streaming Store Consolidation

Derived stores reduced from 28 to 14. Use direct filtering instead of deprecated helpers:

```typescript
// DEPRECATED (removed)
// import { hasRunningTools, subAgentCount } from '$lib/stores/streaming';

// RECOMMENDED
import { runningTools, activeSubAgents } from '$lib/stores/streaming';

// In component
const hasRunning = $runningTools.length > 0;
const count = $activeSubAgents.length;
const running = $activeSubAgents.filter(a => a.status === 'running');
```

**Modal Controller Pattern** (Phase 7 Quick Win):
```typescript
// Creates reactive modal state with create/edit modes
const mcpModal = createModalController<MCPServerConfig>();

// Usage
mcpModal.openCreate();          // Opens in create mode
mcpModal.openEdit(server);      // Opens in edit mode with item
mcpModal.close();               // Closes and clears state

// Template
{#if mcpModal.show}
  <Modal title={mcpModal.mode === 'create' ? 'Add' : 'Edit'}>
    <Form data={mcpModal.editing} />
  </Modal>
{/if}
```

**Async Handler Pattern** (Phase 7 Quick Win):
```typescript
// Wraps async operations with loading state and error handling
const handleSave = createAsyncHandler(
  () => invoke('save_data', { data }),
  {
    setLoading: (l) => saving = l,
    onSuccess: () => message = { type: 'success', text: 'Saved' },
    onError: (e) => message = { type: 'error', text: getErrorMessage(e) }
  }
);
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

### Settings Page Optimizations (Dec 2025)

> Migration from scroll-based to route-based navigation with comprehensive performance optimizations.

| Optimization | Status | Impact | Location |
|-------------|--------|--------|----------|
| Route-based navigation | Active | Code splitting, lazy loading | `src/routes/settings/*` |
| Modal backdrop fix | Active | 15-30% GPU improvement | `global.css:694` |
| GPU scroll acceleration | Active | GPU acceleration | `+layout.svelte:254` |
| CSS containment on grids | Active | ~10% layout time reduction | Grid sections CSS |
| Memoized selectors | Active | ~5-10% JS execution reduction | `llm.ts` memoization |
| Virtual scrolling | Active | ~20 DOM nodes vs 20000 | `MemoryList.svelte` |
| Animation pause on scroll | Active | ~5% GPU during scroll | `global.css:889` |

**Modal Backdrop** - Removed expensive `backdrop-filter: blur(4px)`, replaced with `rgba(0,0,0,0.6)`.

**GPU Scroll Acceleration** - Added `will-change: scroll-position` to content area.

**CSS Containment on Grids** - Applied `contain: layout style` to:
- `.mcp-server-grid` (MCPSection)
- `.provider-grid`, `.models-grid` (LLMSection)
- `.agent-grid` (AgentList)

**Memoized Selectors** - `getFilteredModelsMemoized()` with cache key strategy.

**Virtual Scrolling** - MemoryList uses `@humanspeak/svelte-virtual-list` for 1000+ items.

**Animation Pause** - `.is-scrolling` class pauses animations during scroll.

### Messages Area Optimizations (Dec 2025)

Messages Area optimizations for Agent page.

| Optimization | Status | Impact | Location |
|-------------|--------|--------|----------|
| Conditional animations | Active | 60% GPU reduction (green/warning states) | `TokenDisplay.svelte` |
| Duration utility | Active | DRY utility | `src/lib/utils/duration.ts` |
| Const icon map | Active | 1 less object allocation per render | `ActivityFeed.svelte:52-58` |
| Activity icons module | Active | Single source of truth | `src/lib/utils/activity-icons.ts` |
| Virtual scroll ActivityFeed | Active | 90% DOM reduction for 100+ activities | `ActivityFeed.svelte` |
| ActivityItemDetails extraction | Active | Overflow fixes, component extraction | `ActivityItemDetails.svelte` |

**Conditional Animations** - TokenDisplay pulse animations activate only when `warningLevel === 'critical'`.

**ActivityFeed Virtual Scroll** - Uses `@humanspeak/svelte-virtual-list` with 20-item threshold:
```svelte
const VIRTUAL_SCROLL_THRESHOLD = 20;
const useVirtualScroll = $derived(activities.length >= VIRTUAL_SCROLL_THRESHOLD);

<SvelteVirtualList
  items={activities}
  defaultEstimatedItemHeight={72}
  bufferSize={10}
>
```

**ActivityItemDetails Extraction** - Task details extracted to dedicated component for reduced complexity.

### General Optimization Strategies

**CSS Containment** (Phase 6 - built-in optimization)
```svelte
<!-- MessageList.svelte uses CSS containment for long lists -->
<div
  class="message-list"
  class:performance-mode={messages.length > 50}
>
  {#each messages as message (message.id)}
    <div class="message-wrapper" class:optimize={messages.length > 50}>
      <MessageBubble {message} />
    </div>
  {/each}
</div>

<style>
  /* Enable containment for long lists */
  .message-list.performance-mode {
    contain: strict;
    will-change: scroll-position;
  }

  /* Use content-visibility for off-screen messages */
  .message-wrapper.optimize {
    content-visibility: auto;
    contain-intrinsic-size: 0 100px;
  }
</style>
```

**Virtual Scrolling** (listes >100 items - alternative approach)
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

## 11. Phase 6 Additions

### Skeleton Loading States

**Skeleton Component** (`src/lib/components/ui/Skeleton.svelte`)
```svelte
<script lang="ts">
  export type SkeletonVariant = 'text' | 'circular' | 'rectangular';

  interface Props {
    variant?: SkeletonVariant;
    width?: string;
    height?: string;
    size?: string;
    animate?: boolean;
  }
</script>

<!-- Usage -->
<Skeleton variant="text" width="200px" />
<Skeleton variant="circular" size="48px" />
<Skeleton variant="rectangular" width="100%" height="120px" />
```

**MessageListSkeleton** (`src/lib/components/chat/MessageListSkeleton.svelte`)
```svelte
<!-- Shows placeholder message bubbles during loading -->
<MessageListSkeleton count={3} />
```

### Transition Animations

**Panel Transitions** (ToolExecutionPanel, ReasoningPanel)
```css
.tool-execution-panel,
.reasoning-panel {
  transition: all var(--transition-base, 200ms) ease-out;
}

.panel.expanded {
  box-shadow: var(--shadow-sm);
}

.execution-list,
.step-list {
  animation: slideDown 200ms ease-out;
}

@keyframes slideDown {
  from { opacity: 0; max-height: 0; }
  to { opacity: 1; max-height: 200px; }
}

.execution-item,
.step-item {
  animation: fadeInItem 150ms ease-out;
}

@keyframes fadeInItem {
  from { opacity: 0; transform: translateX(-8px); }
  to { opacity: 1; transform: translateX(0); }
}
```

### Backend Pagination

**Paginated Message Loading** (for long conversation histories)
```typescript
// TypeScript
interface PaginatedMessages {
  messages: Message[];
  total: number;
  offset: number;
  limit: number;
  has_more: boolean;
}

// Usage
const result = await invoke<PaginatedMessages>('load_workflow_messages_paginated', {
  workflowId: 'uuid',
  limit: 50,
  offset: 0
});
```

```rust
// Rust command
#[tauri::command]
pub async fn load_workflow_messages_paginated(
    workflow_id: String,
    limit: Option<u32>,  // Default: 50, max: 200
    offset: Option<u32>, // Default: 0
    state: State<'_, AppState>,
) -> Result<PaginatedMessages, String>
```

### E2E Tests

**Workflow Persistence Tests** (`tests/e2e/workflow-persistence.spec.ts`)
- Skeleton loading display verification
- Workflow selection persistence across reload
- Tool execution panel expansion
- Reasoning panel expansion
- Message list accessibility attributes
- Keyboard navigation in workflow list
- Empty workflow state handling
- Metrics bar display
- Responsive sidebar toggle
- Scroll position maintenance
- Streaming indicator animation

## References

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
