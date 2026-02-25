# SA-020: Resolution hybride agent_id/agent_name pour DelegateTask et ParallelTasks

## Metadata
- **Date**: 2026-02-25
- **Branche**: `security/audit-remediation-tdd`
- **Complexite**: medium
- **Stack**: Rust 1.93 + Tauri 2 + SvelteKit + SurrealDB 2.5
- **Statut**: DONE (P1-P7)

## Contexte

### Demande
Les tools `DelegateTask` et `ParallelTasks` exigent un `agent_id` (UUID v4 brut) pour identifier l'agent cible. Le LLM doit obligatoirement appeler `list_agents` pour decouvrir ces UUIDs avant de deleguer, gaspillant 1 tool call + ~300 tokens par delegation. La resolution par nom (agent_name) est plus naturelle pour un LLM.

### Problemes identifies

| Probleme | Localisation | Impact |
|----------|-------------|--------|
| Agent IDs sont des UUIDs bruts, pas des slugs lisibles | `commands/agent.rs:291` `Uuid::new_v4()` | LLM doit manipuler des UUIDs |
| `list_agents` obligatoire avant delegation | `delegate_task.rs:508-510` schema description | Gaspillage tool call + tokens |
| Pas de lookup par nom dans le registry | `registry.rs` - uniquement `get(id)` | Resolution par nom impossible |
| Noms d'agents NON uniques | `schema.rs:278` index plain (pas UNIQUE) | Ambiguite si resolution par nom |
| ParallelTasks n'extrait pas les vrais noms d'agents | `parallel_tasks.rs:302` `"Parallel task for {}"` | UUIDs dans rapports et events |
| Exemples misleading dans tool description | `delegate_task.rs:493` `"db_agent"` | LLM croit que les IDs sont des slugs |
| Help text UI promet unicite sans l'enforcer | `en.json:465` "A unique name" | Promesse non tenue |

### Decisions prises

| Sujet | Decision |
|-------|----------|
| Approche | Hybride: accepter agent_id OU agent_name |
| Priorite resolution | ID first (fast path), puis name (slow path) |
| Case sensitivity | Case-insensitive + trim pour les noms |
| Unicite noms | Enforcer via UNIQUE index DB + validation backend + frontend |
| Performance | O(n) scan en memoire (<20 agents), pas de reverse index |
| Backward compat | agent_id continue de fonctionner, agent_name est une alternative |
| Interne | L'ID (UUID) reste la cle partout (logs, DB, events, orchestrator) |
| Si les deux fournis | agent_id prend priorite (deterministe) |

## Perimetre

**Inclus:**
- Backend: `get_by_name()` registry, `resolve_agent_ref()` utils, update DelegateTask + ParallelTasks
- DB: index UNIQUE sur agent name, validation unicite dans create/update commands
- Frontend: validation nom duplique dans AgentForm, i18n FR/EN
- Fix: vrais noms d'agents dans ParallelTasks events/rapports
- Documentation: ce fichier + REMEDIATION-STATUS

**Exclus:**
- Injection des agents dans le system prompt (amelioration future)
- Reverse index name->id dans le registry (premature pour <20 agents)
- Fuzzy matching des noms (trop risque, exact match seulement)

## Criteres de succes

- [x] `get_by_name()` fonctionne case-insensitive avec trim
- [x] `resolve_agent_ref()` resout par ID ou par nom
- [x] DelegateTask accepte `agent_name` comme alternative a `agent_id`
- [x] ParallelTasks accepte `agent_name` comme alternative a `agent_id`
- [x] ParallelTasks affiche les vrais noms d'agents dans events/rapports
- [x] Index UNIQUE sur agent name en DB
- [x] `create_agent` et `update_agent` rejettent les noms dupliques
- [x] AgentForm.svelte valide le nom duplique cote frontend
- [x] Cles i18n `agents_name_duplicate` en FR et EN
- [x] Tous les tests existants passent (972 total, 0 failed)
- [x] 22 nouveaux tests Rust (3 P1 + 5 P2 + 4 P3 + 4 P4 + 6 P5)
- [x] `cargo clippy -- -D warnings` et `npm run check` passent

---

## Implementation

### Phase 1: Schema DB + validation unicite noms

#### 1.1 Schema UNIQUE

**Fichier**: `src-tauri/src/db/schema.rs`

```sql
-- AVANT
DEFINE INDEX OVERWRITE agent_name_idx ON agent FIELDS name;
-- APRES
DEFINE INDEX OVERWRITE agent_name_idx ON agent FIELDS name UNIQUE;
```

`DEFINE INDEX OVERWRITE` est idempotent. L'index UNIQUE bloque les futurs doublons sans invalider les existants.

#### 1.2 Validation backend

**Fichier**: `src-tauri/src/commands/agent.rs`

Check unicite via query DB avant CREATE/UPDATE:

```rust
// create_agent: check si le nom existe deja
"SELECT count() AS c FROM agent WHERE string::lowercase(name) = string::lowercase($name) GROUP ALL"

// update_agent: exclure l'agent courant
"... WHERE string::lowercase(name) = string::lowercase($name) AND meta::id(id) != $id GROUP ALL"
```

**Tests TDD** (3):
| Test | Input | Expected |
|------|-------|----------|
| `test_create_agent_rejects_duplicate_name` | 2 agents meme nom | Err("already exists") |
| `test_update_agent_allows_keeping_own_name` | update sans changer nom | Ok |
| `test_update_agent_rejects_collision_with_other` | rename vers nom existant | Err |

### Phase 2: AgentRegistry.get_by_name()

**Fichier**: `src-tauri/src/agents/core/registry.rs`

Nouvelle methode publique:
```rust
pub async fn get_by_name(&self, name: &str) -> Option<(String, Arc<dyn Agent>)>
```

- Case-insensitive + trim
- Itere le HashMap en memoire (O(n), negligeable pour <20 agents)
- Retourne (agent_id, Arc<dyn Agent>) ou None

**Tests TDD** (5):
| Test | Input | Expected |
|------|-------|----------|
| `test_get_by_name_found` | "Database Agent" | Some(("uuid1", _)) |
| `test_get_by_name_case_insensitive` | "database agent" | Some |
| `test_get_by_name_trimmed` | "  Database Agent  " | Some |
| `test_get_by_name_not_found` | "Nonexistent" | None |
| `test_get_by_name_empty` | "" | None |

### Phase 3: resolve_agent_ref() (fonction partagee)

**Fichier**: `src-tauri/src/tools/utils.rs`

```rust
pub async fn resolve_agent_ref(registry: &AgentRegistry, agent_ref: &str) -> ToolResult<String>
```

Logique:
1. Trim + check empty
2. Fast path: `registry.get(trimmed)` (ID lookup)
3. Slow path: `registry.get_by_name(trimmed)` (name lookup)
4. Not found: `ToolError::NotFound`

**Tests TDD** (3):
| Test | Input | Expected |
|------|-------|----------|
| `test_resolve_agent_ref_by_id` | UUID existant | Ok(uuid) |
| `test_resolve_agent_ref_by_name` | Nom existant | Ok(uuid) |
| `test_resolve_agent_ref_not_found` | "ghost" | Err(NotFound) |

### Phase 4: DelegateTaskTool

**Fichier**: `src-tauri/src/tools/delegate_task.rs`

Modifications:
- `validate_input()`: accepter `agent_id` OU `agent_name`
- `execute()`: extraire agent_ref depuis `agent_id` ou `agent_name` (priorite agent_id)
- `delegate()`: utiliser `resolve_agent_ref()` au lieu de `registry.get()` direct
- `definition()`: ajouter `agent_name` dans input_schema, corriger exemples

**Tests TDD** (4):
| Test | Input | Expected |
|------|-------|----------|
| `test_validate_input_accepts_agent_id` | json avec agent_id | Ok |
| `test_validate_input_accepts_agent_name` | json avec agent_name | Ok |
| `test_validate_input_rejects_missing_both` | json sans agent_id ni agent_name | Err |
| `test_definition_has_agent_name_property` | definition().input_schema | Contient "agent_name" |

### Phase 5: ParallelTasksTool

**Fichier**: `src-tauri/src/tools/parallel_tasks.rs`

Modifications:
- `validate_input()`: accepter `agent_id` OU `agent_name` par task
- `execute()`: resolution via `resolve_agent_ref()` pendant le parsing
- `prepare_execution()`: resoudre vrais noms d'agents (plus de `"Parallel task for {uuid}"`)
- `process_results()`: vrais noms dans le rapport agrege
- `definition()`: ajouter `agent_name` dans items.properties

**Tests TDD** (3):
| Test | Input | Expected |
|------|-------|----------|
| `test_validate_input_accepts_agent_name_in_tasks` | tasks avec agent_name | Ok |
| `test_validate_input_accepts_agent_id_in_tasks` | tasks avec agent_id | Ok |
| `test_validate_input_rejects_task_missing_both` | task sans agent_id ni name | Err |

### Phase 6: Frontend validation

**Fichiers**: `AgentForm.svelte`, `en.json`, `fr.json`

- i18n: `agents_name_duplicate` en FR et EN
- Validation inline dans `validate()` contre liste des agents en memoire
- Exclut self en mode edit (`a.id !== agent?.id`)

### Phase 7: Documentation - DONE

- Ce fichier (`SA-020-agent-name-resolution.md`): statut DONE, criteres coches, tests reels (22)
- `REMEDIATION-STATUS.md`: section SA-020 complete, summary mis a jour
- `.claude/learning/patterns.yml`: PAT_AGENT_002 (hybrid resolution)
- `.claude/learning/errors.yml`: ERR_DELEGATE_001 (agent ref not found)
- `.claude/learning/changelog.yml`: entree SA-020
- `.claude/registry/inventory.yml`: nouvelles fonctions utilitaires

---

## Resume des tests

| Phase | Fichier | Tests | Type |
|-------|---------|-------|------|
| 1 | commands/agent.rs | 3 | TDD unit |
| 2 | agents/core/registry.rs | 5 | TDD unit |
| 3 | tools/utils.rs | 4 | TDD unit |
| 4 | tools/delegate_task.rs | 4 | TDD unit |
| 5 | tools/parallel_tasks.rs | 6 | TDD unit |
| **Total** | | **22** | |

## Error/Pattern codes

| Code | Type | Description |
|------|------|-------------|
| ERR_DELEGATE_001 | error | Agent reference not found (neither UUID nor name) |
| PAT_AGENT_002 | pattern | Hybrid resolution: accept UUID or name, resolve via registry scan |

## Fichiers modifies (reel)

| Fichier | Changements |
|---------|-------------|
| `src-tauri/src/db/schema.rs` | +1/-1 (UNIQUE index) |
| `src-tauri/src/agents/core/registry.rs` | +84 (impl + 5 tests) |
| `src-tauri/src/tools/utils.rs` | +169 (impl + 4 tests) |
| `src-tauri/src/tools/delegate_task.rs` | +182/-42 (impl + 4 tests) |
| `src-tauri/src/tools/parallel_tasks.rs` | +287/-91 (impl + 6 tests) |
| `src-tauri/src/commands/agent.rs` | +135 (impl + 3 tests) |
| `src-tauri/src/test_utils.rs` | +/-294 (seeders rewritten for ERR_SURREAL_007) |
| `src/lib/components/settings/agents/AgentForm.svelte` | +11/-1 |
| `src/messages/en.json` | +1 |
| `src/messages/fr.json` | +1 |
| `docs/security-audits/SA-020-agent-name-resolution.md` | +234 |
| `docs/security-audits/REMEDIATION-STATUS.md` | +93 |
| **Total** | **12 fichiers, +1225/-268** |
