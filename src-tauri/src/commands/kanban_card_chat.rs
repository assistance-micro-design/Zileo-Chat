// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Confined per-card review chat with the Kanban agent.
//!
//! When the user clicks "Voir" on a card in review, the report viewer opens an
//! in-place chat backed by a dedicated hidden workflow (`hidden_from_list`) so
//! the conversation never surfaces in the `/agent` sidebar. The chat is owned
//! by the card's Kanban-kind agent, which is confined to the card: it can
//! refine prompts/skills, rerun the worker, move the card or attach a
//! recurrence, but never spawn/delegate (see PAT_KANBAN_STRICT_SEPARATION,
//! enforced in `agents::execution::tools::create_local_tools`).
//!
//! `open_card_review_chat` is idempotent: the first call seeds a single
//! assistant message summarising the worker report + last verdict and links
//! the chat workflow to the card; later calls resume the same workflow and
//! return its message history.

use crate::commands::kanban_analyzer::load_workflow_report;
use crate::commands::kanban_card::get_kanban_card_core;
use crate::commands::message::{load_workflow_messages_core, save_message_core, SaveMessageParams};
use crate::commands::settings_kanban::{load_kanban_settings, resolve_role_agent_id};
use crate::commands::workflow::create_workflow_core;
use crate::db::DBClient;
use crate::models::message::Message;
use crate::security::validate_uuid_field;
use crate::tools::utils::safe_truncate;
use crate::AppState;
use serde::Serialize;
use serde_json::json;
use tauri::State;
use tracing::{info, instrument};

/// Hard cap on the worker report excerpt injected into the seed message.
/// The full report stays available via the worker workflow; the seed only
/// needs enough context to open the conversation without blowing the budget.
const SEED_REPORT_MAX_CHARS: usize = 4000;

/// Init payload returned by [`open_card_review_chat`]: the (resumed or freshly
/// created) chat workflow id plus the messages to render.
#[derive(Debug, Clone, Serialize)]
pub struct CardReviewChatInit {
    pub workflow_id: String,
    pub messages: Vec<Message>,
}

/// Inputs to the pure seed-building function. Extracted so the formatting can
/// be unit-tested without touching the DB.
#[derive(Debug, Default)]
pub(crate) struct CardChatSeedInput {
    pub title: String,
    /// Worker report excerpt (already truncated), or `None` if the card never
    /// ran a worker workflow.
    pub report: Option<String>,
    /// Last analyze verdict summary (e.g. `"verdict: Reject"`).
    pub verdict_summary: Option<String>,
    /// Analyze rationale text.
    pub reasoning: Option<String>,
    /// Suggested prompt edit captured by the last analyze, if any.
    pub suggested_edit: Option<String>,
}

/// Builds the structured seed message shown as the first assistant turn and
/// replayed as history on subsequent turns. Pure and locale-aware (fr/en).
pub(crate) fn build_card_chat_seed(input: &CardChatSeedInput, locale: &str) -> String {
    let fr = locale.trim().to_lowercase().starts_with("fr");
    let mut s = String::new();

    if fr {
        s.push_str(&format!(
            "## Revue de la carte : {}\n\n",
            input.title.trim()
        ));
    } else {
        s.push_str(&format!("## Card review: {}\n\n", input.title.trim()));
    }

    match &input.verdict_summary {
        Some(v) if !v.trim().is_empty() => {
            let label = if fr {
                "Dernier verdict"
            } else {
                "Last verdict"
            };
            s.push_str(&format!("**{}** : {}\n\n", label, v.trim()));
        }
        _ => {}
    }

    if let Some(reasoning) = input.reasoning.as_ref().filter(|r| !r.trim().is_empty()) {
        let label = if fr { "Justification" } else { "Rationale" };
        s.push_str(&format!("**{}** :\n{}\n\n", label, reasoning.trim()));
    }

    if let Some(edit) = input
        .suggested_edit
        .as_ref()
        .filter(|e| !e.trim().is_empty())
    {
        let label = if fr {
            "Édition de prompt suggérée"
        } else {
            "Suggested prompt edit"
        };
        s.push_str(&format!("**{}** :\n{}\n\n", label, edit.trim()));
    }

    match &input.report {
        Some(report) if !report.trim().is_empty() => {
            let label = if fr {
                "Rapport du worker (extrait)"
            } else {
                "Worker report (excerpt)"
            };
            s.push_str(&format!("### {}\n\n{}\n\n", label, report.trim()));
        }
        _ => {
            let msg = if fr {
                "_Aucun rapport de worker disponible pour cette carte._\n\n"
            } else {
                "_No worker report available for this card._\n\n"
            };
            s.push_str(msg);
        }
    }

    if fr {
        s.push_str(
            "Je peux affiner le prompt ou le skill, relancer le worker, déplacer la carte \
             (la valider ou la renvoyer dans la file pour relancer un run) ou attacher une \
             récurrence. Que souhaitez-vous faire ?",
        );
    } else {
        s.push_str(
            "I can refine the prompt or skill, rerun the worker, move the card (validate it or \
             send it back to the queue for a fresh run) or attach a recurrence. What would you \
             like to do?",
        );
    }
    s
}

/// Data extracted from the latest `analyze` interaction for the seed.
struct AnalyzeSeed {
    verdict_summary: Option<String>,
    reasoning: Option<String>,
    suggested_edit: Option<String>,
}

/// Loads the most recent `analyze` interaction for the card and extracts the
/// verdict summary, rationale and any suggested prompt edit (dug out of the
/// SubmitAnalysis tool call). Best-effort: returns `None` on absence/error so
/// the seed degrades gracefully to "report only".
async fn load_latest_analyze_seed(db: &DBClient, card_id: &str) -> Option<AnalyzeSeed> {
    let q = "SELECT final_payload_summary, final_response_text, iterations, created_at \
             FROM kanban_card_interaction \
             WHERE card_id = $cid AND kind = 'analyze' \
             ORDER BY created_at DESC LIMIT 1";
    let rows = db
        .query_json_with_params(q, vec![("cid".to_string(), json!(card_id))])
        .await
        .ok()?;
    let row = rows.into_iter().next()?;

    let verdict_summary = row
        .get("final_payload_summary")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(String::from);
    let reasoning = row
        .get("final_response_text")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(String::from);

    // Dig the suggested_prompt_edit out of the SubmitAnalysis tool call input.
    let suggested_edit = row
        .get("iterations")
        .and_then(|v| v.as_array())
        .and_then(|iters| {
            iters.iter().rev().find_map(|it| {
                it.get("tool_calls")
                    .and_then(|v| v.as_array())
                    .and_then(|calls| {
                        calls.iter().find_map(|call| {
                            let name = call.get("tool_name").and_then(|v| v.as_str())?;
                            if !name.contains("SubmitAnalysis") {
                                return None;
                            }
                            let input_json = call.get("input_json").and_then(|v| v.as_str())?;
                            let parsed: serde_json::Value =
                                serde_json::from_str(input_json).ok()?;
                            parsed
                                .get("suggested_prompt_edit")
                                .and_then(|v| v.as_str())
                                .filter(|s| !s.trim().is_empty())
                                .map(String::from)
                        })
                    })
            })
        });

    Some(AnalyzeSeed {
        verdict_summary,
        reasoning,
        suggested_edit,
    })
}

/// Core entry point (PAT_RUST_015): opens or resumes the per-card review chat.
pub(crate) async fn open_card_review_chat_core(
    db: &DBClient,
    card_id: &str,
    locale: &str,
) -> Result<CardReviewChatInit, String> {
    let validated_card_id = validate_uuid_field(card_id, "card_id")?;
    let card = get_kanban_card_core(db, &validated_card_id).await?;

    // Resume: a chat workflow already exists for this card.
    if let Some(chat_wf) = card.review_chat_workflow_id.clone() {
        let messages = load_workflow_messages_core(db, &chat_wf).await?;
        return Ok(CardReviewChatInit {
            workflow_id: chat_wf,
            messages,
        });
    }

    // First open: create a hidden workflow owned by the EFFECTIVE analyze agent
    // — the configured global analyze supervisor when set & valid, otherwise the
    // card's own `kanban_agent_id` (legacy / graceful fallback). This keeps the
    // review chat owned by the same agent that produced the verdict.
    let settings = load_kanban_settings(db).await;
    let configured_analyze = settings
        .as_ref()
        .ok()
        .and_then(|s| s.analyze_agent_id.clone());
    let owner_agent_id =
        resolve_role_agent_id(db, configured_analyze.as_deref(), &card.kanban_agent_id).await;

    let chat_name = format!("Card chat: {}", safe_truncate(&card.title, 80, true));
    let chat_wf = create_workflow_core(db, chat_name, owner_agent_id, true).await?;

    // Link it to the card so future opens resume the same conversation.
    let link_q = format!(
        "UPDATE kanban_card:`{}` SET review_chat_workflow_id = '{}', updated_at = time::now()",
        validated_card_id, chat_wf
    );
    db.execute(&link_q)
        .await
        .map_err(|e| format!("Failed to link review chat workflow to card: {}", e))?;

    // Build the seed from the worker report + last analyze verdict.
    let report = match card.workflow_id.as_deref() {
        Some(wf) => load_workflow_report(db, wf)
            .await
            .ok()
            .map(|r| safe_truncate(&r, SEED_REPORT_MAX_CHARS, true)),
        None => None,
    };
    let analyze = load_latest_analyze_seed(db, &validated_card_id).await;
    let seed_input = CardChatSeedInput {
        title: card.title.clone(),
        report,
        verdict_summary: analyze.as_ref().and_then(|a| a.verdict_summary.clone()),
        reasoning: analyze.as_ref().and_then(|a| a.reasoning.clone()),
        suggested_edit: analyze.as_ref().and_then(|a| a.suggested_edit.clone()),
    };
    let seed = build_card_chat_seed(&seed_input, locale);

    // Persist the seed as the opening assistant message.
    save_message_core(
        db,
        SaveMessageParams {
            workflow_id: chat_wf.clone(),
            role: "assistant".to_string(),
            content: seed,
            tokens_input: None,
            tokens_output: None,
            model: None,
            provider: None,
            duration_ms: None,
            thinking_tokens: None,
            cost_usd: None,
            cached_tokens: None,
            cache_write_tokens: None,
            model_id_used: None,
            message_id: None,
            attachments: None,
        },
    )
    .await?;

    let messages = load_workflow_messages_core(db, &chat_wf).await?;
    info!(card_id = %validated_card_id, workflow_id = %chat_wf, "Opened card review chat (seeded)");
    Ok(CardReviewChatInit {
        workflow_id: chat_wf,
        messages,
    })
}

/// Tauri command — opens or resumes the confined review chat for a card.
#[tauri::command]
#[instrument(name = "open_card_review_chat", skip(state), fields(card_id = %card_id))]
pub async fn open_card_review_chat(
    card_id: String,
    locale: String,
    state: State<'_, AppState>,
) -> Result<CardReviewChatInit, String> {
    open_card_review_chat_core(&state.db, &card_id, &locale).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_includes_report_verdict_and_suggestion_fr() {
        let input = CardChatSeedInput {
            title: "Rapport hebdo".to_string(),
            report: Some("Le rapport détaille X et Y.".to_string()),
            verdict_summary: Some("verdict: NeedsImprovement".to_string()),
            reasoning: Some("Le ton est trop technique.".to_string()),
            suggested_edit: Some("Rends le résumé accessible.".to_string()),
        };
        let seed = build_card_chat_seed(&input, "fr");
        assert!(seed.contains("Revue de la carte : Rapport hebdo"));
        assert!(seed.contains("Dernier verdict"));
        assert!(seed.contains("NeedsImprovement"));
        assert!(seed.contains("Justification"));
        assert!(seed.contains("Édition de prompt suggérée"));
        assert!(seed.contains("Rapport du worker (extrait)"));
        assert!(seed.contains("relancer le worker"));
    }

    #[test]
    fn seed_degrades_without_report_en() {
        let input = CardChatSeedInput {
            title: "Weekly".to_string(),
            ..Default::default()
        };
        let seed = build_card_chat_seed(&input, "en");
        assert!(seed.contains("Card review: Weekly"));
        assert!(seed.contains("No worker report available"));
        assert!(!seed.contains("Last verdict"));
        assert!(seed.contains("rerun the worker"));
    }

    #[tokio::test]
    async fn open_then_resume_returns_same_workflow() {
        use crate::test_utils::setup_test_state;
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        let card_id = uuid::Uuid::new_v4().to_string();
        // Seed a review card with no worker workflow (report-less path).
        let q = format!(
            "CREATE kanban_card:`{card_id}` CONTENT {{
                id: '{card_id}', title: 'Test card', description: 'desc',
                kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                target_folder_id: NONE, status: 'review', `column`: 'review',
                `column_order`: 0, workflow_id: NONE, review_chat_workflow_id: NONE,
                error_summary: NONE, created_at: time::now(), updated_at: time::now()
            }}"
        );
        state.db.execute(&q).await.unwrap();

        let first = open_card_review_chat_core(&state.db, &card_id, "en")
            .await
            .expect("first open seeds the chat");
        assert_eq!(
            first.messages.len(),
            1,
            "first open persists one seed message"
        );
        assert_eq!(
            first.messages[0].role,
            crate::models::message::MessageRole::Assistant
        );

        // The card must now carry the link.
        let card = get_kanban_card_core(&state.db, &card_id).await.unwrap();
        assert_eq!(
            card.review_chat_workflow_id.as_deref(),
            Some(first.workflow_id.as_str())
        );

        let second = open_card_review_chat_core(&state.db, &card_id, "en")
            .await
            .expect("second open resumes");
        assert_eq!(
            second.workflow_id, first.workflow_id,
            "resume returns the same workflow id"
        );
        assert_eq!(
            second.messages.len(),
            1,
            "resume returns existing history, no new seed"
        );
    }
}
