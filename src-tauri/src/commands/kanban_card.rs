// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Kanban card CRUD commands.
//!
//! Each `#[tauri::command]` is a thin wrapper around a `_core` function
//! that takes `&DBClient` directly, so the core can be tested without
//! instantiating `tauri::State` (pattern PAT_RUST_015).

use crate::commands::scheduler::start_next_pending_card_core;
use crate::db::DBClient;
use crate::models::{
    KanbanCard, KanbanCardCreate, KanbanCardStatus, KanbanCardUpdate, KanbanColumn,
};
use crate::security::{serialize_for_query, validate_uuid_field};
use crate::AppState;
use serde_json::json;
use tauri::State;
use tracing::{info, instrument, warn};

// `column` is a reserved keyword in SurrealQL 2.6 — backtick-quote it (and
// `column_order`, which the parser otherwise mis-tokenises after ORDER BY).
const KANBAN_CARD_FIELDS: &str = "meta::id(id) AS id, title, description, kanban_agent_id, \
    target_agent_id, prompt_id, inline_prompt, variables, target_folder_id, status, `column`, \
    `column_order`, workflow_id, review_chat_workflow_id, error_summary, created_at, updated_at";

/// Validates the description length (max 5000 chars, like the schema ASSERT).
pub(crate) fn validate_description(desc: &str) -> Result<String, String> {
    let trimmed = desc.trim();
    if trimmed.len() > 5000 {
        return Err("description exceeds 5000 characters".to_string());
    }
    Ok(trimmed.to_string())
}

/// Validates the title (1-200 chars, non-empty after trim).
pub(crate) fn validate_title(title: &str) -> Result<String, String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err("title cannot be empty".to_string());
    }
    if trimmed.len() > 200 {
        return Err("title exceeds 200 characters".to_string());
    }
    Ok(trimmed.to_string())
}

/// Validates the `variables` JSON-stringified map.
pub(crate) fn validate_variables_json(s: &str) -> Result<String, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(s).map_err(|e| format!("variables must be valid JSON: {}", e))?;
    if !parsed.is_object() {
        return Err("variables must be a JSON object".to_string());
    }
    // Cap size to prevent DoS via huge variable blobs.
    if s.len() > 32 * 1024 {
        return Err("variables exceed 32 KB".to_string());
    }
    Ok(s.to_string())
}

/// Validates `prompt_id` XOR `inline_prompt`.
pub(crate) fn validate_xor_prompt(
    prompt_id: &Option<String>,
    inline_prompt: &Option<String>,
) -> Result<(), String> {
    match (prompt_id, inline_prompt) {
        (Some(_), Some(_)) => Err("prompt_id and inline_prompt are mutually exclusive".to_string()),
        (None, None) => Err("either prompt_id or inline_prompt is required".to_string()),
        _ => Ok(()),
    }
}

/// Persists a new kanban card.
///
/// `initial_status` is the status the row starts in — `Ready` for the normal
/// create path (the scheduler then promotes it), `Proposed` for an async compose
/// result awaiting human validation. It is a HARDCODED enum value (never
/// user-controllable), so embedding its wire string in the query is safe (M-2:
/// the async compose reuses this single validated+bound path instead of a
/// duplicated persistence helper). `column` is always `todo` (placeholder for
/// proposed cards; the scheduler only ever promotes `status='ready'`).
pub async fn create_kanban_card_core(
    db: &DBClient,
    data: KanbanCardCreate,
    initial_status: KanbanCardStatus,
) -> Result<KanbanCard, String> {
    validate_xor_prompt(&data.prompt_id, &data.inline_prompt)?;
    let title = validate_title(&data.title)?;
    let description = validate_description(&data.description)?;
    let kanban_agent_id = validate_uuid_field(&data.kanban_agent_id, "kanban_agent_id")?;
    let target_agent_id = validate_uuid_field(&data.target_agent_id, "target_agent_id")?;
    let variables = validate_variables_json(&data.variables)?;
    let prompt_id = match &data.prompt_id {
        Some(id) => Some(validate_uuid_field(id, "prompt_id")?),
        None => None,
    };
    let target_folder_id = match &data.target_folder_id {
        Some(id) => Some(validate_uuid_field(id, "target_folder_id")?),
        None => None,
    };
    let inline_prompt = data.inline_prompt;

    let card_id = match data.id.as_deref() {
        Some(id) if !id.trim().is_empty() => validate_uuid_field(id, "id")?,
        _ => uuid::Uuid::new_v4().to_string(),
    };
    let prompt_id_sql = match &prompt_id {
        Some(id) => format!("'{}'", id),
        None => "NONE".to_string(),
    };
    let inline_prompt_sql = match &inline_prompt {
        Some(p) => serialize_for_query(p, "inline_prompt")?,
        None => "NONE".to_string(),
    };
    let folder_sql = match &target_folder_id {
        Some(id) => format!("'{}'", id),
        None => "NONE".to_string(),
    };

    let status_sql = initial_status.as_str();
    let query = format!(
        "CREATE kanban_card:`{card_id}` CONTENT {{
            id: '{card_id}',
            title: $title,
            description: $description,
            kanban_agent_id: '{kanban_agent_id}',
            target_agent_id: '{target_agent_id}',
            prompt_id: {prompt_id_sql},
            inline_prompt: {inline_prompt_sql},
            variables: $variables,
            target_folder_id: {folder_sql},
            status: '{status_sql}',
            `column`: 'todo',
            `column_order`: 0,
            workflow_id: NONE,
            review_chat_workflow_id: NONE,
            error_summary: NONE,
            created_at: time::now(),
            updated_at: time::now()
        }}"
    );

    db.execute_with_params(
        &query,
        vec![
            ("title".to_string(), json!(title)),
            ("description".to_string(), json!(description)),
            ("variables".to_string(), json!(variables)),
        ],
    )
    .await
    .map_err(|e| format!("Failed to create kanban_card: {}", e))?;

    get_kanban_card_core(db, &card_id).await
}

pub async fn get_kanban_card_core(db: &DBClient, card_id: &str) -> Result<KanbanCard, String> {
    let validated_id = validate_uuid_field(card_id, "card_id")?;
    let query = format!("SELECT {KANBAN_CARD_FIELDS} FROM kanban_card:`{validated_id}`");
    let results = db
        .query_json(&query)
        .await
        .map_err(|e| format!("Failed to load kanban_card: {}", e))?;
    let row = results
        .into_iter()
        .next()
        .ok_or_else(|| "Kanban card not found".to_string())?;
    serde_json::from_value(row).map_err(|e| format!("Failed to deserialize kanban_card: {}", e))
}

pub async fn list_kanban_cards_core(
    db: &DBClient,
    kanban_agent_id: Option<String>,
) -> Result<Vec<KanbanCard>, String> {
    let where_clause = match &kanban_agent_id {
        Some(id) => {
            let validated = validate_uuid_field(id, "kanban_agent_id")?;
            format!("WHERE kanban_agent_id = '{}'", validated)
        }
        None => String::new(),
    };
    // ORDER BY column_order ASC, created_at ASC mirrors the FIFO promotion
    // query in start_next_pending_card_core, so the visual order in `todo`
    // matches the order in which cards will actually execute.
    let query = format!(
        "SELECT {KANBAN_CARD_FIELDS} FROM kanban_card {where_clause} ORDER BY `column_order` ASC, created_at ASC"
    );
    let results = db
        .query_json(&query)
        .await
        .map_err(|e| format!("Failed to list kanban_cards: {}", e))?;
    results
        .into_iter()
        .map(|v| {
            serde_json::from_value(v)
                .map_err(|e| format!("Failed to deserialize kanban_card: {}", e))
        })
        .collect()
}

pub async fn update_kanban_card_core(
    db: &DBClient,
    card_id: &str,
    update: KanbanCardUpdate,
) -> Result<KanbanCard, String> {
    let validated_id = validate_uuid_field(card_id, "card_id")?;

    let mut set_clauses: Vec<String> = vec!["updated_at = time::now()".to_string()];
    let mut params: Vec<(String, serde_json::Value)> = Vec::new();

    if let Some(title) = update.title {
        let t = validate_title(&title)?;
        set_clauses.push("title = $title".to_string());
        params.push(("title".to_string(), json!(t)));
    }
    if let Some(desc) = update.description {
        let d = validate_description(&desc)?;
        set_clauses.push("description = $description".to_string());
        params.push(("description".to_string(), json!(d)));
    }
    if let Some(target_agent) = update.target_agent_id {
        let validated_agent = validate_uuid_field(&target_agent, "target_agent_id")?;
        set_clauses.push(format!("target_agent_id = '{}'", validated_agent));
    }
    if let Some(vars) = update.variables {
        let v = validate_variables_json(&vars)?;
        set_clauses.push("variables = $variables".to_string());
        params.push(("variables".to_string(), json!(v)));
    }
    if let Some(prompt_id_opt) = update.prompt_id {
        let sql = match prompt_id_opt {
            Some(id) => {
                let validated = validate_uuid_field(&id, "prompt_id")?;
                format!("'{}'", validated)
            }
            None => "NONE".to_string(),
        };
        set_clauses.push(format!("prompt_id = {}", sql));
    }
    if let Some(inline_opt) = update.inline_prompt {
        let sql = match inline_opt {
            Some(p) => serialize_for_query(&p, "inline_prompt")?,
            None => "NONE".to_string(),
        };
        set_clauses.push(format!("inline_prompt = {}", sql));
    }
    if let Some(folder_opt) = update.target_folder_id {
        let sql = match folder_opt {
            Some(id) => {
                let validated = validate_uuid_field(&id, "target_folder_id")?;
                format!("'{}'", validated)
            }
            None => "NONE".to_string(),
        };
        set_clauses.push(format!("target_folder_id = {}", sql));
    }

    let query = format!(
        "UPDATE kanban_card:`{validated_id}` SET {}",
        set_clauses.join(", ")
    );
    db.execute_with_params(&query, params)
        .await
        .map_err(|e| format!("Failed to update kanban_card: {}", e))?;

    get_kanban_card_core(db, &validated_id).await
}

/// Cascade-deletes a card's confined review chat workflow (the hidden
/// `review_chat_workflow_id`) and everything attached to it (messages, tool
/// executions, thinking steps, sub-agent executions, …). The chat is hidden
/// from the sidebar (`hidden_from_list`), so without this it would leak in the
/// DB forever once its owning card is deleted or auto-purged (I5).
///
/// Mirrors `db::queries::cascade::delete_workflow_related` but takes `&DBClient`
/// (the kanban-card cores hold a plain client, not an `Arc`) and runs
/// sequentially. The canonical table list `CASCADE_DELETE_TABLES` is reused so
/// the two cascades never drift. A pre-SELECT confirms the workflow row exists
/// first, so a NONE / dangling link is a clean no-op rather than a blind DELETE
/// fan-out (ERR_SURREAL_013/014). The WORKER workflow (`workflow_id`) is NOT
/// touched here — it stays consultable independently of its card.
pub(crate) async fn cascade_review_chat_workflow(db: &DBClient, chat_workflow_id: &str) {
    // Malformed legacy links carry nothing safe to delete — silently skip.
    let Ok(wf) = validate_uuid_field(chat_workflow_id, "review_chat_workflow_id") else {
        return;
    };

    // Pre-SELECT: only cascade when the chat workflow row actually exists.
    match db
        .query_json(&format!("SELECT meta::id(id) AS id FROM workflow:`{}`", wf))
        .await
    {
        Ok(rows) if rows.is_empty() => return,
        Ok(_) => {}
        Err(e) => {
            warn!(workflow_id = %wf, error = %e, "Failed to look up review chat workflow for cascade");
            return;
        }
    }

    // memory_chunk first: its parent link `memory_id.workflow_id` must still
    // resolve before the parent `memory` rows are removed below.
    if let Err(e) = db
        .execute_with_params(
            "DELETE memory_chunk WHERE memory_id.workflow_id = $wf",
            vec![("wf".to_string(), json!(wf))],
        )
        .await
    {
        warn!(workflow_id = %wf, error = %e, "review chat memory_chunk cascade failed (non-fatal)");
    }

    for table in crate::db::queries::workflow::CASCADE_DELETE_TABLES {
        let q = format!("DELETE {} WHERE workflow_id = $wf", table);
        if let Err(e) = db
            .execute_with_params(&q, vec![("wf".to_string(), json!(wf))])
            .await
        {
            warn!(table = %table, workflow_id = %wf, error = %e, "review chat cascade failed (non-fatal)");
        }
    }

    if let Err(e) = db.execute(&format!("DELETE workflow:`{}`", wf)).await {
        warn!(workflow_id = %wf, error = %e, "Failed to delete review chat workflow row");
    }
}

pub async fn delete_kanban_card_core(
    db: &DBClient,
    card_id: &str,
    also_delete_schedule: bool,
) -> Result<(), String> {
    let validated_id = validate_uuid_field(card_id, "card_id")?;

    if also_delete_schedule {
        let q = format!(
            "DELETE kanban_schedule WHERE card_template_id = '{}'",
            validated_id
        );
        db.execute(&q)
            .await
            .map_err(|e| format!("Failed to delete linked schedule: {}", e))?;
    }

    // Cascade the confined review chat workflow (hidden_from_list) so it does
    // not leak (I5). The WORKER `workflow_id` is intentionally preserved — it
    // stays consultable independently of its card.
    if let Some(chat_wf) = get_kanban_card_core(db, &validated_id)
        .await
        .ok()
        .and_then(|c| c.review_chat_workflow_id)
    {
        cascade_review_chat_workflow(db, &chat_wf).await;
    }

    let query = format!("DELETE kanban_card:`{}`", validated_id);
    db.execute(&query)
        .await
        .map_err(|e| format!("Failed to delete kanban_card: {}", e))?;
    Ok(())
}

/// Allowed transitions between columns. Returns true if the move is permitted.
///
/// `send_back` (MoveCardTool / card chat) always re-queues to `Todo` — the
/// scheduler then re-promotes the card to `Doing` via the existing promotion
/// path (option b, 2026-05-29). `Doing` is therefore NEVER a manual target:
/// it is owned by the scheduler/executor.
pub fn is_transition_allowed(from: &KanbanColumn, to: &KanbanColumn) -> bool {
    use KanbanColumn::*;
    matches!(
        (from, to),
        (Todo, Todo)              // reorder
            | (Review, Done)      // user validates
            | (Review, Review)    // reorder
            | (Review, Todo)      // send back to queue (MoveCardTool / chat)
            | (Done, Done)        // reorder
            | (Done, Todo)        // re-queue a validated card (MoveCardTool / chat)
            | (Doing, Review)     // auto: workflow failed (backend trigger)
            | (Doing, Done) // auto: workflow ok (backend trigger)
    )
}

/// Resolves the id of the card whose review chat is `chat_workflow_id`.
///
/// Shared by the card-chat tools (MoveCard / ScheduleCard / RerunWorker) so
/// they self-gate identically: a clear error when called outside a card review
/// chat (no chat workflow id, or no card links back to it).
pub(crate) async fn resolve_card_id_by_review_chat(
    db: &DBClient,
    chat_workflow_id: Option<&str>,
) -> Result<String, String> {
    let wf = chat_workflow_id
        .filter(|w| !w.is_empty() && *w != "default")
        .ok_or_else(|| "this tool is only usable inside a card review chat".to_string())?;
    let q = "SELECT meta::id(id) AS id FROM kanban_card \
             WHERE review_chat_workflow_id = $wf LIMIT 1";
    let rows = db
        .query_json_with_params(q, vec![("wf".to_string(), json!(wf))])
        .await
        .map_err(|e| format!("Failed to resolve card by review chat: {}", e))?;
    rows.into_iter()
        .next()
        .and_then(|r| r["id"].as_str().map(String::from))
        .ok_or_else(|| "No card is linked to this review chat".to_string())
}

/// Persist the workflow_id link on a kanban card. Required so the
/// `workflow_complete` listener can find the card via
/// `mark_card_done_core` (which matches `WHERE workflow_id = $wid`).
pub async fn set_kanban_card_workflow_id_core(
    db: &DBClient,
    card_id: &str,
    workflow_id: &str,
) -> Result<(), String> {
    let validated_card = validate_uuid_field(card_id, "card_id")?;
    let validated_wf = validate_uuid_field(workflow_id, "workflow_id")?;
    let q = format!(
        "UPDATE kanban_card:`{validated_card}` SET workflow_id = '{validated_wf}', updated_at = time::now()"
    );
    db.execute(&q)
        .await
        .map_err(|e| format!("Failed to set workflow_id on kanban_card: {}", e))?;
    Ok(())
}

/// Clones a completed card (column `review`/`done`) into a fresh `ready`/`todo`
/// card, transfers any `kanban_schedule` rows pointing to the source onto the
/// clone, then deletes the source. Used by the "Duplicate as template" action
/// so users can attach a recurrence to a card whose original instance is no
/// longer relevant in the board.
pub async fn duplicate_kanban_card_as_template_core(
    db: &DBClient,
    source_id: &str,
) -> Result<KanbanCard, String> {
    let validated_source = validate_uuid_field(source_id, "card_id")?;
    let source = get_kanban_card_core(db, &validated_source).await?;
    if !matches!(source.column, KanbanColumn::Review | KanbanColumn::Done) {
        return Err("Only cards in review or done can be duplicated as template".to_string());
    }

    let new_id = uuid::Uuid::new_v4().to_string();
    let prompt_id_sql = match &source.prompt_id {
        Some(id) => format!("'{}'", id),
        None => "NONE".to_string(),
    };
    let inline_prompt_sql = match &source.inline_prompt {
        Some(p) => serialize_for_query(p, "inline_prompt")?,
        None => "NONE".to_string(),
    };
    let folder_sql = match &source.target_folder_id {
        Some(id) => format!("'{}'", id),
        None => "NONE".to_string(),
    };

    let query = format!(
        "CREATE kanban_card:`{new_id}` CONTENT {{
            id: '{new_id}',
            title: $title,
            description: $description,
            kanban_agent_id: '{kanban_agent_id}',
            target_agent_id: '{target_agent_id}',
            prompt_id: {prompt_id_sql},
            inline_prompt: {inline_prompt_sql},
            variables: $variables,
            target_folder_id: {folder_sql},
            status: 'ready',
            `column`: 'todo',
            `column_order`: 0,
            workflow_id: NONE,
            review_chat_workflow_id: NONE,
            error_summary: NONE,
            created_at: time::now(),
            updated_at: time::now()
        }}",
        kanban_agent_id = source.kanban_agent_id,
        target_agent_id = source.target_agent_id,
    );
    db.execute_with_params(
        &query,
        vec![
            ("title".to_string(), json!(source.title)),
            ("description".to_string(), json!(source.description)),
            ("variables".to_string(), json!(source.variables)),
        ],
    )
    .await
    .map_err(|e| format!("Failed to create duplicated kanban_card: {}", e))?;

    // Transfer schedules onto the clone BEFORE deleting the source so we never
    // observe an orphan window. delete_kanban_card_core is invoked with
    // also_delete_schedule=false because the rows have already been moved.
    let transfer_q = format!(
        "UPDATE kanban_schedule SET card_template_id = '{}' WHERE card_template_id = '{}'",
        new_id, validated_source
    );
    db.execute(&transfer_q)
        .await
        .map_err(|e| format!("Failed to transfer schedules: {}", e))?;

    delete_kanban_card_core(db, &validated_source, false).await?;

    get_kanban_card_core(db, &new_id).await
}

pub async fn move_kanban_card_core(
    db: &DBClient,
    card_id: &str,
    new_column: KanbanColumn,
    new_order: i64,
) -> Result<KanbanCard, String> {
    let card = get_kanban_card_core(db, card_id).await?;
    if !is_transition_allowed(&card.column, &new_column) {
        return Err(format!(
            "transition {:?} -> {:?} not allowed",
            card.column, new_column
        ));
    }
    let validated_id = validate_uuid_field(card_id, "card_id")?;
    let column_sql = format!("'{}'", new_column.as_str());
    // Keep `status` consistent with the destination column:
    //   * Done  -> 'done'  (validate path).
    //   * Todo  -> 'ready' (send_back re-queue): a review/done card carries
    //     status='done', which the scheduler promotion query
    //     (`WHERE status='ready' AND column='todo'`) would never match —
    //     resetting to 'ready' re-arms it for re-promotion (option b).
    // Other targets (Review/Doing reorders, auto transitions) keep status as-is.
    let status_extra = match new_column {
        KanbanColumn::Done => ", status = 'done'",
        KanbanColumn::Todo => ", status = 'ready'",
        _ => "",
    };
    let query = format!(
        "UPDATE kanban_card:`{validated_id}` SET `column` = {column_sql}, `column_order` = {new_order}, updated_at = time::now(){status_extra}"
    );
    db.execute(&query)
        .await
        .map_err(|e| format!("Failed to move kanban_card: {}", e))?;
    get_kanban_card_core(db, &validated_id).await
}

// ---------------------------------------------------------------------------
// Tauri command wrappers
// ---------------------------------------------------------------------------

#[tauri::command]
#[instrument(name = "create_kanban_card", skip(state, config))]
pub async fn create_kanban_card(
    config: KanbanCardCreate,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Creating kanban card");
    let card = create_kanban_card_core(&state.db, config, KanbanCardStatus::Ready).await?;
    // Kick the scheduler so the freshly-ready card is promoted immediately
    // instead of waiting for the next 60s tick. Failures here are best-effort:
    // the scheduler loop will still pick the card up on its next tick.
    let app_handle_opt = state.app_handle.read().ok().and_then(|g| g.clone());
    if let Some(handle) = app_handle_opt {
        if let Err(e) = start_next_pending_card_core(&state.db, &handle).await {
            warn!(error = %e, "Failed to promote freshly created card immediately");
        }
    }
    Ok(card.id)
}

#[tauri::command]
#[instrument(name = "get_kanban_card", skip(state), fields(card_id = %card_id))]
pub async fn get_kanban_card(
    card_id: String,
    state: State<'_, AppState>,
) -> Result<KanbanCard, String> {
    get_kanban_card_core(&state.db, &card_id).await
}

#[tauri::command]
#[instrument(name = "list_kanban_cards", skip(state))]
pub async fn list_kanban_cards(
    kanban_agent_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<KanbanCard>, String> {
    list_kanban_cards_core(&state.db, kanban_agent_id).await
}

#[tauri::command]
#[instrument(name = "update_kanban_card", skip(state, config), fields(card_id = %card_id))]
pub async fn update_kanban_card(
    card_id: String,
    config: KanbanCardUpdate,
    state: State<'_, AppState>,
) -> Result<KanbanCard, String> {
    update_kanban_card_core(&state.db, &card_id, config).await
}

#[tauri::command]
#[instrument(name = "delete_kanban_card", skip(state), fields(card_id = %card_id))]
pub async fn delete_kanban_card(
    card_id: String,
    also_delete_schedule: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    delete_kanban_card_core(&state.db, &card_id, also_delete_schedule.unwrap_or(false)).await
}

#[tauri::command]
#[instrument(name = "set_kanban_card_workflow_id", skip(state), fields(card_id = %card_id, workflow_id = %workflow_id))]
pub async fn set_kanban_card_workflow_id(
    card_id: String,
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_kanban_card_workflow_id_core(&state.db, &card_id, &workflow_id).await
}

#[tauri::command]
#[instrument(name = "duplicate_kanban_card_as_template", skip(state), fields(card_id = %card_id))]
pub async fn duplicate_kanban_card_as_template(
    card_id: String,
    state: State<'_, AppState>,
) -> Result<KanbanCard, String> {
    duplicate_kanban_card_as_template_core(&state.db, &card_id).await
}

#[tauri::command]
#[instrument(name = "move_kanban_card", skip(state), fields(card_id = %card_id))]
pub async fn move_kanban_card(
    card_id: String,
    new_column: KanbanColumn,
    new_order: i64,
    state: State<'_, AppState>,
) -> Result<KanbanCard, String> {
    move_kanban_card_core(&state.db, &card_id, new_column, new_order).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_xor_prompt_both() {
        let r = validate_xor_prompt(&Some("a".to_string()), &Some("b".to_string()));
        assert!(r.is_err());
    }

    #[test]
    fn test_validate_xor_prompt_neither() {
        let r = validate_xor_prompt(&None, &None);
        assert!(r.is_err());
    }

    #[test]
    fn test_validate_xor_prompt_one() {
        assert!(validate_xor_prompt(&Some("a".to_string()), &None).is_ok());
        assert!(validate_xor_prompt(&None, &Some("b".to_string())).is_ok());
    }

    #[test]
    fn test_validate_title() {
        assert!(validate_title("  hello  ").is_ok());
        assert!(validate_title("").is_err());
        assert!(validate_title(&"x".repeat(201)).is_err());
    }

    #[test]
    fn test_validate_variables_json_ok() {
        assert!(validate_variables_json("{}").is_ok());
        assert!(validate_variables_json(r#"{"name":"Alice"}"#).is_ok());
    }

    #[test]
    fn test_validate_variables_json_bad() {
        assert!(validate_variables_json("[]").is_err()); // not an object
        assert!(validate_variables_json("not json").is_err());
    }

    #[test]
    fn test_transition_allowed() {
        use KanbanColumn::*;
        // Validate.
        assert!(is_transition_allowed(&Review, &Done));
        // send_back = re-queue to Todo (option b): valid from both review and
        // done. The scheduler re-promotes via the existing tested path.
        assert!(is_transition_allowed(&Review, &Todo));
        assert!(is_transition_allowed(&Done, &Todo));
        // Reorders.
        assert!(is_transition_allowed(&Todo, &Todo));
        assert!(is_transition_allowed(&Review, &Review));
        assert!(is_transition_allowed(&Done, &Done));
        // Auto backend transitions (workflow complete).
        assert!(is_transition_allowed(&Doing, &Review));
        assert!(is_transition_allowed(&Doing, &Done));
        // Doing is never a manual send_back target under option (b): a card is
        // re-queued to Todo and the scheduler promotes it to Doing itself.
        assert!(!is_transition_allowed(&Review, &Doing));
        assert!(!is_transition_allowed(&Done, &Doing));
        assert!(!is_transition_allowed(&Todo, &Doing));
    }

    /// Option (b): send_back re-queues a card to Todo with `status='ready'` so
    /// the scheduler re-promotes it (a review/done card carries `status='done'`,
    /// which the promotion query `WHERE status='ready' AND column='todo'` would
    /// otherwise never match → dead card). Covers both Review→Todo and
    /// M-2: the async compose reuses `create_kanban_card_core` with
    /// `initial_status = Proposed`. The persisted row must carry
    /// `status='proposed'`, `column='todo'` (placeholder), and reuse the
    /// pre-generated id passed in `KanbanCardCreate.id`.
    #[tokio::test]
    async fn create_with_proposed_status_persists_proposed_todo_and_reuses_id() {
        use crate::test_utils::setup_test_state;
        let (state, _g) = setup_test_state().await;
        let card_id = uuid::Uuid::new_v4().to_string();
        let agent = uuid::Uuid::new_v4().to_string();
        let data = KanbanCardCreate {
            id: Some(card_id.clone()),
            title: "Generated task".to_string(),
            description: "d".to_string(),
            kanban_agent_id: agent.clone(),
            target_agent_id: agent.clone(),
            prompt_id: None,
            inline_prompt: Some("do it".to_string()),
            variables: "{}".to_string(),
            target_folder_id: None,
        };

        let created = create_kanban_card_core(&state.db, data, KanbanCardStatus::Proposed)
            .await
            .expect("proposed create succeeds");

        assert_eq!(created.id, card_id, "the pre-generated id must be reused");
        assert_eq!(created.status, KanbanCardStatus::Proposed);
        assert!(matches!(created.column, KanbanColumn::Todo));
    }

    /// Done→Todo.
    #[tokio::test]
    async fn test_send_back_to_todo_resets_status_to_ready() {
        use crate::test_utils::setup_test_state;
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();

        for source_column in ["review", "done"] {
            let card_id = uuid::Uuid::new_v4().to_string();
            // A review/done card always has status='done' (mark_card_done_core /
            // apply_verdict).
            let q = format!(
                "CREATE kanban_card:`{card_id}` CONTENT {{
                    id: '{card_id}', title: 't', description: '',
                    kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                    prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                    target_folder_id: NONE, status: 'done', `column`: '{source_column}',
                    `column_order`: 0, workflow_id: NONE, review_chat_workflow_id: NONE,
                    error_summary: NONE, created_at: time::now(), updated_at: time::now()
                }}"
            );
            state.db.execute(&q).await.unwrap();

            let card = move_kanban_card_core(&state.db, &card_id, KanbanColumn::Todo, 0)
                .await
                .expect("send_back to todo succeeds");
            assert!(matches!(card.column, KanbanColumn::Todo));
            assert_eq!(
                card.status,
                crate::models::kanban_card::KanbanCardStatus::Ready,
                "send_back from {source_column} must reset status to 'ready' for re-promotion"
            );
        }
    }

    /// Regression lock: Review→Doing is rejected under option (b). The card
    /// chat can only validate (→Done) or re-queue (→Todo); Doing is owned by the
    /// scheduler/executor, never a manual target.
    #[tokio::test]
    async fn test_send_back_to_doing_is_rejected() {
        use crate::test_utils::setup_test_state;
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        let card_id = uuid::Uuid::new_v4().to_string();
        let q = format!(
            "CREATE kanban_card:`{card_id}` CONTENT {{
                id: '{card_id}', title: 't', description: '',
                kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                target_folder_id: NONE, status: 'done', `column`: 'review',
                `column_order`: 0, workflow_id: NONE, review_chat_workflow_id: NONE,
                error_summary: NONE, created_at: time::now(), updated_at: time::now()
            }}"
        );
        state.db.execute(&q).await.unwrap();

        let err = move_kanban_card_core(&state.db, &card_id, KanbanColumn::Doing, 0).await;
        assert!(
            err.is_err(),
            "Review->Doing must be rejected under option (b)"
        );
    }

    // duplicate_kanban_card_as_template_core: clones a review/done card into
    // a fresh ready/todo card, transfers schedules pointing to the source
    // onto the clone, then deletes the source.
    #[tokio::test]
    async fn test_duplicate_as_template_transfers_schedules_and_deletes_source() {
        use crate::models::KanbanScheduleCreate;
        use crate::test_utils::setup_test_state;
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        let source_id = uuid::Uuid::new_v4().to_string();

        // Seed a 'done' card directly (skip column transition checks).
        let create_source = format!(
            "CREATE kanban_card:`{source_id}` CONTENT {{
                id: '{source_id}', title: 'src', description: 'desc',
                kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                prompt_id: NONE, inline_prompt: 'inline prompt',
                variables: '{{\"k\":\"v\"}}', target_folder_id: NONE,
                status: 'done', `column`: 'done', `column_order`: 0,
                workflow_id: NONE, error_summary: NONE,
                created_at: time::now(), updated_at: time::now()
            }}"
        );
        state.db.execute(&create_source).await.unwrap();

        // Attach a schedule to the source.
        let sched = super::super::kanban_schedule::create_kanban_schedule_core(
            &state.db,
            KanbanScheduleCreate {
                card_template_id: source_id.clone(),
                days_of_week: vec![0, 2, 4],
                hour: 9,
                minute: 30,
                skip_if_pending: false,
            },
        )
        .await
        .expect("schedule create");

        // Duplicate.
        let clone = duplicate_kanban_card_as_template_core(&state.db, &source_id)
            .await
            .expect("duplicate succeeds");

        assert_ne!(clone.id, source_id, "clone must have a fresh id");
        assert_eq!(clone.title, "src");
        assert_eq!(clone.inline_prompt.as_deref(), Some("inline prompt"));
        assert_eq!(clone.variables, "{\"k\":\"v\"}");
        assert_eq!(
            clone.status,
            crate::models::kanban_card::KanbanCardStatus::Ready
        );
        assert!(matches!(clone.column, KanbanColumn::Todo));

        // Schedule must now point to the clone.
        let reloaded =
            super::super::kanban_schedule::get_kanban_schedule_core(&state.db, &sched.id)
                .await
                .expect("schedule still exists");
        assert_eq!(reloaded.card_template_id, clone.id);

        // Source must be gone.
        let src_lookup = get_kanban_card_core(&state.db, &source_id).await;
        assert!(src_lookup.is_err(), "source card must be deleted");
    }

    #[tokio::test]
    async fn test_update_target_agent_id_persists() {
        use crate::models::KanbanCardUpdate;
        use crate::test_utils::setup_test_state;
        let (state, _g) = setup_test_state().await;
        let agent_a = uuid::Uuid::new_v4().to_string();
        let agent_b = uuid::Uuid::new_v4().to_string();
        let cid = uuid::Uuid::new_v4().to_string();
        let q = format!(
            "CREATE kanban_card:`{cid}` CONTENT {{
                id: '{cid}', title: 't', description: '',
                kanban_agent_id: '{agent_a}', target_agent_id: '{agent_a}',
                prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                target_folder_id: NONE, status: 'review', `column`: 'review',
                `column_order`: 0, workflow_id: NONE, error_summary: NONE,
                created_at: time::now(), updated_at: time::now()
            }}"
        );
        state.db.execute(&q).await.unwrap();
        let updated = update_kanban_card_core(
            &state.db,
            &cid,
            KanbanCardUpdate {
                target_agent_id: Some(agent_b.clone()),
                ..Default::default()
            },
        )
        .await
        .expect("update succeeds");
        assert_eq!(updated.target_agent_id, agent_b);
        // kanban_agent_id must remain untouched (not editable via update).
        assert_eq!(updated.kanban_agent_id, agent_a);
    }

    /// I5: deleting a card must cascade its confined review chat workflow (the
    /// hidden `review_chat_workflow_id`) plus everything attached to it. Hidden
    /// from the sidebar, it would otherwise leak in the DB forever. The WORKER
    /// workflow (`workflow_id`) stays preserved on purpose (consultable
    /// independently).
    #[tokio::test]
    async fn test_delete_card_cascades_review_chat_workflow() {
        use crate::test_utils::setup_test_state;
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        let card_id = uuid::Uuid::new_v4().to_string();
        let chat_wf = uuid::Uuid::new_v4().to_string();
        let worker_wf = uuid::Uuid::new_v4().to_string();

        // Hidden review chat workflow + a message attached to it.
        let create_wf = format!(
            "CREATE workflow:`{chat_wf}` SET id = '{chat_wf}', name = 'chat', agent_id = '{agent_id}', \
             status = 'idle', hidden_from_list = true, pinned = false, \
             created_at = time::now(), updated_at = time::now()"
        );
        state.db.execute(&create_wf).await.unwrap();
        let mid = uuid::Uuid::new_v4().to_string();
        let create_msg = format!(
            "CREATE message:`{mid}` SET id = '{mid}', workflow_id = '{chat_wf}', role = 'assistant', \
             content = 'seed', tokens = 0, created_at = time::now(), updated_at = time::now()"
        );
        state.db.execute(&create_msg).await.unwrap();

        // Worker workflow that must SURVIVE (only the chat is cascaded).
        let create_worker = format!(
            "CREATE workflow:`{worker_wf}` SET id = '{worker_wf}', name = 'worker', agent_id = '{agent_id}', \
             status = 'completed', hidden_from_list = false, pinned = false, \
             created_at = time::now(), updated_at = time::now()"
        );
        state.db.execute(&create_worker).await.unwrap();

        let create_card = format!(
            "CREATE kanban_card:`{card_id}` CONTENT {{
                id: '{card_id}', title: 't', description: '',
                kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                target_folder_id: NONE, status: 'done', `column`: 'done',
                `column_order`: 0, workflow_id: '{worker_wf}',
                review_chat_workflow_id: '{chat_wf}', error_summary: NONE,
                created_at: time::now(), updated_at: time::now()
            }}"
        );
        state.db.execute(&create_card).await.unwrap();

        delete_kanban_card_core(&state.db, &card_id, false)
            .await
            .expect("delete succeeds");

        // Chat workflow + its messages must be gone.
        let wf_rows = state
            .db
            .query_json(&format!(
                "SELECT meta::id(id) AS id FROM workflow:`{}`",
                chat_wf
            ))
            .await
            .unwrap();
        assert!(wf_rows.is_empty(), "hidden chat workflow must be cascaded");
        let msg_rows = state
            .db
            .query_json(&format!(
                "SELECT meta::id(id) AS id FROM message WHERE workflow_id = '{}'",
                chat_wf
            ))
            .await
            .unwrap();
        assert!(msg_rows.is_empty(), "chat messages must be cascaded");

        // Worker workflow must SURVIVE.
        let worker_rows = state
            .db
            .query_json(&format!(
                "SELECT meta::id(id) AS id FROM workflow:`{}`",
                worker_wf
            ))
            .await
            .unwrap();
        assert_eq!(worker_rows.len(), 1, "worker workflow must be preserved");
    }

    #[tokio::test]
    async fn test_duplicate_as_template_rejects_todo_card() {
        use crate::test_utils::setup_test_state;
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        let cid = uuid::Uuid::new_v4().to_string();
        let q = format!(
            "CREATE kanban_card:`{cid}` CONTENT {{
                id: '{cid}', title: 't', description: '',
                kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                target_folder_id: NONE, status: 'ready', `column`: 'todo',
                `column_order`: 0, workflow_id: NONE, error_summary: NONE,
                created_at: time::now(), updated_at: time::now()
            }}"
        );
        state.db.execute(&q).await.unwrap();
        let r = duplicate_kanban_card_as_template_core(&state.db, &cid).await;
        assert!(r.is_err(), "must reject non-review/done sources");
    }

    // Regression: SurrealQL 2.6 treats `column` as a reserved keyword. Without
    // backtick-quoting `column` and `column_order` the parser mis-tokenises
    // `column_order` after ORDER BY and the query fails with "Missing order
    // idiom column_order". This test exercises the full SELECT / WHERE /
    // ORDER BY chain to lock in the quoting.
    #[tokio::test]
    async fn test_list_kanban_cards_orders_by_column_order() {
        use crate::test_utils::setup_test_state;
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        // Two cards with reversed column_order to confirm ORDER BY is honoured.
        for (idx, order) in [("a", 10i64), ("b", 1i64)] {
            let cid = uuid::Uuid::new_v4().to_string();
            let q = format!(
                "CREATE kanban_card:`{cid}` CONTENT {{
                    id: '{cid}', title: '{idx}', description: '',
                    kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                    prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                    target_folder_id: NONE, status: 'todo', `column`: 'todo',
                    `column_order`: {order}, workflow_id: NONE, error_summary: NONE,
                    created_at: time::now(), updated_at: time::now()
                }}"
            );
            state.db.execute(&q).await.unwrap();
        }
        let cards = list_kanban_cards_core(&state.db, Some(agent_id.clone()))
            .await
            .expect("ORDER BY `column_order` must parse and run");
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].title, "b"); // column_order = 1 comes first
        assert_eq!(cards[1].title, "a"); // column_order = 10 second
    }
}
