---
description: Workflow d'implémentation complet pour Zileo-Chat-3 avec validation qualité
allowed-tools: Task(subagent_type:*), mcp__serena__*, mcp__context7__*, mcp__sequential-thinking__*, Glob, Read, Write, Edit, MultiEdit, Bash(git:*, npm:*, cargo:*), TodoWrite
argument-hint: <description-implémentation>
---

# Workflow d'Implémentation Zileo-Chat-3

**Stack**: SvelteKit 2.49.0 + Svelte 5.43.14 | Rust 1.91.1 + Tauri 2.9.4 | SurrealDB 2.3.10

## Objectif

Implémenter `$ARGUMENTS` avec les standards de qualité SuperClaude, validation complète (lint, typecheck, tests), puis générer un rapport détaillé du travail accompli.

---

## Configuration

```
PROJECT_ROOT: (working directory)
TASK_REPORTS: docs/taches
SRC_FRONTEND: src
SRC_BACKEND: src-tauri
TYPES_DIR: src/types (alias: $types)
```

**IMPORTANT - TypeScript Imports**:
- Always use `$types` alias: `import type { X } from '$types/module'`
- Never use `$lib/types` (does not exist)

**IMPORTANT - Tauri IPC Parameter Naming**:
- Rust commands use `snake_case` parameters (e.g., `workflow_id`, `default_model_id`)
- TypeScript `invoke()` calls use `camelCase` parameters (e.g., `workflowId`, `defaultModelId`)
- Tauri converts automatically between the two formats
- Single-word params remain unchanged (e.g., `id`, `name`, `provider`)

**Complexité**: [auto|simple|medium|complex|critical]
- **simple**: <3 étapes, 1-2 fichiers
- **medium**: 3-7 étapes, 3-10 fichiers
- **complex**: >7 étapes, >10 fichiers, architecture
- **critical**: Système complet, sécurité, production

---

## Principes

- **EXCELLENCE**: Code production-ready, pas de placeholders/TODO/mock
- **SINCÉRITÉ**: Communication honnête, pas de langage marketing
- **PARALLEL-FIRST**: Opérations indépendantes en parallèle
- **COMPLETE**: Finir à 100% ce qui est commencé
- **VALIDATED**: Lint + typecheck + tests - ZÉRO erreur

---

## Interdictions Strictes

**Code**:
- ❌ **Emojis** dans code/commentaires
- ❌ **Type 'any'** - typage strict obligatoire
- ❌ **Mock data** - données réelles ou génération valide
- ❌ **TODO comments** pour fonctionnalités core
- ❌ **Code incomplet** - finir ce qui est commencé
- ❌ **Placeholders** en production

**Process**:
- ❌ **Skip tests** pour faire passer le build
- ❌ **Skip validation** pour résoudre rapidement
- ❌ **Accélérer** au détriment de la qualité

---

## Workflow

### Phase 0: Préparation

**Git & Context**:
```bash
1. git status && git branch
2. git checkout -b feature/<nom> si nécessaire
3. Serena: list_memories() → read_memory() si contexte précédent
```

**Analyse Complexité**:
```
Évaluer:
├─ Nombre d'étapes estimé
├─ Fichiers impactés (frontend/backend/db)
├─ Dépendances et risques
└─ Définir COMPLEXITY_LEVEL (auto si unclear)
```

### Phase 1: Analyse & Planning

**1.1 Décomposition**:
```
Extraire de $ARGUMENTS:
├─ Objectif principal
├─ Fonctionnalités requises
├─ Contraintes techniques (Tauri IPC, async Rust, etc.)
├─ Critères d'acceptation
└─ Dépendances identifiées
```

**1.2 Exploration Parallèle**:
```
Operations parallèles:
├─ Read fichiers clés
├─ Grep patterns pertinents
├─ Glob fichiers similaires
└─ Serena: get_symbols_overview() pour structure

Si complexité >medium:
└─ Task agent (subagent_type=Explore) pour discovery
```

**1.3 Patterns & Architecture**:
```
Identifier:
├─ Components Svelte réutilisables (src/lib/components/)
├─ Commands Rust existantes (src-tauri/src/commands/)
├─ Stores Svelte (src/lib/stores/)
├─ Types existants (src/types/ + src-tauri/src/models/)
├─ Patterns IPC Tauri (invoke)
└─ Queries SurrealDB similaires
```

**1.4 TodoWrite** (si >3 étapes):
```
TodoWrite avec:
├─ Tâches atomiques Frontend/Backend/Types/DB
├─ Identifier tâches parallélisables
├─ Marquer dépendances séquentielles
└─ Estimer effort

Si complexité >medium:
└─ Serena: write_memory("plan_<task>", plan_détaillé)
```

### Phase 2: Implémentation

**2.1 Types & Contracts** (toujours en premier):

**Frontend** (`src/types/feature.ts`):
```typescript
/**
 * Description du type
 */
export interface FeatureData {
  /** ID unique */
  id: string;
  /** Nom de la feature */
  name: string;
  /** Métadonnées additionnelles */
  metadata: Record<string, unknown>;
}

/** Statuts possibles */
export type FeatureStatus = 'pending' | 'active' | 'completed';
```

**Backend** (`src-tauri/src/models/feature.rs`):
```rust
use serde::{Deserialize, Serialize};

/// Données de la feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureData {
    /// ID unique
    pub id: String,
    /// Nom de la feature
    pub name: String,
    /// Métadonnées additionnelles
    pub metadata: serde_json::Value,
}
```

**2.2 Backend Rust** (si applicable):

**Command** (`src-tauri/src/commands/feature.rs`):
```rust
use tauri::State;

/// Description de ce que fait la command
#[tauri::command]
pub async fn feature_action(
    param: String,
    state: State<'_, AppState>
) -> Result<ReturnType, String> {
    // Implementation
    // Error handling avec Result
    Ok(result)
}
```

**Enregistrement** (`src-tauri/src/main.rs`):
```rust
.invoke_handler(tauri::generate_handler![
    commands::feature::feature_action,
    // ... autres commands
])
```

**Tests Rust**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_action() {
        // Test unitaire
    }
}
```

**2.3 Frontend Svelte** (si applicable):

**Component** (`src/lib/components/Feature.svelte`):
```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/tauri';
  import type { FeatureData } from '$types/feature';

  /** Props avec types stricts */
  interface Props {
    data: FeatureData;
    onUpdate?: (updated: FeatureData) => void;
  }

  let { data, onUpdate }: Props = $props();

  /** État local */
  let loading = $state(false);

  /**
   * Appel command Tauri
   */
  async function handleAction() {
    loading = true;
    try {
      const result = await invoke<FeatureData>('feature_action', {
        param: data.id
      });
      onUpdate?.(result);
    } catch (error) {
      console.error('Error:', error);
    } finally {
      loading = false;
    }
  }
</script>

<!-- Template avec accessibilité -->
<button
  onclick={handleAction}
  disabled={loading}
  aria-busy={loading}
>
  {loading ? 'Loading...' : 'Action'}
</button>
```

**Store Svelte** (si state global, importer depuis `$types`):
```typescript
// src/lib/stores/feature.ts
import { writable } from 'svelte/store';
import type { FeatureData } from '$types/feature';  // ALWAYS use $types alias

/**
 * Store pour gérer l'état de la feature
 */
export const featureStore = writable<FeatureData[]>([]);

/**
 * Actions sur le store
 */
export const featureActions = {
  add: (item: FeatureData) => {
    featureStore.update(items => [...items, item]);
  },
  remove: (id: string) => {
    featureStore.update(items => items.filter(i => i.id !== id));
  }
};
```

**2.4 Database SurrealDB** (si schéma nécessaire):
```surql
-- Schema definition
DEFINE TABLE feature SCHEMAFULL;
DEFINE FIELD id ON feature TYPE string;
DEFINE FIELD name ON feature TYPE string;
DEFINE FIELD created_at ON feature TYPE datetime DEFAULT time::now();

-- Indexes
DEFINE INDEX idx_name ON feature FIELDS name;
```

**2.5 Documentation Inline**:
```
Pour CHAQUE fonction/type/variable:
- JSDoc/TSDoc (TypeScript)
- Rustdoc (Rust)
- Description claire de l'objectif
- @param avec types et descriptions
- @returns avec type et description
- @throws si applicable
- @example si complexe
```

**2.6 Stratégie Éditions**:
```
├─ MultiEdit: >3 fichiers similaires
├─ Edit: Modifications ciblées
├─ Write: Nouveaux fichiers uniquement si nécessaire
├─ Serena replace_symbol_body: Modifications symboliques
└─ Batch parallèle pour opérations indépendantes
```

**2.7 Checkpoints** (tous les 30min ou après tâche majeure):
```
1. TodoWrite: Mise à jour statuts (in_progress → completed)
2. Git: Commit incrémental descriptif
3. Serena: write_memory("checkpoint_<timestamp>", état)
4. Validation partielle (lint fichiers modifiés)
```

### Phase 3: Validation

**3.1 Validation Frontend**:
```bash
# Séquentiellement
1. npm run lint    # ZÉRO erreur acceptée
2. npm run check   # TypeScript strict
3. npm run test    # Si tests unitaires
```

**3.2 Validation Backend**:
```bash
# Séquentiellement
1. cargo fmt --check              # Format
2. cargo clippy -- -D warnings    # Lint
3. cargo test                     # Tests unitaires
4. cargo build --release          # Compilation
```

**3.3 Tests E2E** (si UI):
```bash
# Playwright si parcours critique
npx playwright test
```

**3.4 Investigation si Échecs**:
```
Si erreurs:
├─ Root cause analysis (pas de skip)
├─ Sequential pour debugging complexe
├─ Fix systématique
└─ Re-validation complète
```

### Phase 4: Revue Finale

**Checklist Qualité**:
```
- [ ] Tous TodoWrite items complétés
- [ ] Aucun TODO/FIXME/XXX dans le code
- [ ] Aucun 'any', mock data, emoji
- [ ] Types stricts partout (TS + Rust)
- [ ] Documentation complète (JSDoc/Rustdoc)
- [ ] Patterns projet respectés
- [ ] Tests passent (frontend + backend)
- [ ] Lint: 0 erreur
- [ ] Typecheck: 0 erreur
- [ ] Build: succès
```

**Git Metrics**:
```bash
git diff --stat
git diff --shortstat
```

### Phase 5: Rapport & Sauvegarde

**5.1 Commit Git**:
```bash
git add <fichiers-pertinents>
git commit -m "$(cat <<'EOF'
<type>: <description courte>

<description détaillée>

- Changement 1
- Changement 2

🤖 Generated with Claude Code
Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

**5.2 Rapport** (généré dans `docs/taches/YYYY-MM-DD_<task>.md`):
```markdown
# Rapport - [USER_PROMPT résumé]

## Métadonnées
- **Date**: YYYY-MM-DD HH:MM
- **Complexité**: [simple|medium|complex|critical]
- **Durée**: Xh XXmin
- **Stack**: Svelte 5.43 + Rust 1.91 + Tauri 2.9 + SurrealDB 2.3

## Objectif
$ARGUMENTS

## Travail Réalisé

### Fonctionnalités Implémentées
- [Feature 1] - Description technique
- [Feature 2] - Description technique

### Fichiers Modifiés

**Frontend** (Svelte/TypeScript):
- `src/routes/...` - [Action: Créé/Modifié]
- `src/lib/components/...` - [Action]
- `src/lib/stores/...` - [Action]
- `src/types/...` - [Action]

**Backend** (Rust):
- `src-tauri/src/commands/...` - [Action]
- `src-tauri/src/models/...` - [Action]
- `src-tauri/src/main.rs` - [Enregistrement commands]

**Database**:
- Schémas SurrealDB: [Si applicable]

### Statistiques Git
```
[Sortie git diff --stat]
```

### Types Créés/Modifiés

**TypeScript** (`src/types/feature.ts`):
```typescript
interface FeatureData { ... }
type FeatureStatus = ...
```

**Rust** (`src-tauri/src/models/feature.rs`):
```rust
struct FeatureData { ... }
```

### Composants Clés

**Frontend**:
- `Feature.svelte` - [Description et responsabilité]
  - Props: [Liste]
  - Events: [Liste]
  - Stores: [Utilise quels stores]

**Backend**:
- `feature_action()` - [Description command]
  - Params: [Liste]
  - Returns: [Type]
  - Errors: [Gestion]

## Décisions Techniques

### Architecture
- **Structure**: [Choix et justification]
- **IPC Tauri**: [Patterns invoke utilisés]
- **State**: [Store Svelte ou local state]
- **Database**: [Queries SurrealDB si applicable]

### Patterns Utilisés
- **Pattern 1**: [Nom] - [Justification]
- **Pattern 2**: [Nom] - [Justification]

## Validation

### Tests Frontend
- **Lint**: ✅ PASS (0 erreurs)
- **TypeCheck**: ✅ PASS (0 erreurs)
- **Unit tests**: X/X PASS

### Tests Backend
- **Clippy**: ✅ PASS (0 warnings)
- **Cargo test**: ✅ X/X PASS
- **Build release**: ✅ SUCCESS

### Qualité Code
- ✅ Types stricts (TypeScript + Rust)
- ✅ Documentation complète (JSDoc + Rustdoc)
- ✅ Standards projet respectés
- ✅ Pas de any/mock/emoji/TODO
- ✅ Accessibilité (si UI)

## Prochaines Étapes

### Suggestions
- [Amélioration future 1]
- [Optimisation possible 2]

## Métriques

### Code
- **Lignes ajoutées**: +XXX
- **Lignes supprimées**: -XXX
- **Fichiers modifiés**: X
- **Complexité**: [Si analysée]

### Performance
- [Métriques si mesurées]
```

**5.3 Serena Memory**:
```
Si complexité >medium:
└─ write_memory("session_summary_<task>", {
    objective: $ARGUMENTS,
    files_modified: [...],
    patterns_used: [...],
    key_decisions: [...],
    next_steps: [...]
  })
```

**5.4 Cleanup**:
```
└─ Supprimer fichiers temporaires
└─ Vérifier workspace propre
```

---

## Gestion Complexité

| Niveau | Critères | Outils | Validation |
|--------|----------|--------|------------|
| **Simple** | <3 étapes, 1-2 fichiers | Native tools | Lint + typecheck |
| **Medium** | 3-7 étapes, 3-10 fichiers | + Serena + TodoWrite | + Tests unitaires |
| **Complex** | >7 étapes, >10 fichiers | + Sequential + Task agents | + Tests intégration |
| **Critical** | Système complet, sécurité | + Context7 + Checkpoints 20min | + E2E + Security review |

---

## Outils MCP

### Serena (complexité >medium)
- **Session**: `list_memories()` → `read_memory()` → `write_memory()`
- **Symbolic**: `find_symbol()`, `replace_symbol_body()`, `rename_symbol()`
- **Search**: `search_for_pattern()`, `get_symbols_overview()`

### Sequential (complexité >complex)
- Analyse architecturale multi-composants
- Root cause analysis debugging
- Design système avec Tauri IPC

### Context7 (frameworks/libs externes)
- Doc officielle: Svelte, SvelteKit, Tauri, SurrealDB
- Best practices: Rust async, Tauri commands, Svelte stores
- Patterns: State management, IPC, queries DB

### Task Agents (exploration)
- Discovery codebase (subagent_type=Explore)
- Analysis patterns existants

---

## Checklist Final

Avant de marquer terminé:

- [ ] USER_PROMPT implémenté à 100%
- [ ] Frontend: Lint ✅ + TypeCheck ✅
- [ ] Backend: Clippy ✅ + Tests ✅ + Build ✅
- [ ] Types stricts synchronisés (TS ↔ Rust)
- [ ] Documentation complète (JSDoc + Rustdoc)
- [ ] Pas de any/mock/emoji/TODO
- [ ] TodoWrite: tous items completed
- [ ] Git commit avec message descriptif
- [ ] Rapport sauvegardé (`docs/taches/`)
- [ ] Serena memory (si >medium)
- [ ] Workspace propre

---

## Validation Commands

### Frontend
```bash
npm run lint          # ESLint
npm run check         # svelte-check + TypeScript
npm run test          # Vitest (si tests)
npm run build         # Production build
```

### Backend
```bash
cargo fmt --check     # Format verification
cargo clippy -- -D warnings  # Linting strict
cargo test            # Unit tests
cargo build --release # Release build
```

### E2E
```bash
npx playwright test   # Si tests E2E configurés
```

---

**RAPPEL**: Implémentation complète avec validation ZÉRO erreur. Pas de compromis sur la qualité.
