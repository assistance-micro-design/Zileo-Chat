---
description: Workflow multi-agent pour synchroniser la documentation avec le code reel
allowed-tools: Task(subagent_type:*), mcp__serena__*, mcp__sequential-thinking__*, Glob, Read, Write, Edit, Bash(git:*, cargo:*, npm:*), TodoWrite
argument-hint: <"auto" | "CLAUDE.md" | "API_REFERENCE.md" | "docs/...">
---

# Workflow de Synchronisation Documentation - Zileo-Chat-3

**Stack**: SvelteKit 2.49.0 + Svelte 5.43.14 | Rust 1.91.1 + Tauri 2.9.4 | SurrealDB 2.3.10

## Objectif

Mettre a jour la documentation pour refleter le **code reel** du projet.
- **Pas d'invention** : Documenter uniquement ce qui existe dans le code
- **Pas de speculation** : Pas de features "prevues" ou "a venir"
- **Precision** : Signatures, types, et comportements exacts

---

## Architecture Multi-Agent (Claude Agent SDK Pattern)

Ce workflow utilise le pattern **orchestrator-worker** du Claude Agent SDK:

```
Orchestrator (Opus/Sonnet)
    |
    +-- [PARALLEL] Discovery Phase (up to 10 concurrent)
    |       |-- doc-backend-analyzer
    |       |-- doc-frontend-analyzer
    |       |-- doc-deps-analyzer
    |       +-- (autres selon scope)
    |
    +-- [SEQUENTIAL] Gap Detection
    |       +-- doc-gap-detector
    |
    +-- [SEQUENTIAL] Documentation Updates
            +-- doc-writer
```

### Subagents Disponibles (.claude/agents/)

| Agent | Fichier | Role | Model |
|-------|---------|------|-------|
| `doc-backend-analyzer` | doc-backend-analyzer.md | Analyse commands Rust, tools, DB | sonnet |
| `doc-frontend-analyzer` | doc-frontend-analyzer.md | Analyse components Svelte, stores | sonnet |
| `doc-deps-analyzer` | doc-deps-analyzer.md | Extraction versions exactes | haiku |
| `doc-gap-detector` | doc-gap-detector.md | Detection ecarts code/doc | sonnet |
| `doc-writer` | doc-writer.md | Application des corrections | sonnet |

---

## Configuration

```
PROJECT_ROOT: (working directory)
DOCS_DIR: docs
CLAUDE_MD: CLAUDE.md
AGENTS_DIR: .claude/agents
SRC_FRONTEND: src
SRC_BACKEND: src-tauri
TYPES_DIR: src/types
```

---

## Principes Fondamentaux

- **VERITE DU CODE** : Le code source est la seule source de verite
- **ZERO INVENTION** : Ne jamais documenter ce qui n'existe pas
- **PRECISION** : Signatures exactes, pas d'approximations
- **PARALLELISME** : Maximiser les agents concurrents independants
- **ISOLATION** : Chaque subagent a son propre contexte

---

## Documents par Priorite

### Priorite Haute
| Document | Subagents Requis |
|----------|------------------|
| `CLAUDE.md` | backend + frontend + deps |
| `docs/API_REFERENCE.md` | backend |
| `docs/AGENT_TOOLS_DOCUMENTATION.md` | backend |
| `docs/DATABASE_SCHEMA.md` | backend |

### Priorite Moyenne
| Document | Subagents Requis |
|----------|------------------|
| `docs/TECH_STACK.md` | deps |
| `docs/TOOLS_REFERENCE.md` | backend |
| `docs/FRONTEND_SPECIFICATIONS.md` | frontend |

---

## Workflow Execution

### Phase 0: Scope Determination

```
Parse $ARGUMENTS:

"auto"           -> ALL high-priority docs
                    Agents: backend + frontend + deps (PARALLEL)

"CLAUDE.md"      -> CLAUDE.md only
                    Agents: backend + frontend + deps (PARALLEL)

"docs/API_*.md"  -> API Reference only
                    Agents: backend only

"docs/TECH_*.md" -> Tech Stack only
                    Agents: deps only
```

### Phase 1: Parallel Code Discovery

**IMPORTANT**: Lancer les agents en parallele dans UN SEUL message avec plusieurs Task calls.

```
# Pour scope = "auto" ou "CLAUDE.md":
# Lancer 3 agents EN PARALLELE (meme message)

Task #1:
  description: "Analyze backend code"
  subagent_type: "general-purpose"
  model: "sonnet"
  prompt: |
    Tu es doc-backend-analyzer.

    Analyse TOUTES les commandes Tauri dans src-tauri/src/commands/.

    Pour CHAQUE fichier .rs, extraire:
    1. Toutes les fonctions #[tauri::command]
    2. Signature exacte (params + return type)
    3. Fichier:ligne

    Analyser aussi src-tauri/src/tools/ pour les outils.

    FORMAT DE SORTIE:
    ```
    # Backend Analysis Report

    ## Tauri Commands (Total: X)

    ### commands/agent.rs
    | Command | Signature | Line |
    |---------|-----------|------|
    | create_agent | (config: AgentConfigCreate) -> Result<String> | 42 |

    ### commands/workflow.rs
    ...

    ## Tools (Total: Y)
    | Tool | Operations |
    |------|------------|
    | MemoryTool | add, list, get, delete, search |
    ...
    ```

    REGLE: Documenter UNIQUEMENT ce qui existe dans le code.

Task #2:
  description: "Analyze frontend code"
  subagent_type: "general-purpose"
  model: "sonnet"
  prompt: |
    Tu es doc-frontend-analyzer.

    Analyse le frontend SvelteKit:
    - src/lib/components/ui/*.svelte (composants)
    - src/lib/stores/*.ts (stores)
    - src/types/*.ts (types)

    FORMAT DE SORTIE:
    ```
    # Frontend Analysis Report

    ## UI Components (Total: X)
    | Component | Props | File |
    |-----------|-------|------|
    | Button | variant, size, disabled | Button.svelte |

    ## Stores (Total: Y)
    | Store | Exports | File |
    |-------|---------|------|
    | agentStore | agents, loadAgents, isLoading | agents.ts |

    ## Types (Total: Z)
    | Type | Fields | File |
    |------|--------|------|
    | AgentConfig | id, name, llm, tools | agent.ts |
    ```

Task #3:
  description: "Extract dependency versions"
  subagent_type: "general-purpose"
  model: "haiku"
  prompt: |
    Tu es doc-deps-analyzer.

    Lire package.json et src-tauri/Cargo.toml.
    Extraire TOUTES les versions exactes.

    FORMAT DE SORTIE:
    ```
    # Dependencies Report

    ## Frontend (package.json)
    - svelte: 5.43.14
    - @sveltejs/kit: 2.49.0
    - vite: 5.4.0
    ...

    ## Backend (Cargo.toml)
    - tauri: 2.9.4
    - surrealdb: 2.3.10
    - rig-core: 0.24.0
    ...
    ```
```

### Phase 2: Read Existing Documentation

Apres completion des agents (Phase 1), lire les documents cibles:

```
Pour chaque document dans scope:
  1. Read le fichier complet
  2. Extraire les claims verifiables:
     - Nombres (X commands, Y components)
     - Signatures de fonctions
     - Listes de features
     - Versions
```

### Phase 3: Gap Detection (Sequential Thinking)

Utiliser `mcp__sequential-thinking__sequentialthinking`:

```
Thought 1: Comparer nombres
  - Doc dit "37 Tauri commands"
  - Agent 1 dit "42 commandes"
  -> GAP: Nombre incorrect (+5)

Thought 2: Verifier signatures
  - Doc: create_agent(config: AgentConfig)
  - Code: create_agent(config: AgentConfigCreate)
  -> GAP: Type incorrect

Thought 3: Chercher elements manquants
  - Code a: Button, Badge, Toggle, Accordion
  - Doc liste: Button, Badge
  -> GAP: 2 composants non documentes

Thought 4: Chercher elements obsoletes
  - Doc mentionne: old_command
  - Code n'a pas: old_command
  -> GAP: Element obsolete a supprimer

Thought 5: Prioriser
  - HIGH: Signatures incorrectes
  - MEDIUM: Elements manquants
  - LOW: Nombres incorrects
```

### Phase 4: Apply Documentation Updates

Pour chaque GAP identifie:

```
1. Read la section concernee du document
2. Edit avec la correction exacte
3. Verifier le changement

REGLES:
- JAMAIS Write sans Read prealable
- JAMAIS inventer de contenu
- TOUJOURS copier les signatures du code
- CONSERVER le format existant
```

### Phase 5: Validation

```
1. Relire les sections modifiees
2. Verifier coherence inter-documents:
   - Memes commandes dans API_REFERENCE et CLAUDE.md
   - Memes versions dans TECH_STACK et CLAUDE.md
3. Verifier format markdown valide
```

### Phase 6: Report & Commit

```
Generer rapport:

# Doc Sync Report - YYYY-MM-DD

## Scope: $ARGUMENTS

## Changes
| Document | Added | Removed | Fixed |
|----------|-------|---------|-------|
| CLAUDE.md | 5 | 1 | 3 |

## Details
- Updated command count: 37 -> 42
- Added components: Toggle, Accordion
- Fixed signature: create_agent

## Validation
- Cross-doc coherence: PASS
- No speculation: PASS
```

Git commit:
```bash
git add CLAUDE.md docs/*.md
git commit -m "$(cat <<'EOF'
docs: sync documentation with codebase reality

Scope: [auto|specific-doc]

Updates:
- Command count corrected (37 -> 42)
- Added undocumented components
- Fixed incorrect signatures

Analysis performed by parallel subagents.
No speculation - only code-verified changes.

Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

## Decision Matrix

```
$ARGUMENTS         | Agents (PARALLEL)           | Docs to Update
-------------------|-----------------------------|-----------------
"auto"             | backend + frontend + deps   | All high-priority
"CLAUDE.md"        | backend + frontend + deps   | CLAUDE.md
"docs/API_*.md"    | backend                     | API_REFERENCE.md
"docs/TOOLS_*.md"  | backend                     | TOOLS_*.md
"docs/TECH_*.md"   | deps                        | TECH_STACK.md
"docs/FRONTEND_*"  | frontend                    | FRONTEND_*.md
"docs/DATABASE_*"  | backend                     | DATABASE_*.md
```

---

## Exemple: /Sync_Doc_Zileo CLAUDE.md

**Step 1**: 3 agents en PARALLELE (un seul message)
```
[Task backend-analyzer] Running...
[Task frontend-analyzer] Running...
[Task deps-analyzer] Running...
```

**Step 2**: Resultats agrages
```
Backend: 42 commands, 6 tools
Frontend: 14 components, 7 stores
Deps: svelte 5.43.14, tauri 2.9.4
```

**Step 3**: Gap detection vs CLAUDE.md
```
- Commands: 37 (doc) vs 42 (code) -> FIX
- Components: 10 (doc) vs 14 (code) -> ADD 4
- Signature create_agent -> FIX type
```

**Step 4**: Apply edits
```
Edit CLAUDE.md: "37 Tauri commands" -> "42 Tauri commands"
Edit CLAUDE.md: Add Toggle, Accordion, Switch, Tooltip to UI table
Edit CLAUDE.md: AgentConfig -> AgentConfigCreate
```

**Step 5**: Commit with report

---

## Interdictions Strictes

**JAMAIS**:
- Documenter features non implementees
- Approximer des signatures
- Ajouter "TODO" ou "coming soon"
- Modifier sans evidence code
- Inventer des comportements

**TOUJOURS**:
- Lire le code avant de documenter
- Copier les signatures exactes
- Verifier avec 2+ sources
- Conserver le style existant

---

## Checklist Final

- [ ] Agents paralleles lances et completes
- [ ] Documentation existante lue
- [ ] Gaps identifies via sequential thinking
- [ ] Edits appliques (Read before Edit)
- [ ] Verification croisee code <-> doc
- [ ] Coherence inter-documents
- [ ] Rapport genere
- [ ] Git commit

---

**PRINCIPE**: La documentation est le miroir du code.
**METHODE**: Parallel discovery, sequential analysis, precise edits.
**GARANTIE**: Zero invention, zero speculation.
