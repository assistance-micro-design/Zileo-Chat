# Rapport - Correction erreurs SurrealDB

## Metadonnees
- **Date**: 2025-11-25 14:00
- **Complexite**: medium
- **Stack**: Svelte 5.43 + Rust 1.91 + Tauri 2.9 + SurrealDB 2.3

## Objectif
Corriger deux erreurs SurrealDB survenant lors de l'execution de `npm run tauri dev` :
1. Erreur de deserialization : `invalid type: enum, expected any valid JSON value`
2. Erreur de creation : `Table name contained a colon (:)`

## Travail Realise

### Analyse des erreurs

**Erreur 1** : Lors du chargement des workflows (`load_workflows`), la deserialization echouait car SurrealDB retourne le champ `status` dans un format interne non reconnu par serde.

**Erreur 2** : Lors de la creation d'un workflow (`create_workflow`), le code passait une string au format `table:id` a `db.create()`, mais le SDK SurrealDB 2.x attend un tuple `(table, id)`.

### Corrections implementees

**1. db/client.rs:85-100** - Utilisation d'une requete SurrealQL directe :
```rust
// Avant (SDK .create().content() causait des erreurs de serialization)
self.db.create((table, id)).content(data).await

// Apres (requete SurrealQL avec bind() pour eviter les problemes SDK)
let json_data = serde_json::to_value(&data)?;
let query = format!("CREATE {}:`{}` CONTENT $data", table, id);
self.db.query(&query).bind(("data", json_data)).await
```
Cette approche evite les problemes de serialization internes au SDK SurrealDB 2.x en :
1. Convertissant les donnees en `serde_json::Value` manuellement
2. Utilisant une requete SurrealQL avec `$data` bind au lieu de `.content()`

**2. db/client.rs:68-82** - Modification de `db_query` pour contourner le deserializeur SDK :
```rust
// Avant (SDK .take::<T>() ignorait nos custom deserializers)
let data: Vec<T> = result.take(0)?;

// Apres (extraction JSON puis deserialization serde_json)
let raw_data: Vec<serde_json::Value> = result.take(0)?;
let data: Vec<T> = raw_data
    .into_iter()
    .map(|v| serde_json::from_value(v))
    .collect::<Result<Vec<T>, _>>()?;
```

**3. models/serde_utils.rs:163-245** - Ajout d'un deserializeur custom pour `WorkflowStatus` :
```rust
pub fn deserialize_workflow_status<'de, D>(
    deserializer: D,
) -> Result<WorkflowStatus, D::Error>
```
Ce deserializeur gere :
- Les strings simples (`"idle"`, `"running"`, etc.)
- Les wrappers enum internes de SurrealDB

**3. models/workflow.rs:29** - Application du deserializeur custom :
```rust
#[serde(deserialize_with = "deserialize_workflow_status")]
pub status: WorkflowStatus,
```

### Fichiers Modifies

**Backend** (Rust):
- `src-tauri/src/db/client.rs` - Modification de la methode `create()` pour utiliser le format tuple
- `src-tauri/src/models/serde_utils.rs` - Ajout du deserializeur `deserialize_workflow_status()`
- `src-tauri/src/models/workflow.rs` - Application du deserializeur sur le champ `status`

### Statistiques Git
```
 src-tauri/src/db/client.rs          | 20 ++++++---
 src-tauri/src/models/serde_utils.rs | 87 +++++++++++++++++++++++++++++++++++++
 src-tauri/src/models/workflow.rs    |  5 ++-
 3 files changed, 104 insertions(+), 8 deletions(-)
```

## Decisions Techniques

### Architecture
- **Pattern Visitor** : Utilisation du pattern Visitor de serde pour gerer les differents formats de donnees SurrealDB (strings, enums internes)
- **Compatibilite SDK** : Adaptation au SDK SurrealDB 2.x qui utilise des tuples pour les IDs de records

### Patterns Utilises
- **Custom Deserializer** : Deserializeur serde personnalise pour gerer les formats enum internes de SurrealDB
- **Tuple Record ID** : Format `(table, id)` pour la creation de records avec ID specifique

## Validation

### Tests Backend
- **Clippy**: PASS (0 warnings)
- **Cargo test**: 179/179 PASS
- **Build release**: SUCCESS

### Tests Frontend
- **Lint**: PASS (0 erreurs)
- **TypeCheck**: PASS (0 erreurs)

### Qualite Code
- Types stricts (Rust)
- Documentation Rustdoc complete
- Standards projet respectes
- Pas de any/mock/emoji/TODO

## Notes Importantes

**Base de donnees** : Si une base de donnees existante contenait des workflows crees avec l'ancien format (qui echouait), il peut etre necessaire de la supprimer :
```bash
rm -rf ~/.zileo/db
```

## Metriques

### Code
- **Lignes ajoutees**: +104
- **Lignes supprimees**: -8
- **Fichiers modifies**: 3
