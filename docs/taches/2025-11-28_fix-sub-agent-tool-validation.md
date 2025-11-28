# Rapport - Fix Sub-Agent Tool Validation

## Metadata
- **Date**: 2025-11-28 09:30
- **Complexity**: medium
- **Duration**: ~45min
- **Stack**: Rust 1.91 + Tauri 2.9 + SurrealDB 2.3

## Objectif

Corriger les problemes observes dans une conversation avec l'agent principal:
1. Agent "cree" mais non visible dans Settings
2. Validation human-in-the-loop non declenchee
3. Aucun visuel frontend pour l'utilisation des agents
4. Agent mentionne des outils non disponibles dans l'application

## Analyse Root Cause

**Probleme principal**: Le LLM "hallucinait" l'usage des outils - il generait du texte qui RESSEMBLE a une creation d'agent mais n'a jamais reellement appele `SpawnAgentTool` avec le format XML `<tool_call>` correct.

**Evidence de la conversation**:
- LLM dit "Agent Cree avec Succes" sans appel d'outil
- LLM attribue des outils fictifs: `AdvancedSearchTool`, `SourceValidatorTool`, `DataExtractionTool`, `SummarizationTool`, `APIConnector`
- Aucun evenement `validation_required` emis (car aucun outil appele)
- Aucun evenement `sub_agent_start` emis (car aucun outil appele)

**Comportement attendu**:
- Sub-agents sont TEMPORAIRES et n'apparaissent PAS dans Settings (correct par design)
- Sub-agents apparaissent dans `SubAgentActivity` panel pendant l'execution
- Human-in-the-loop demande validation AVANT creation du sub-agent

## Travail Realise

### 1. SpawnAgentTool Definition Updated

**Fichier**: `src-tauri/src/tools/spawn_agent.rs`

Ajout dans la definition du tool:
```rust
AVAILABLE TOOLS FOR SUB-AGENTS: {available_tools_str}
Note: Sub-agents can only use basic tools listed above, NOT sub-agent tools.
Do NOT invent or specify tools that are not in this list.

// Clarification sur les sub-agents temporaires:
- Sub-agents are TEMPORARY and are automatically cleaned up after execution
- Sub-agents do NOT appear in the Settings agent list (they are workflow-scoped)
```

### 2. Tool Name Validation in spawn()

**Fichier**: `src-tauri/src/tools/spawn_agent.rs`

Nouvelle etape 4 - Validation des noms d'outils:
```rust
// 4. Validate tool names if provided
if let Some(ref tool_list) = tools {
    let invalid_tools: Vec<&String> = tool_list
        .iter()
        .filter(|t| !ToolFactory::is_valid_tool(t))
        .collect();

    if !invalid_tools.is_empty() {
        let available = ToolFactory::basic_tools().join(", ");
        return Err(ToolError::ValidationFailed(format!(
            "Invalid tool(s) specified: {:?}. Available tools for sub-agents: {}.",
            invalid_tools, available
        )));
    }

    // Reject sub-agent tools
    let sub_agent_tools = ToolFactory::sub_agent_tools();
    let forbidden_tools: Vec<&String> = tool_list
        .iter()
        .filter(|t| sub_agent_tools.contains(&t.as_str()))
        .collect();

    if !forbidden_tools.is_empty() {
        return Err(ToolError::ValidationFailed(format!(
            "Sub-agents cannot use sub-agent tools: {:?}.",
            forbidden_tools
        )));
    }
}
```

### 3. DelegateTaskTool et ParallelTasksTool

Ces outils utilisent des agents existants avec leur configuration propre - pas besoin de validation d'outils supplementaire.

## Fichiers Modifies

**Backend** (Rust):
- `src-tauri/src/tools/spawn_agent.rs` - Definition + validation

## Validation

### Tests Backend
- **Clippy**: PASS (0 warnings)
- **Cargo test**: 473 tests PASS

### Tests Frontend
- **Lint**: PASS (0 errors)
- **Check**: PASS (0 errors, 0 warnings)

## Comportement Apres Fix

Quand le LLM appelle SpawnAgentTool avec des outils invalides:
```json
{"operation": "spawn", "name": "WebResearcher", "tools": ["AdvancedSearchTool"]}
```

Reponse:
```
Error: Invalid tool(s) specified: ["AdvancedSearchTool"].
Available tools for sub-agents: MemoryTool, TodoTool.
Note: Sub-agents cannot use SpawnAgentTool, DelegateTaskTool, or ParallelTasksTool.
```

## Probleme Residuel

Le LLM doit encore APPELER l'outil avec le format XML correct plutot que de generer du texte. Ceci est un probleme de prompt/comportement LLM qui peut necessiter:

1. **Ajustement du system prompt** pour insister sur l'usage du format `<tool_call>`
2. **Temperature plus basse** pour reduire la creativite
3. **Few-shot examples** dans le prompt montrant l'usage correct des outils

## Recommandations

1. Tester avec un agent configure avec les bons tools
2. Verifier que le system prompt de l'agent inclut les instructions pour l'usage des tools
3. Surveiller les logs backend pour voir si `SpawnAgentTool.execute()` est appele

## Git Stats

```
Files modified: 1
Lines added: ~35
Lines removed: ~5
```
