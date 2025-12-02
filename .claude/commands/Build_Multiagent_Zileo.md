---
description: Workflow multi-agent pour Zileo-Chat-3 avec orchestration parallele/sequentielle
allowed-tools: Task(subagent_type:*), mcp__serena__*, mcp__context7__*, mcp__sequential-thinking__*, Glob, Read, Write, Edit, MultiEdit, Bash(git:*, npm:*, cargo:*), TodoWrite
argument-hint: <description-implementation>
---

# Workflow Multi-Agent Zileo-Chat-3

**Stack**: SvelteKit 2.49.0 + Svelte 5.43.14 | Rust 1.91.1 + Tauri 2.9.4 | SurrealDB 2.3.10

## Objectif

Implementer `$ARGUMENTS` via orchestration multi-agent intelligente.
Detection automatique des dependances pour execution parallele ou sequentielle.

---

## Document d'Entree Attendu

La commande attend un document de specification (format `docs/specs/YYYY-MM-DD_spec-*.md`) contenant:

| Section | Usage pour Orchestration |
|---------|--------------------------|
| **Metadata** | Complexity -> niveau agents |
| **Context** | Perimetre -> scope agents |
| **Etat Actuel** | Patterns -> reutilisation |
| **Architecture Proposee** | Diagrammes -> dependances |
| **Composants** | Types/Backend/Frontend -> ordre execution |
| **Plan d'Implementation** | Phases -> graphe dependances |
| **Estimation** | Temps -> parallelisation |
| **Tests** | Validation -> agents validation |

---

## Configuration

```
PROJECT_ROOT: (working directory)
TASK_REPORTS: docs/taches
SRC_FRONTEND: src
SRC_BACKEND: src-tauri
TYPES_DIR: src/types (alias: $types)
```

**Conventions**:
- TypeScript imports: `$types` alias uniquement
- Tauri IPC: snake_case (Rust) <-> camelCase (TS)

---

## Agents Disponibles

| Agent | subagent_type | Role |
|-------|---------------|------|
| **Explorer** | `Explore` | Discovery codebase, patterns existants |
| **Planner** | `Plan` | Decomposition, detection dependances |
| **Builder** | `general-purpose` | Implementation code |
| **Guide** | `claude-code-guide` | Documentation Claude Code/SDK |

---

## Phase 0: Lecture et Analyse Spec

### 0.1 Charger le Document Spec

```
1. Lire le document spec reference dans $ARGUMENTS
2. Extraire sections cles:
   - Metadata.Complexity -> niveau orchestration
   - Plan d'Implementation -> phases et taches
   - Composants -> types, backend, frontend
   - Architecture Proposee -> dependances
```

### 0.2 Analyser Dependances (Sequential Thinking)

Utiliser `mcp__sequential-thinking__sequentialthinking` pour:

```
Thought 1: Identifier les phases du Plan d'Implementation
Thought 2: Pour chaque phase, lister inputs/outputs
Thought 3: Construire graphe de dependances
Thought 4: Marquer phases parallelisables vs sequentielles
Thought 5: Definir ordre d'execution optimal
```

### 0.3 Regles de Detection

```
SEQUENTIEL obligatoire si:
  - Phase N produit des types utilises par Phase N+1
  - Phase N cree une Tauri command appelee par Phase N+1
  - Phase N modifie schema DB utilise par Phase N+1
  - Phase N depend de outputs de Phase N-1

PARALLELE possible si:
  - Phases dans domaines distincts sans partage
  - Fichiers modifies differents
  - Pas de dependance de types nouveaux
  - Tests independants
```

### 0.4 Construire le Graphe

Format du graphe de dependances:

```
phases:
  - id: "types"
    name: "Types et Models"
    domain: "shared"
    outputs: ["src/types/*.ts", "src-tauri/src/models/*.rs"]
    depends_on: []
    parallel_group: 1

  - id: "backend"
    name: "Backend Commands"
    domain: "rust"
    outputs: ["src-tauri/src/commands/*.rs"]
    depends_on: ["types"]
    parallel_group: 2

  - id: "store"
    name: "Store Frontend"
    domain: "svelte"
    outputs: ["src/lib/stores/*.ts"]
    depends_on: ["types"]
    parallel_group: 2  # Meme groupe que backend = PARALLELE

  - id: "components"
    name: "Components"
    domain: "svelte"
    outputs: ["src/lib/components/**/*.svelte"]
    depends_on: ["types", "backend", "store"]
    parallel_group: 3

  - id: "integration"
    name: "Integration Page"
    domain: "svelte"
    outputs: ["src/routes/**/*.svelte"]
    depends_on: ["components"]
    parallel_group: 4
```

---

## Phase 1: TodoWrite Initial

Creer la liste des taches basee sur le graphe:

```
TodoWrite:
  todos:
    # Groupe 1 - Sequentiel premier
    - content: "Phase Types: [details from spec]"
      status: pending
      activeForm: "Implementing types"

    # Groupe 2 - Parallele
    - content: "Phase Backend: [details from spec]"
      status: pending
      activeForm: "Implementing backend"

    - content: "Phase Store: [details from spec]"
      status: pending
      activeForm: "Implementing store"

    # Groupe 3 - Apres groupe 2
    - content: "Phase Components: [details from spec]"
      status: pending
      activeForm: "Implementing components"

    # Groupe 4 - Dernier
    - content: "Phase Integration: [details from spec]"
      status: pending
      activeForm: "Integrating in pages"

    - content: "Validation complete"
      status: pending
      activeForm: "Validating all"
```

---

## Phase 2: Execution Multi-Agent

### 2.1 Groupe Sequentiel (Types - Toujours Premier)

```
Task:
  description: "Implement shared types"
  subagent_type: "general-purpose"
  prompt: |
    Implementer les types selon la spec:

    [Coller section "Composants > Types Synchronises" du doc spec]

    Fichiers a creer/modifier:
    - src/types/<feature>.ts
    - src-tauri/src/models/<feature>.rs

    Validation requise:
    - cargo check
    - npm run check

    Retourner: liste fichiers modifies et status validation
```

### 2.2 Groupe Parallele (Backend + Store)

Lancer EN PARALLELE (un seul message, plusieurs Task):

```
Task #1:
  description: "Implement Rust backend"
  subagent_type: "general-purpose"
  prompt: |
    Implementer les commands Rust selon la spec:

    [Coller section "Composants > Backend Commands" du doc spec]

    Fichiers:
    - src-tauri/src/commands/<feature>.rs
    - src-tauri/src/main.rs (registration)

    Validation: cargo clippy + cargo test

Task #2:
  description: "Implement Svelte store"
  subagent_type: "general-purpose"
  prompt: |
    Implementer le store Svelte selon la spec:

    [Coller section "Composants > Store" du doc spec]

    Fichiers:
    - src/lib/stores/<feature>.ts

    Validation: npm run check
```

### 2.3 Groupe Sequentiel (Components - Apres Backend+Store)

```
Task:
  description: "Implement Svelte components"
  subagent_type: "general-purpose"
  prompt: |
    Implementer les composants selon la spec:

    [Coller sections "Composants > Frontend Components" du doc spec]

    Fichiers a creer:
    - src/lib/components/<path>/<Component>.svelte

    Pattern a suivre:
    - Svelte 5 runes ($state, $props)
    - Types stricts depuis $types
    - invoke() pour Tauri IPC

    Validation: npm run lint + npm run check
```

### 2.4 Groupe Final (Integration)

```
Task:
  description: "Integrate in pages"
  subagent_type: "general-purpose"
  prompt: |
    Integrer les composants dans les pages:

    [Coller section "Integration" du doc spec]

    Fichiers a modifier:
    - src/routes/<path>/+page.svelte

    Validation: npm run check
```

---

## Phase 3: Validation Multi-Agent (Parallele)

Lancer les deux validations EN PARALLELE:

```
Task #1:
  description: "Validate frontend"
  subagent_type: "general-purpose"
  model: "haiku"  # Rapide pour validation
  prompt: |
    Executer validation frontend:
    1. npm run lint
    2. npm run check
    3. npm run test (si tests existent)

    Reporter: PASS/FAIL avec details erreurs

Task #2:
  description: "Validate backend"
  subagent_type: "general-purpose"
  model: "haiku"
  prompt: |
    Executer validation backend:
    1. cargo fmt --check
    2. cargo clippy -- -D warnings
    3. cargo test

    Reporter: PASS/FAIL avec details erreurs
```

### 3.1 Gestion Echecs

```
SI validation FAIL:
  1. Analyser erreurs retournees
  2. Lancer Agent correctif (sequentiel):

  Task:
    description: "Fix validation errors"
    subagent_type: "general-purpose"
    prompt: |
      Corriger les erreurs suivantes:
      [Erreurs du rapport validation]

      Appliquer corrections et re-valider.

  3. Re-executer Phase 3 validation
```

---

## Phase 4: Rapport et Commit

### 4.1 Mettre a jour TodoWrite

Marquer toutes les taches comme completed.

### 4.2 Generer Rapport

Creer `docs/taches/YYYY-MM-DD_<feature>.md`:

```markdown
# Rapport - [Titre depuis spec]

## Metadata
- **Date**: YYYY-MM-DD HH:MM
- **Spec source**: [chemin doc spec]
- **Complexity**: [depuis spec]

## Orchestration Multi-Agent

### Graphe Execution
```
Groupe 1 (SEQ): Types
      |
      v
Groupe 2 (PAR): Backend + Store
      |
      v
Groupe 3 (SEQ): Components
      |
      v
Groupe 4 (SEQ): Integration
      |
      v
Validation (PAR): Frontend + Backend
```

### Agents Utilises
| Phase | Agent | Execution |
|-------|-------|-----------|
| Types | Builder | Sequentiel |
| Backend | Builder | Parallele |
| Store | Builder | Parallele |
| Components | Builder | Sequentiel |
| Integration | Builder | Sequentiel |
| Validation FE | Builder | Parallele |
| Validation BE | Builder | Parallele |

## Fichiers Modifies

### Types (src/types/, src-tauri/src/models/)
- [liste fichiers]

### Backend (src-tauri/src/commands/)
- [liste fichiers]

### Frontend (src/lib/)
- [liste fichiers]

## Validation

### Frontend
- Lint: PASS/FAIL
- TypeCheck: PASS/FAIL
- Tests: X/Y PASS

### Backend
- Clippy: PASS/FAIL
- Tests: X/Y PASS
- Build: PASS/FAIL

## Metriques
- Agents paralleles: X
- Agents sequentiels: Y
- Temps total: Xmin
```

### 4.3 Git Commit

```bash
git add <fichiers>
git commit -m "$(cat <<'EOF'
<type>(<scope>): <description>

Multi-agent implementation:
- Types: [fichiers]
- Backend: [fichiers]
- Frontend: [fichiers]

Spec: docs/specs/YYYY-MM-DD_spec-<feature>.md

Generated with Claude Code (Multi-Agent)
Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

### 4.4 Serena Memory (si complexity > medium)

```
mcp__serena__write_memory:
  memory_file_name: "build_<feature>_session"
  content: |
    # Session Build: <feature>

    ## Spec Source
    docs/specs/YYYY-MM-DD_spec-<feature>.md

    ## Dependances Detectees
    - types -> backend, store
    - backend, store -> components
    - components -> integration

    ## Patterns Utilises
    - [patterns from spec]

    ## Fichiers Cles
    - [liste]
```

---

## Decision Tree Rapide

```
$ARGUMENTS contient spec?
    |
   OUI -> Lire spec -> Extraire Plan Implementation
    |
    v
Combien de phases dans le plan?
    |
    +-- 1-2 phases -> Agent unique, sequentiel
    |
    +-- 3+ phases -> Analyser dependances
            |
            v
        Phases avec outputs partages?
            |
            +-- OUI -> Sequentiel pour ces phases
            |
            +-- NON -> Parallele possible
                    |
                    v
                Domaines differents (FE/BE)?
                    |
                    +-- OUI -> Parallele
                    |
                    +-- NON -> Evaluer fichiers
```

---

## Exemple Concret

### Input: Spec Prompt Library

```
Spec sections:
- Phase 1: Types et Models (1h) -> outputs: types TS + Rust
- Phase 2: Backend Commands (1.5h) -> depends: types
- Phase 3: Store Frontend (0.5h) -> depends: types
- Phase 4: Components Settings (1.5h) -> depends: types, backend, store
- Phase 5: Integration Settings (0.5h) -> depends: components
- Phase 6: ChatInput Integration (1h) -> depends: store
```

### Analyse Dependances

```
Phase 1 (types) -> SEQUENTIEL premier
Phase 2 + 3 -> PARALLELE (backend et store independants)
Phase 4 -> SEQUENTIEL apres 2+3
Phase 5 + 6 -> PARALLELE (pages differentes)
```

### Execution

```
1. Task Builder: Types (seul)
   [attendre completion]

2. Task Builder: Backend  } PARALLELE
   Task Builder: Store    } (meme message)
   [attendre completion des 2]

3. Task Builder: Components (seul)
   [attendre completion]

4. Task Builder: Integration Settings  } PARALLELE
   Task Builder: Integration ChatInput } (meme message)
   [attendre completion des 2]

5. Task Validation: Frontend } PARALLELE
   Task Validation: Backend  }
```

---

## Checklist Final

- [ ] Spec lue et analysee
- [ ] Dependances detectees correctement
- [ ] Graphe execution construit
- [ ] Phases paralleles maximisees
- [ ] Phases sequentielles respectent ordre deps
- [ ] TodoWrite complete avec toutes phases
- [ ] Validation PASS (FE + BE)
- [ ] Rapport genere
- [ ] Git commit effectue
- [ ] Serena memory (si applicable)

---

## Interdictions

**Code**:
- Emojis dans code/commentaires
- Type 'any' - typage strict
- Mock data
- TODO comments core
- Code incomplet

**Process**:
- Skip validation
- Ignorer dependances
- Parallele quand sequentiel requis

---

**PRINCIPE**: Maximiser parallelisme, respecter dependances, zero compromis qualite.
