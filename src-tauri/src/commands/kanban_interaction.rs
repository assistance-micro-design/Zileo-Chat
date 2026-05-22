// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Lecture des interactions meta de l'agent Kanban pour affichage historique.
//!
//! Une "interaction" est un tool_loop execute par compose_card.rs ou
//! kanban_analyzer.rs. Les writes sont realises depuis ces modules (Phase 3
//! du plan) ; cette commande expose uniquement la lecture pour le viewer.

use crate::db::DBClient;
use crate::models::kanban_card_interaction::KanbanCardInteraction;
use crate::security::validate_uuid_field;
use crate::AppState;
use serde_json::json;
use std::sync::Arc;
use tauri::State;
use tracing::{instrument, warn};

/// Champs SELECT canoniques pour reconstruire un `KanbanCardInteraction`.
///
/// Garder synchronise avec la struct (PAT_PERSISTED_FIELD_RUST_STRUCT_SYNC) :
/// si on ajoute un champ persiste sur la table, l'ajouter ici sinon il sera
/// silencieusement omis a la lecture.
const INTERACTION_FIELDS: &str = "meta::id(id) AS id, card_id, kind, kanban_agent_id, \
    provider, model_id_used, task_input, iterations, final_payload_summary, \
    final_response_text, total_tokens_input, total_tokens_output, total_cost_usd, \
    created_at";

const MAX_INTERACTIONS_PER_CARD: usize = 100;

/// Charge les interactions meta d'une carte (compose + analyze, dans l'ordre).
///
/// Retourne une liste vide si la carte n'a aucune interaction historisee
/// (cas standard pour les anciennes cartes anterieures a la feature).
///
/// Pattern PAT_RUST_015 : la commande Tauri delegue ici pour pouvoir tester
/// la logique sans instancier `tauri::State`.
pub async fn load_card_interactions_core(
    db: &Arc<DBClient>,
    card_id: &str,
) -> Result<Vec<KanbanCardInteraction>, String> {
    let validated = validate_uuid_field(card_id, "card_id")?;
    let query = format!(
        "SELECT {INTERACTION_FIELDS} FROM kanban_card_interaction \
         WHERE card_id = $card_id \
         ORDER BY created_at ASC \
         LIMIT {MAX_INTERACTIONS_PER_CARD}"
    );
    let rows = db
        .query_json_with_params(&query, vec![("card_id".to_string(), json!(validated))])
        .await
        .map_err(|e| format!("Failed to query kanban_card_interaction: {}", e))?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        match serde_json::from_value::<KanbanCardInteraction>(row.clone()) {
            Ok(interaction) => out.push(interaction),
            Err(e) => {
                warn!(
                    card_id = %validated,
                    error = %e,
                    raw = %row,
                    "Skipping malformed kanban_card_interaction row"
                );
            }
        }
    }
    Ok(out)
}

#[tauri::command]
#[instrument(name = "load_card_interactions", skip(state), fields(card_id = %card_id))]
pub async fn load_card_interactions(
    card_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<KanbanCardInteraction>, String> {
    load_card_interactions_core(&state.db, &card_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;

    /// Inserts a minimal interaction row via raw query for the load test.
    /// Lives in tests because Phase 1 ships read-only ; persist_interaction
    /// comes in Phase 3 alongside the compose/analyze refactor.
    async fn insert_dummy_interaction(
        db: &Arc<DBClient>,
        id: &str,
        card_id: &str,
        kind: &str,
    ) -> Result<(), String> {
        let query = format!(
            "CREATE kanban_card_interaction:`{id}` CONTENT {{ \
                id: '{id}', \
                card_id: '{card_id}', \
                kind: '{kind}', \
                kanban_agent_id: 'agent-1', \
                provider: 'mistral', \
                model_id_used: 'mistral-medium-2505', \
                task_input: 'demo', \
                iterations: [], \
                total_tokens_input: 0, \
                total_tokens_output: 0, \
                total_cost_usd: 0.0, \
                created_at: time::now() \
            }}"
        );
        db.execute(&query)
            .await
            .map_err(|e| format!("insert failed: {}", e))
    }

    #[tokio::test]
    async fn load_returns_empty_for_unknown_card() {
        let (state, _guard) = setup_test_state().await;
        let card_id = uuid::Uuid::new_v4().to_string();
        let out = load_card_interactions_core(&state.db, &card_id)
            .await
            .unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn load_returns_inserted_row() {
        let (state, _guard) = setup_test_state().await;
        let card_id = uuid::Uuid::new_v4().to_string();
        let interaction_id = uuid::Uuid::new_v4().to_string();
        insert_dummy_interaction(&state.db, &interaction_id, &card_id, "compose")
            .await
            .unwrap();

        let out = load_card_interactions_core(&state.db, &card_id)
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
        let it = &out[0];
        assert_eq!(it.card_id, card_id);
        assert_eq!(
            it.kind,
            crate::models::kanban_card_interaction::InteractionKind::Compose
        );
        assert_eq!(it.kanban_agent_id, "agent-1");
        assert!(it.iterations.is_empty());
    }

    #[tokio::test]
    async fn load_orders_by_created_at_ascending() {
        let (state, _guard) = setup_test_state().await;
        let card_id = uuid::Uuid::new_v4().to_string();
        let first = uuid::Uuid::new_v4().to_string();
        let second = uuid::Uuid::new_v4().to_string();
        insert_dummy_interaction(&state.db, &first, &card_id, "compose")
            .await
            .unwrap();
        // Force chronological gap so SurrealDB orders them deterministically.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        insert_dummy_interaction(&state.db, &second, &card_id, "analyze")
            .await
            .unwrap();

        let out = load_card_interactions_core(&state.db, &card_id)
            .await
            .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(
            out[0].kind,
            crate::models::kanban_card_interaction::InteractionKind::Compose
        );
        assert_eq!(
            out[1].kind,
            crate::models::kanban_card_interaction::InteractionKind::Analyze
        );
    }

    #[tokio::test]
    async fn load_rejects_invalid_uuid() {
        let (state, _guard) = setup_test_state().await;
        let err = load_card_interactions_core(&state.db, "not-a-uuid")
            .await
            .unwrap_err();
        assert!(err.contains("card_id"), "got: {}", err);
    }
}
