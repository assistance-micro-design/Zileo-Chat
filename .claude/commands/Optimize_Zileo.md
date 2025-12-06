---
description: Workflow de planification d'optimisations pour Zileo-Chat-3 (amelioration de l'existant uniquement)
allowed-tools: Task(subagent_type:*), mcp__serena__*, mcp__sequential-thinking__*, WebSearch, WebFetch, Glob, Grep, Read, Write, Bash(git:*)
argument-hint: <domain: agents|llm|frontend|backend|db|tools|workflows|sidebar|mcp|security|streaming|i18n|stores|types>
---

# Workflow d'Optimisation Zileo-Chat-3

**Stack**: SvelteKit 2.49.0 + Svelte 5.43.14 | Rust 1.91.1 + Tauri 2.9.4 | SurrealDB 2.3.10

## Objectif

Planifier des **optimisations et ameliorations** pour le domaine `$ARGUMENTS`.

**REGLES STRICTES**:
- **ZERO nouvelle fonctionnalite** : Ameliorer l'existant uniquement
- **ZERO regression** : Performance egale ou superieure
- **ZERO speculation** : Baser sur le code reel
- **Best practices** : Rechercher les patterns actuels (2024-2025)

---

## Domaines Valides

| Domaine | Scope | Fichiers Principaux |
|---------|-------|---------------------|
| `agents` | Systeme multi-agent, orchestration | `src-tauri/src/agents/` |
| `llm` | Providers, Rig.rs, prompts | `src-tauri/src/llm/` |
| `frontend` | Composants UI, pages | `src/lib/components/`, `src/routes/` |
| `backend` | Commandes Tauri, IPC | `src-tauri/src/commands/` |
| `db` | SurrealDB, queries, schema | `src-tauri/src/db/` |
| `tools` | Outils MCP (Memory, Todo, etc.) | `src-tauri/src/tools/` |
| `workflows` | Execution, streaming | `src-tauri/src/commands/workflow.rs` |
| `sidebar` | Navigation, menus | `src/lib/components/layout/` |
| `mcp` | Serveurs MCP, integration | `src-tauri/src/mcp/` |
| `security` | Injection, validation, CSP | Multiple |
| `streaming` | Events Tauri, temps reel | `src-tauri/src/commands/streaming.rs` |
| `i18n` | Traductions, localisation | `src/messages/`, `src/lib/i18n/` |
| `stores` | State management Svelte | `src/lib/stores/` |
| `types` | TypeScript/Rust sync | `src/types/`, `src-tauri/src/models/` |

**IMPORTANT**: Argument obligatoire. Pas de "all" ou "auto".

---

## Configuration

```
PROJECT_ROOT: (working directory)
SPEC_OUTPUT: docs/specs
DOCS_DIR: docs
SRC_FRONTEND: src
SRC_BACKEND: src-tauri
```

---

## Workflow Multi-Agent

### Phase 0: Validation

```
1. Verifier que $ARGUMENTS est un domaine valide
2. Si invalide -> Lister les domaines disponibles et STOP
3. Identifier les fichiers principaux du domaine
```

### Phase 1: Discovery Parallele (4 Agents)

Lancer **EN PARALLELE** (un seul message, plusieurs Task):

**Agent 1 - Analyse Code Existant**:
```
Task:
  description: "Analyze existing $ARGUMENTS code"
  subagent_type: "Explore"
  prompt: |
    Analyse approfondie du domaine $ARGUMENTS dans Zileo-Chat-3.

    Chercher dans les fichiers specifiques au domaine:
    - Patterns actuellement utilises
    - Architecture et structure
    - Points de complexite
    - Duplications de code
    - Fonctions longues ou complexes
    - Gestion d'erreurs
    - Performance potentielle

    Output: Rapport structure avec:
    - Fichiers analyses (path:line)
    - Patterns identifies
    - Points d'attention
    - Metriques de complexite si possible
```

**Agent 2 - Best Practices Web (Stack Principale)**:
```
Task:
  description: "Search best practices $ARGUMENTS 2025"
  subagent_type: "general-purpose"
  prompt: |
    Rechercher les meilleures pratiques 2024-2025 pour $ARGUMENTS.

    Stack: [Adapter selon domaine]
    - Frontend: Svelte 5, SvelteKit 2, Vite 5
    - Backend: Rust, Tauri 2, async/tokio
    - DB: SurrealDB 2.x
    - LLM: Rig.rs, streaming

    Utiliser WebSearch pour:
    1. "$ARGUMENTS best practices 2025"
    2. "[stack specifique] optimization patterns"
    3. "[stack specifique] performance tips"

    Output: Rapport avec:
    - Sources (URLs)
    - Patterns recommandes
    - Anti-patterns a eviter
    - Exemples de code (si pertinents)

    IMPORTANT: Ne pas inventer. Citer les sources.
```

**Agent 3 - Documentation Existante**:
```
Task:
  description: "Review existing docs for $ARGUMENTS"
  subagent_type: "Explore"
  prompt: |
    Lire la documentation existante liee a $ARGUMENTS:
    
    - docs/specs (future update in prevision)
    - CLAUDE.md (section pertinente)
    - docs/ARCHITECTURE_DECISIONS.md
    - docs/TECH_STACK.md
    - docs/API_REFERENCE.md (si backend)
    - docs/FRONTEND_SPECIFICATIONS.md (si frontend)
    - docs/AGENT_TOOLS_DOCUMENTATION.md (si tools)

    Identifier:
    - Decisions architecturales existantes
    - Contraintes documentees
    - Patterns etablis a respecter
    - Points non optimises mentionnes

    Output: Resume des contraintes et opportunites.
```

**Agent 4 - Analyse Dependencies**:
```
Task:
  description: "Analyze dependencies for $ARGUMENTS"
  subagent_type: "general-purpose"
  model: "haiku"
  prompt: |
    Analyser les dependances liees a $ARGUMENTS:

    - package.json (frontend)
    - Cargo.toml (backend)

    Verifier:
    1. Versions actuelles vs dernieres stables
    2. Deprecations connues
    3. Alternatives plus performantes
    4. Features non utilisees

    WebSearch si necessaire pour:
    - "[crate/package] changelog 2025"
    - "[crate/package] alternatives"

    Output: Tableau comparatif avec recommandations.
```

### Phase 2: Analyse et Consolidation

Utiliser `mcp__sequential-thinking__sequentialthinking`:

```
Thought 1: Synthetiser resultats des 4 agents
  - Quels patterns sont deja bien implementes?
  - Quels gaps par rapport aux best practices?
  - Quelles contraintes non negociables?

Thought 2: Categoriser les opportunites
  - PERFORMANCE: Temps, memoire, CPU
  - MAINTENABILITE: Lisibilite, modularite
  - SECURITE: Validation, injection, CSP
  - PATTERNS: Alignment avec conventions modernes
  - DEPENDENCIES: Mises a jour, simplifications

Thought 3: Evaluer impact et effort
  - Chaque opportunite: Impact (high/medium/low)
  - Chaque opportunite: Effort (high/medium/low)
  - Risque de regression

Thought 4: Prioriser
  - Quick wins: Impact high + Effort low
  - Strategic: Impact high + Effort high
  - Nice to have: Impact low + Effort low
  - Defer: Impact low + Effort high

Thought 5: Verifier non-regression
  - Chaque changement preserve le comportement?
  - Tests existants couvrent le scope?
  - Besoin de nouveaux tests?
```

### Phase 3: Redaction Plan d'Optimisation

Creer le document `docs/specs/YYYY-MM-DD_optimization-<domain>.md`:

---

## Format de Sortie

```markdown
# Plan d'Optimisation - [Domaine]

## Metadata
- **Date**: YYYY-MM-DD
- **Domaine**: $ARGUMENTS
- **Stack**: [Stack specifique au domaine]
- **Impact estime**: [Performance/Maintenabilite/Securite]

## Resume Executif

[2-3 phrases sur l'objectif et les gains attendus]

## Etat Actuel

### Analyse du Code
[Resume Agent 1]

| Fichier | Complexite | Points d'attention |
|---------|------------|-------------------|
| path/file.rs:L42 | Haute | Description |

### Patterns Identifies
- **Pattern 1**: [Description] - Fichiers: `path`
- **Pattern 2**: [Description] - Fichiers: `path`

### Metriques Actuelles
[Si mesurables: temps de build, bundle size, etc.]

## Best Practices (2024-2025)

### Sources Consultees
- [Titre](URL) - Date
- [Titre](URL) - Date

### Patterns Recommandes
1. **[Pattern]**: Description et benefice
2. **[Pattern]**: Description et benefice

### Anti-Patterns a Eviter
1. **[Anti-Pattern]**: Pourquoi et alternative

## Contraintes du Projet

[Resume Agent 3 - Decisions existantes a respecter]

- **Decision 1**: [Description] - Source: `docs/ARCHITECTURE_DECISIONS.md`
- **Decision 2**: [Description]

## Plan d'Optimisation

### Quick Wins (Impact haut, Effort faible)

#### OPT-1: [Titre]
- **Fichiers**: `path/file.rs`, `path/file.svelte`
- **Changement**: Description precise
- **Benefice**: [Performance/Maintenabilite/etc.]
- **Risque regression**: Faible/Moyen
- **Validation**: Comment verifier le succes

#### OPT-2: [Titre]
...

### Optimisations Strategiques (Impact haut, Effort eleve)

#### OPT-3: [Titre]
- **Fichiers**: Liste
- **Changement**: Description
- **Phases**:
  1. Etape 1
  2. Etape 2
- **Prerequis**: [Dependencies, refactoring prealable]
- **Risque regression**: Moyen/Eleve
- **Tests requis**: Liste

### Nice to Have (Impact faible, Effort faible)

#### OPT-4: [Titre]
...

### Differe (Impact faible, Effort eleve)

[Liste des optimisations non prioritaires avec justification]

## Dependencies

### Mises a Jour Recommandees

| Package/Crate | Actuel | Recommande | Breaking Changes |
|---------------|--------|------------|------------------|
| [nom] | X.Y.Z | A.B.C | [Oui/Non - details] |

### Nouvelles Dependencies (si justifie)

| Package/Crate | Raison | Impact bundle |
|---------------|--------|---------------|
| [nom] | [Justification] | +Xkb |

## Verification Non-Regression

### Tests Existants
- [ ] `npm run test` - Scope: [X tests couvrent le domaine]
- [ ] `cargo test` - Scope: [Y tests couvrent le domaine]

### Tests a Ajouter
- [ ] Test 1: Description
- [ ] Test 2: Description

### Benchmarks (si applicable)
```bash
# Avant optimisation
[commande]

# Apres optimisation
[commande]
```

## Estimation

| Optimisation | Effort | Impact | Priorite |
|--------------|--------|--------|----------|
| OPT-1 | 0.5h | Haut | P1 |
| OPT-2 | 1h | Haut | P1 |
| OPT-3 | 4h | Haut | P2 |

**Total estime**: Xh

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|--------|-------------|--------|------------|
| Regression fonctionnelle | Moyenne | Eleve | Tests + review |
| Performance degradee | Faible | Eleve | Benchmarks avant/apres |

## Prochaines Etapes

1. [ ] Valider ce plan avec l'utilisateur
2. [ ] Executer OPT-1 (quick win)
3. [ ] Mesurer impact
4. [ ] Continuer avec OPT-2...

## References

- Code analyse: [Liste fichiers avec paths]
- Documentation: [Liste docs consultes]
- Sources externes: [URLs]
```

---

## Checklist Final

Avant sauvegarde du plan:

- [ ] Domaine valide et fichiers identifies
- [ ] 4 agents discovery executes en parallele
- [ ] Best practices recherchees sur internet (sources citees)
- [ ] Documentation existante consultee
- [ ] Contraintes projet respectees
- [ ] Optimisations categorisees (quick wins, strategic, etc.)
- [ ] Impact et effort estimes
- [ ] Non-regression verifiee (tests existants + nouveaux)
- [ ] **ZERO nouvelle fonctionnalite**
- [ ] **ZERO code implemente** (plan uniquement)
- [ ] Spec sauvegardee dans `docs/specs/`

---

## Exemples d'Utilisation

### /Optimize_Zileo agents
```
-> Analyse src-tauri/src/agents/
-> WebSearch "rust multi-agent patterns 2025", "tauri async agents"
-> Plan: refactoring orchestrator, simplification lifecycle
```

### /Optimize_Zileo security
```
-> Analyse validation.rs, tous les inputs
-> WebSearch "tauri security best practices 2025", "rust input validation"
-> Plan: renforcement CSP, sanitization, prompt injection prevention
```

### /Optimize_Zileo frontend
```
-> Analyse src/lib/components/
-> WebSearch "svelte 5 performance 2025", "sveltekit optimization"
-> Plan: memoization, lazy loading, bundle splitting
```

### /Optimize_Zileo db
```
-> Analyse src-tauri/src/db/, queries SurrealDB
-> WebSearch "surrealdb optimization 2025", "surrealdb indexing"
-> Plan: indexes, query batching, connection pooling
```

---

## Interdictions

**Code**:
- Aucune implementation (plan uniquement)
- Aucun emoji dans le plan
- Aucune speculation sur les gains

**Process**:
- Pas de "all" ou liste de domaines
- Pas de nouvelles features deguisees en "optimisation"
- Pas d'approximation sur les sources

---

## Outils MCP

### Task (Exploration)
- 4 agents en parallele pour discovery
- Prompts auto-contenus avec output expectations

### WebSearch (Best Practices)
- Rechercher patterns actuels (2024-2025)
- Citer les sources trouvees

### Sequential Thinking (Analyse)
- Consolidation des resultats
- Priorisation impact/effort

### Serena (Code)
- `get_symbols_overview()`: Structure
- `find_symbol()`: Localiser fonctions
- `search_for_pattern()`: Patterns similaires

---

**PRINCIPE**: Ameliorer sans casser. Optimiser sans inventer.
**METHODE**: Parallel discovery, sequential analysis, prioritized plan.
**GARANTIE**: Zero regression, zero nouvelle feature.
