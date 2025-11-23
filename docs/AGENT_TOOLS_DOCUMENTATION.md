# Documentation des Outils Agents par Défaut

Documentation technique des outils natifs disponibles pour les agents du système multi-agents.

---

## 1. Todo Tool

**Objectif** : Gestion hiérarchique du workflow et orchestration des tâches agents

### Opérations Disponibles
- Création de tâches structurées
- Mise à jour du statut en temps réel
- Réorganisation dynamique des priorités
- Suppression de tâches obsolètes

### Structure de Tâche
```
{
  nom: string,              // Identificateur unique de la tâche
  description: string,      // Détails opérationnels
  agent_assigné: string?,   // Optionnel - Agent responsable
  priorité: 1-5,           // 1=Critique, 5=Faible
  status: enum             // pending | in_progress | completed | blocked
}
```

### Cas d'Usage
- **Orchestration multi-agents** : Coordination de workflows complexes entre plusieurs agents
- **Traçabilité** : Suivi de progression pour opérations longues (>3 étapes)
- **Gestion de dépendances** : Organisation séquentielle ou parallèle des tâches

---

## 2. Memory Tool

**Objectif** : Persistance vectorielle dans SurrealDB pour mémoire contextuelle agents

### Architecture

**Base de données** : SurrealDB avec support embeddings vectoriels ([Doc officielle](https://surrealdb.com/docs/surrealdb/models/vector))

**Indexation** : HNSW (Hierarchical Navigable Small World) pour recherche haute dimension

**Recherche** : KNN et similarité cosinus pour retrieval sémantique

### Opérations Disponibles

#### Workflow
- `activate_workflow` : Activation contexte workflow spécifique
- `activate_general` : Mode général sans scope particulier

#### Mémoire
- `add_memory` : Ajout nouvelle entrée vectorielle
- `write_memory` : Écriture/remplacement mémoire existante
- `read_memory` : Lecture par ID ou recherche sémantique
- `list_memories` : Liste toutes entrées disponibles
- `edit_memory` : Modification partielle d'une entrée
- `delete_memory` : Suppression définitive
- `replace_content` : Remplacement de contenu dans mémoire

#### Recherche
- `search_for_pattern` : Recherche pattern-matching dans mémoires
- `think_about_collected_information` : Analyse métacognitive des données collectées
- `think_about_task_adherence` : Validation conformité objectifs
- `think_about_whether_you_are_done` : Évaluation complétude tâche

### Structure de Mémoire Recommandée

```
{
  id: uuid,                    // Identifiant unique
  type: enum,                  // user_pref | context | knowledge | decision
  content: string,             // Contenu textuel
  embedding: float[],          // Vecteur dense (dim: 768-1536)
  metadata: {
    agent_source: string,      // Agent créateur
    timestamp: datetime,       // Horodatage création
    workflow_id: string?,      // Association workflow si applicable
    priority: number,          // Importance (0.0-1.0)
    tags: string[]            // Classification sémantique
  },
  relations: uuid[]           // Liens vers autres mémoires
}
```

### Cas d'Usage
- **Préférences utilisateur** : Stockage personnalisation interface, modèles préférés
- **Contexte conversationnel** : Continuité dialogue entre sessions
- **Base de connaissances** : Accumulation expertise projet-specific
- **Décisions architecturales** : Historique choix techniques et justifications

### Bonnes Pratiques
- **Dimensionnalité** : Utiliser embeddings selon provider
  - 768D : Ollama (nomic-embed-text), BERT léger
  - 1024D : Mistral (mistral-embed), Ollama (mxbai-embed-large)
  - 1536D : OpenAI (text-embedding-3-small)
  - 3072D : OpenAI (text-embedding-3-large)
- **Indexation** : Créer index HNSW pour >1000 entrées (optimisation requêtes)
- **Scope** : Séparer mémoires workflow-specific et générales pour isolation
- **Nettoyage** : Purger mémoires temporaires post-workflow avec `delete_memory`

---

## 3. Internal Report Tool

**Objectif** : Communication inter-agents via rapports Markdown persistés localement

### Opérations
- `read` : Lecture rapports existants
- `write` : Création nouveaux rapports
- `glob` : Recherche pattern-based de rapports
- `delete` : Suppression rapports obsolètes

### Localisation Tauri

**Répertoire** : `appDataDir()` résolu comme `${dataDir}/${bundleIdentifier}`
([Référence officielle](https://v2.tauri.app/plugin/file-system/))

**Sécurité** : Scope configuration requis avec glob patterns (ex: `["$APPDATA/reports/*"]`)

**Initialisation** : Création manuelle du répertoire au premier lancement application

### Structure de Rapport
```
# Titre Rapport
**Agent** : nom_agent
**Timestamp** : ISO-8601
**Type** : analysis | decision | error | status

## Contexte
[Description situation/problème]

## Données
[Informations pertinentes structurées]

## Conclusions
[Résultats, décisions, recommandations]

## Actions Requises
- [ ] Action 1
- [ ] Action 2
```

### Cas d'Usage
- **Handoff inter-agents** : Transmission contexte entre agents spécialisés
- **Audit trail** : Traçabilité décisions pour debugging/analyse
- **Coordination asynchrone** : Communication non-bloquante entre agents
- **Rapports utilisateur** : Synthèses techniques pour revue humaine

### Bonnes Pratiques
- **Nomenclature** : `{timestamp}_{agent}_{type}.md` pour organisation chronologique
- **Atomicité** : Un rapport = une unité sémantique complète
- **Cleanup** : Archivage ou suppression rapports >30 jours selon politique retention
- **Compression** : Utiliser symboles markdown pour verbosité réduite (tableaux, listes)

---

## Intégration et Orchestration

### Workflow Type
1. **Initialisation** : Agent active workflow via `activate_workflow`
2. **Planification** : Création tâches avec Todo Tool
3. **Contexte** : Chargement mémoires pertinentes via `search_for_pattern`
4. **Exécution** : Progression tâches + écriture mémoires intermédiaires
5. **Communication** : Génération rapports pour handoff si multi-agents
6. **Finalisation** : Validation `think_about_whether_you_are_done`, cleanup temporaires

### Exemple Séquence
```
activate_workflow("code_review")
→ search_for_pattern("preferences_code_style")
→ TodoWrite([
    {nom: "analyze_files", priorité: 1, status: "in_progress"},
    {nom: "generate_report", priorité: 2, status: "pending"}
  ])
→ [Exécution analyse]
→ write_memory(type: "decision", content: "patterns_found")
→ write_report("analysis_results.md")
→ think_about_whether_you_are_done()
→ delete_memory(workflow_temps)
```

---

## Références Techniques

### SurrealDB
- [Vector Database Introduction](https://surrealdb.com/docs/surrealdb/models/vector)
- [Vector Search Reference](https://surrealdb.com/docs/surrealdb/reference-guide/vector-search)
- [Embeddings Integration](https://surrealdb.com/docs/integrations/embeddings)

### Tauri
- [File System Plugin](https://v2.tauri.app/plugin/file-system/)
- [Path API Reference](https://v2.tauri.app/reference/javascript/api/namespacepath/)
- [App Data Discussion](https://github.com/tauri-apps/tauri/discussions/5557)

---

**Version** : 1.0
**Dernière mise à jour** : 2025-11-23
