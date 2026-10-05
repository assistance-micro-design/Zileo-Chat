// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//! Tests for the tool execution loop.
use super::*;
use crate::agents::core::agent::Task;
use crate::llm::pricing::ModelPricingRow;
use crate::models::function_calling::ToolChoiceMode;
use crate::models::AgentConfig;

#[test]
fn opening_tool_choice_only_applies_to_first_iteration() {
    // Required is honoured on the opening turn so the model must emit a
    // tool call (Kanban analyze / compose root cause: model writes prose
    // and finishes without ever submitting). `true` = model accepts a
    // forced tool_choice (the common case).
    assert_eq!(
        tool_choice_for_iteration(1, ToolChoiceMode::Required, true),
        ToolChoiceMode::Required
    );
    // Subsequent turns fall back to Auto so the loop can terminate once
    // the model has submitted — a blanket Required would never let any
    // turn be tool-free, spinning until max_iterations.
    assert_eq!(
        tool_choice_for_iteration(2, ToolChoiceMode::Required, true),
        ToolChoiceMode::Auto
    );
    assert_eq!(
        tool_choice_for_iteration(50, ToolChoiceMode::Required, true),
        ToolChoiceMode::Auto
    );
}

#[test]
fn opening_tool_choice_auto_stays_auto_every_iteration() {
    // The standard workflow path passes Auto and must never be forced.
    for iteration in [1usize, 2, 10] {
        assert_eq!(
            tool_choice_for_iteration(iteration, ToolChoiceMode::Auto, true),
            ToolChoiceMode::Auto
        );
    }
}

#[test]
fn forced_opening_downgrades_to_auto_when_model_rejects_it() {
    // deepseek-v4 via RouterLab returns HTTP 400 on a forced tool_choice:
    // its `supports_forced_tool_choice` is false, so an opening Required
    // is downgraded to Auto to keep the call valid.
    assert_eq!(
        tool_choice_for_iteration(1, ToolChoiceMode::Required, false),
        ToolChoiceMode::Auto
    );
    // Later turns are Auto regardless (no change from the supported path).
    assert_eq!(
        tool_choice_for_iteration(2, ToolChoiceMode::Required, false),
        ToolChoiceMode::Auto
    );
    // Auto is unaffected by the capability flag.
    assert_eq!(
        tool_choice_for_iteration(1, ToolChoiceMode::Auto, false),
        ToolChoiceMode::Auto
    );
}

fn make_task(description: &str, context: serde_json::Value) -> Task {
    Task {
        id: "test-task".to_string(),
        description: description.to_string(),
        context,
    }
}

#[test]
fn test_build_initial_messages_first_call() {
    let task = make_task(
        "Mon nom est Bob",
        serde_json::json!({
            "is_primary_agent": true,
            "workflow_id": "wf-1",
            "locale": "fr",
        }),
    );

    let msgs = build_initial_messages(&task, "SYSTEM PROMPT".to_string());

    assert_eq!(msgs.len(), 2, "First call must produce [system, user]");
    assert_eq!(msgs[0]["role"], "system");
    assert_eq!(msgs[0]["content"], "SYSTEM PROMPT");
    assert_eq!(msgs[1]["role"], "user");
    // build_prompt wraps task.description with optional context, but our
    // context contains no `conversation_history` and only is_primary_agent/
    // workflow_id/locale -> they appear as "Context: ```json{...}```".
    // We just assert the user content contains the original description.
    let user_content = msgs[1]["content"].as_str().unwrap();
    assert!(
        user_content.contains("Mon nom est Bob"),
        "User content must contain task.description, got: {}",
        user_content
    );
}

#[test]
fn test_build_initial_messages_continuation_no_duplication() {
    let history = serde_json::json!([
        {"role": "user", "content": "Mon nom est Bob"},
        {"role": "assistant", "content": "Enchante Bob"},
        {"role": "user", "content": "Comment je m'appelle?"},
    ]);
    let task = make_task(
        "Comment je m'appelle?",
        serde_json::json!({
            "conversation_messages": history,
            "is_primary_agent": true,
            "workflow_id": "wf-1",
        }),
    );

    let msgs = build_initial_messages(&task, "REGEN SYSTEM".to_string());

    assert_eq!(
        msgs.len(),
        4,
        "Continuation must produce [system, ...history] (no extra user append)"
    );
    assert_eq!(msgs[0]["role"], "system");
    assert_eq!(msgs[0]["content"], "REGEN SYSTEM");
    assert_eq!(msgs[1]["role"], "user");
    assert_eq!(msgs[1]["content"], "Mon nom est Bob");
    assert_eq!(msgs[2]["role"], "assistant");
    assert_eq!(msgs[2]["content"], "Enchante Bob");
    assert_eq!(msgs[3]["role"], "user");
    assert_eq!(msgs[3]["content"], "Comment je m'appelle?");

    // Defense-in-depth: the last user content must appear exactly once.
    let occurrences = msgs
        .iter()
        .filter(|m| m["content"].as_str() == Some("Comment je m'appelle?"))
        .count();
    assert_eq!(
        occurrences, 1,
        "Current user message must NOT be duplicated"
    );
}

#[test]
fn test_build_initial_messages_empty_history_fallback() {
    // Empty array -> fall back to first-call behavior.
    let task = make_task(
        "Premier tour",
        serde_json::json!({
            "conversation_messages": [],
            "workflow_id": "wf-1",
        }),
    );

    let msgs = build_initial_messages(&task, "SYSTEM PROMPT".to_string());

    assert_eq!(
        msgs.len(),
        2,
        "Empty conversation_messages must trigger first-call fallback"
    );
    assert_eq!(msgs[0]["role"], "system");
    assert_eq!(msgs[1]["role"], "user");
    let user_content = msgs[1]["content"].as_str().unwrap();
    assert!(user_content.contains("Premier tour"));
}

#[test]
fn test_build_initial_messages_missing_context_key() {
    // No conversation_messages key at all -> first-call.
    let task = make_task("Hello", serde_json::json!({"workflow_id": "wf-1"}));

    let msgs = build_initial_messages(&task, "SP".to_string());
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0]["content"], "SP");
}

#[test]
fn test_build_initial_messages_continuation_preserves_order() {
    let history = serde_json::json!([
        {"role": "user", "content": "1"},
        {"role": "assistant", "content": "2"},
        {"role": "user", "content": "3"},
        {"role": "assistant", "content": "4"},
        {"role": "user", "content": "5"},
    ]);
    let task = make_task("5", serde_json::json!({"conversation_messages": history}));

    let msgs = build_initial_messages(&task, "S".to_string());

    let contents: Vec<&str> = msgs[1..]
        .iter()
        .map(|m| m["content"].as_str().unwrap())
        .collect();
    assert_eq!(contents, vec!["1", "2", "3", "4", "5"]);
}

// =========================================================================
// PricingCache — live cost during streaming.
//
// Cover the three call paths driving the live-cost feature added so the
// TokenDisplay metrics bar grows progressively (`~ X$`) instead of
// jumping from 0 to the final value at the very end:
//   1. model seeded -> load() returns Some(row)
//   2. model absent  -> load() returns None
//   3. compute_iteration_local_cost honours the cached pricing
// =========================================================================

use crate::models::LLMConfig;
use crate::test_utils::{seed_llm_model, setup_test_state};

fn make_agent_config(provider: &str, model: &str) -> AgentConfig {
    AgentConfig {
        id: "test-agent".to_string(),
        name: "Test".to_string(),
        lifecycle: crate::models::Lifecycle::Permanent,
        llm: LLMConfig {
            provider: provider.to_string(),
            model: model.to_string(),
            temperature: 0.7,
            max_tokens: 1024,
            is_reasoning: false,
            context_window: None,
        },
        tools: vec![],
        mcp_servers: vec![],
        skills: vec![],
        folders: vec![],
        require_file_confirmation: false,
        system_prompt: String::new(),
        max_tool_iterations: 10,
        reasoning_effort: None,
        kind: None,
        auto_analyze_reports: false,
        mcp_tool_allowlist: Vec::new(),
    }
}

#[tokio::test]
async fn pricing_cache_loads_row_when_model_seeded() {
    let (state, _guard) = setup_test_state().await;
    seed_llm_model(&state.db, "Mistral", "mistral-medium", 2.0, 6.0).await;

    let cache =
        PricingCache::load(&state.db, &make_agent_config("Mistral", "mistral-medium")).await;

    let row = cache.pricing.expect("seeded llm_model row must be cached");
    assert!((row.input_price_per_mtok - 2.0).abs() < 1e-9);
    assert!((row.output_price_per_mtok - 6.0).abs() < 1e-9);
}

#[tokio::test]
async fn pricing_cache_load_returns_none_when_model_absent() {
    // Defensive: tool loop must keep working when the agent's model is
    // not yet registered in `llm_model` — the chunk's `cost_usd` simply
    // stays `None` and the frontend falls back to the final cost.
    let (state, _guard) = setup_test_state().await;

    let cache = PricingCache::load(&state.db, &make_agent_config("Custom", "unknown-model")).await;

    assert!(cache.pricing.is_none());
}

#[tokio::test]
async fn supports_forced_tool_choice_defaults_true_for_seeded_and_absent_models() {
    let (state, _guard) = setup_test_state().await;
    // A seeded row that never set the flag must read as true (schema
    // DEFAULT + `?? true` coalesce), preserving the historical behaviour.
    seed_llm_model(&state.db, "Mistral", "mistral-medium", 2.0, 6.0).await;
    assert!(load_supports_forced_tool_choice(&state.db, "mistral-medium", "Mistral").await);
    // An absent model card must also default to true so a missing row
    // never disables a flow's forced opening turn.
    assert!(load_supports_forced_tool_choice(&state.db, "unknown-model", "Custom").await);
}

#[tokio::test]
async fn supports_forced_tool_choice_reads_false_when_disabled() {
    let (state, _guard) = setup_test_state().await;
    let id = seed_llm_model(&state.db, "RouterLab", "deepseek-v4-pro", 1.0, 2.0).await;
    state
        .db
        .execute(&format!(
            "UPDATE llm_model:`{}` SET supports_forced_tool_choice = false",
            id
        ))
        .await
        .expect("failed to disable supports_forced_tool_choice");

    // Provider scoping: the flag is read for the (provider, api_name) pair.
    assert!(
        !load_supports_forced_tool_choice(&state.db, "deepseek-v4-pro", "RouterLab").await,
        "disabled flag must read as false"
    );
}

#[test]
fn compute_iteration_local_cost_returns_none_when_pricing_absent() {
    let cache = PricingCache { pricing: None };

    let cost = cache.compute_iteration_local_cost(1_000, 500, None, None);

    assert!(
        cost.is_none(),
        "absent pricing -> None so the wire chunk omits cost_usd"
    );
}

#[test]
fn compute_iteration_local_cost_uses_cached_pricing() {
    // 1k input * $2/MTok + 500 output * $6/MTok = 0.002 + 0.003 = $0.005
    let cache = PricingCache {
        pricing: Some(ModelPricingRow {
            model_id: "mid".to_string(),
            input_price_per_mtok: 2.0,
            output_price_per_mtok: 6.0,
            cache_read_price_per_mtok: 0.0,
            cache_write_price_per_mtok: 0.0,
        }),
    };

    let cost = cache
        .compute_iteration_local_cost(1_000, 500, None, None)
        .expect("Some pricing -> Some(cost)");

    assert!(
        (cost - 0.005).abs() < 1e-9,
        "expected $0.005 for 1k in / 500 out at $2/$6 MTok, got ${}",
        cost
    );
}

#[test]
fn compute_iteration_local_cost_propagates_cache_savings() {
    // 80% of input served from cache @ 50% of input price ->
    // 200 regular * $2/M + 800 cache-read * $1/M + 100 output * $6/M
    // = 0.0004 + 0.0008 + 0.0006 = $0.0018
    let cache = PricingCache {
        pricing: Some(ModelPricingRow {
            model_id: "mid".to_string(),
            input_price_per_mtok: 2.0,
            output_price_per_mtok: 6.0,
            cache_read_price_per_mtok: 1.0,
            cache_write_price_per_mtok: 0.0,
        }),
    };

    let cost = cache
        .compute_iteration_local_cost(1_000, 100, Some(800), None)
        .expect("Some pricing -> Some(cost)");

    assert!(
        (cost - 0.0018).abs() < 1e-9,
        "expected $0.0018 with cache savings, got ${}",
        cost
    );
}
