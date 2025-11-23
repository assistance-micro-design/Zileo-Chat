# Orchestration Intra-Workflow

> **Objectif** : Définir comment l'agent principal détermine l'exécution parallèle ou séquentielle des opérations au sein d'un workflow

---

## Principes Fondamentaux

### 1. Analyse des Dépendances

**L'agent principal évalue chaque opération** :
- Les données d'entrée nécessaires
- Les données de sortie produites
- Les relations entre opérations

**Décision** :
- **Parallèle** : Opérations indépendantes (pas de dépendances de données)
- **Séquentiel** : Opération B nécessite le résultat de A

### 2. Limitation Architecturale

**Règle stricte** : Les sous-agents NE PEUVENT PAS lancer d'autres sous-agents

**Raison** : Réutilisabilité et maintenabilité du code
- Évite récursion complexe
- Garantit contrôle centralisé
- Simplifie debugging et traçabilité

**Seul l'orchestrateur principal** peut spawner et coordonner des sous-agents

---

## Types d'Opérations Orchestrables

### Sous-Agents
Agents spécialisés délégués pour tâches complexes

**Exemples** :
- DB Agent : Requêtes et analytics database
- API Agent : Appels services externes
- Code Agent : Refactoring et analyse code

### Tools (MCP Locaux)
Outils custom exposés via MCP Server interne

**Exemples** :
- `query_surrealdb` : Requête DB directe
- `store_memory` : Persistance mémoire vectorielle
- `validate_input` : Validation données métier

### MCP Servers Externes
Services MCP distants accessibles via MCP Client

**Exemples** :
- `serena` : Analyse sémantique codebase
- `context7` : Documentation officielle libraries
- `playwright` : Automation browser
- `sequential-thinking` : Raisonnement multi-étapes

---

## Matrice de Décision

### Détection Automatique des Dépendances

```rust
// Conceptuel - Analyse de dépendances
struct Operation {
    id: String,
    inputs: Vec<DataRef>,      // Données requises
    outputs: Vec<DataRef>,     // Données produites
    operation_type: OpType,    // SubAgent | Tool | MCP
}

fn analyze_dependencies(ops: Vec<Operation>) -> ExecutionPlan {
    let mut graph = DependencyGraph::new();

    for op in ops {
        graph.add_node(op.id);
        for input in op.inputs {
            // Si input produit par autre opération → dépendance
            if let Some(producer) = find_producer(&ops, &input) {
                graph.add_edge(producer.id, op.id);
            }
        }
    }

    // Topological sort pour ordre exécution
    graph.parallel_batches() // Retourne groupes exécutables en parallèle
}
```

### Exemples de Classification

| Scénario | Type | Raison |
|----------|------|--------|
| Lire 5 fichiers code | **Parallèle** | Lectures indépendantes |
| Analyser puis refactorer | **Séquentiel** | Refactor nécessite résultats analyse |
| Query users + Query messages | **Parallèle** | Requêtes DB indépendantes |
| Fetch API data puis store DB | **Séquentiel** | Store nécessite data fetchée |
| Appel serena + context7 | **Parallèle** | MCP servers distincts, pas dépendance |
| Search code → Refactor matches | **Séquentiel** | Refactor dépend de search results |

---

## Patterns d'Orchestration

### Pattern 1 : Fan-Out / Fan-In

**Cas d'usage** : Opérations parallèles suivies d'agrégation

```
Orchestrateur
    ├─ [Parallel] Agent DB (query users)
    ├─ [Parallel] Agent API (fetch external data)
    └─ [Parallel] MCP serena (search code patterns)
    ↓
[Sequential] Agrégation résultats → Décision
```

**Exemple concret** :
```rust
// Étape 1 : Exécution parallèle
let (users, api_data, code_patterns) = join_all([
    db_agent.execute(query_users_task),
    api_agent.execute(fetch_data_task),
    mcp_client.call("serena::search_pattern", params),
]).await;

// Étape 2 : Agrégation séquentielle
let decision = orchestrator.aggregate_and_decide(users, api_data, code_patterns);
```

### Pattern 2 : Pipeline Séquentiel

**Cas d'usage** : Transformations en chaîne

```
Orchestrateur
    ↓
MCP serena (find symbols) [Sequential]
    ↓
Tool validate_refactor [Sequential]
    ↓
Agent Code (apply refactor) [Sequential]
    ↓
Tool store_memory (save changes) [Sequential]
```

**Exemple concret** :
```rust
// Chaque étape dépend de la précédente
let symbols = mcp_client.call("serena::find_symbol", query).await?;
let validation = tool_validate(symbols).await?;
let refactored = code_agent.execute(refactor_task(validation)).await?;
tool_store_memory(refactored).await?;
```

### Pattern 3 : Hybride (Optimisé)

**Cas d'usage** : Mélange parallèle + séquentiel

```
Orchestrateur
    ├─ [Parallel] MCP context7 (get docs React)
    ├─ [Parallel] MCP context7 (get docs Svelte)
    ↓
[Sequential] Agent UI (generate component avec docs)
    ↓
[Parallel] MCP playwright (test accessibility)
[Parallel] Tool validate_a11y (WCAG check)
    ↓
[Sequential] Agrégation validation → Report
```

**Exemple concret** :
```rust
// Phase 1 : Fetch docs en parallèle
let (react_docs, svelte_docs) = join_all([
    mcp_client.call("context7::get_library_docs", "react"),
    mcp_client.call("context7::get_library_docs", "svelte"),
]).await;

// Phase 2 : Génération séquentielle (nécessite docs)
let component = ui_agent.execute(
    generate_task(react_docs, svelte_docs)
).await?;

// Phase 3 : Validation parallèle (indépendantes)
let (playwright_result, a11y_result) = join_all([
    mcp_client.call("playwright::test", component),
    tool_validate_a11y(component),
]).await;

// Phase 4 : Agrégation séquentielle
let report = aggregate_validation(playwright_result, a11y_result);
```

---

## Gestion des Erreurs

### Stratégies selon Type d'Exécution

**Parallèle** :
- Échec partiel acceptable si pas critique
- Continue avec résultats disponibles
- Log erreurs pour review

```rust
// Tolérance aux échecs partiels
let results = join_all(parallel_ops).await;
let successful = results.into_iter()
    .filter_map(|r| r.ok())
    .collect();

if successful.is_empty() {
    return Err("All parallel operations failed");
}
// Continue avec succès partiels
```

**Séquentiel** :
- Échec = arrêt pipeline immédiat
- Rollback si nécessaire
- Notification utilisateur

```rust
// Échec bloquant
let step1 = operation_a().await?; // ? = fail-fast
let step2 = operation_b(step1).await?;
let step3 = operation_c(step2).await?;
```

### Retry Logic

**Opérations idempotentes** : Retry automatique avec backoff

```rust
async fn retry_operation<T>(
    op: impl Future<Output = Result<T>>,
    max_attempts: u32
) -> Result<T> {
    for attempt in 1..=max_attempts {
        match op.await {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_attempts => {
                sleep(Duration::from_millis(100 * 2u64.pow(attempt))).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}
```

**Opérations non-idempotentes** : Pas de retry automatique, validation humaine si critique

---

## Optimisations Performance

### Batch Processing

**Regroupe opérations similaires** pour réduire overhead

```rust
// Au lieu de : N appels individuels
for file in files {
    mcp_client.call("serena::read_file", file).await;
}

// Préférer : 1 appel batch
mcp_client.call("serena::read_files_batch", files).await;
```

### Caching Intelligent

**Évite recalculs** pour opérations déterministes

```rust
// Cache LRU pour résultats MCP
let cache_key = format!("context7::docs::{}", library);
if let Some(cached) = cache.get(&cache_key) {
    return cached;
}
let result = mcp_client.call("context7::get_library_docs", library).await?;
cache.insert(cache_key, result.clone(), Duration::from_secs(3600));
```

### Timeouts Adaptatifs

**Ajuste timeouts** selon type opération et historique

```rust
struct OperationStats {
    avg_duration: Duration,
    p95_duration: Duration,
}

fn adaptive_timeout(op_type: &str, stats: &OperationStats) -> Duration {
    // Timeout = P95 + 50% marge
    stats.p95_duration * 1.5
}
```

---

## Monitoring et Observabilité

### Métriques par Workflow

```rust
struct WorkflowMetrics {
    total_duration: Duration,
    parallel_batches: Vec<BatchMetrics>,
    sequential_steps: Vec<StepMetrics>,

    // Efficacité parallélisme
    parallelization_ratio: f32, // Ops parallèles / Total ops
    speedup_factor: f32,         // Temps théorique séq / Temps réel
}

struct BatchMetrics {
    operations: Vec<String>,
    max_duration: Duration,  // Bottleneck du batch
    avg_duration: Duration,
}
```

### Visualisation Exécution

**Gantt Chart** pour analyser bottlenecks

```
Time →
0ms     500ms    1000ms   1500ms   2000ms
|--------|--------|--------|--------|--------|
[DB Query                        ] 1800ms ← Bottleneck
[API Call 1     ]
[API Call 2       ]
[MCP serena  ]
                 [Aggregate       ] 400ms
```

---

## Validation Avant Exécution

### Checklist Orchestrateur

Avant lancer workflow, vérifier :

1. **Ressources disponibles** : Agents requis actifs, MCP servers accessibles
2. **Permissions** : Agent autorisé utiliser tools/MCP demandés
3. **Dépendances cohérentes** : Pas de cycles dans graph de dépendances
4. **Timeouts raisonnables** : Estimations basées sur historique
5. **Rollback possible** : Stratégie définie pour opérations critiques

### Dry-Run Mode

**Simulation sans exécution** pour valider plan

```rust
let execution_plan = orchestrator.plan(workflow);

// Analyse plan sans exécuter
println!("Parallel batches: {}", execution_plan.parallel_batches.len());
println!("Critical path duration: {:?}", execution_plan.critical_path_duration);
println!("Resource requirements: {:?}", execution_plan.resources);

if user_approves(execution_plan) {
    orchestrator.execute(execution_plan).await?;
}
```

---

## Exemples Cas d'Usage Réels

### Cas 1 : Code Review Workflow

**Tâches** :
1. Lire fichiers modifiés (parallèle)
2. Analyser patterns (parallèle)
3. Chercher best practices docs (parallèle)
4. Générer recommandations (séquentiel - nécessite 1-3)
5. Valider syntaxe (parallèle avec 4)

**Orchestration** :
```
Batch 1 [Parallel]:
  - MCP serena::read_file (file1.rs)
  - MCP serena::read_file (file2.rs)
  - MCP serena::search_pattern (anti-patterns)
  - MCP context7::get_docs (rust best practices)

Step 2 [Sequential]:
  - Agent Code: analyze_and_recommend(batch1_results)

Batch 3 [Parallel]:
  - Tool validate_syntax (recommendations)
  - Tool check_formatting (recommendations)

Step 4 [Sequential]:
  - Tool store_memory (final_report)
```

### Cas 2 : Data Analytics Workflow

**Tâches** :
1. Query multiple tables DB (parallèle)
2. Fetch external enrichment data API (parallèle avec 1)
3. Join datasets (séquentiel - nécessite 1+2)
4. Compute aggregations (parallèle sur partitions)
5. Generate visualizations (séquentiel - nécessite 4)

**Orchestration** :
```
Batch 1 [Parallel]:
  - Agent DB: query_users
  - Agent DB: query_events
  - Agent API: fetch_demographics
  - Agent API: fetch_market_data

Step 2 [Sequential]:
  - Tool join_datasets(batch1_results)

Batch 3 [Parallel]:
  - Tool compute_agg (partition 1)
  - Tool compute_agg (partition 2)
  - Tool compute_agg (partition 3)

Step 4 [Sequential]:
  - Agent Analytics: generate_charts(batch3_results)
  - Tool store_memory (analytics_report)
```

---

## Best Practices

### DO ✅

- **Analyser dépendances** avant exécution
- **Maximiser parallélisme** quand pas de dépendances
- **Batch similaires opérations** pour overhead réduit
- **Timeout adaptatifs** selon historique performance
- **Cache résultats** déterministes
- **Log détaillé** pour debugging et optimisation
- **Fail-fast** sur erreurs critiques en séquentiel

### DON'T ❌

- **Sous-agents lancent sous-agents** : Violation règle architecture
- **Paralléliser avec dépendances** : Résultats incorrects
- **Ignorer erreurs parallèles** : Valider résultats partiels acceptables
- **Timeout uniformes** : Ajuster selon type opération
- **Surcharge parallélisme** : Limite selon ressources disponibles (CPU, mémoire)
- **Nesting excessif** : Max 3 niveaux orchestration

---

## Références

**Architecture** : [MULTI_AGENT_ARCHITECTURE.md](MULTI_AGENT_ARCHITECTURE.md)
**Tools Agents** : [AGENT_TOOLS_DOCUMENTATION.md](AGENT_TOOLS_DOCUMENTATION.md)
**MCP Integration** : [MCP_ARCHITECTURE_DECISION.md](MCP_ARCHITECTURE_DECISION.md)

---

**Version** : 1.0
**Dernière mise à jour** : 2025-11-23
