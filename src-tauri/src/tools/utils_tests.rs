use super::*;
use crate::agents::core::agent::{Agent, Report, ReportMetrics, ReportStatus, Task};
use crate::models::{AgentConfig, LLMConfig, Lifecycle};
use async_trait::async_trait;

#[test]
fn test_validate_not_empty_valid() {
    assert!(validate_not_empty("hello", "field").is_ok());
}

#[test]
fn test_validate_not_empty_invalid() {
    let result = validate_not_empty("", "field");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ToolError::ValidationFailed(_)
    ));
}

#[test]
fn test_validate_length_valid() {
    assert!(validate_length("hello", 10, "field").is_ok());
}

#[test]
fn test_validate_length_invalid() {
    let result = validate_length("hello world", 5, "field");
    assert!(result.is_err());
}

#[test]
fn test_validate_range_valid() {
    assert!(validate_range(5, 1, 10, "field").is_ok());
}

#[test]
fn test_validate_range_invalid() {
    let result = validate_range(15, 1, 10, "field");
    assert!(result.is_err());
}

#[test]
fn test_validate_enum_value_valid() {
    assert!(validate_enum_value("pending", &["pending", "done"], "status").is_ok());
}

#[test]
fn test_validate_enum_value_invalid() {
    let result = validate_enum_value("invalid", &["pending", "done"], "status");
    assert!(result.is_err());
}

#[test]
fn test_param_query_builder_simple() {
    let (query, params) = ParamQueryBuilder::new("memory")
        .select(&["content", "type"])
        .build();
    assert_eq!(
        query,
        "SELECT meta::id(id) AS id, content, type FROM memory"
    );
    assert!(params.is_empty());
}

#[test]
fn test_param_query_builder_with_params() {
    let (query, params) = ParamQueryBuilder::new("memory")
        .select(&["content"])
        .where_eq_param("type", "type_filter", serde_json::json!("knowledge"))
        .order_by("created_at", true)
        .limit(10)
        .build();
    assert!(query.contains("WHERE type = $type_filter"));
    assert!(query.contains("ORDER BY created_at DESC"));
    assert!(query.contains("LIMIT 10"));
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].0, "type_filter");
    assert_eq!(params[0].1, serde_json::json!("knowledge"));
}

// --- resolve_agent_ref tests ---

/// Minimal test agent for resolve_agent_ref tests
struct TestAgent {
    config: AgentConfig,
}

impl TestAgent {
    fn new(id: &str, lifecycle: Lifecycle) -> Self {
        Self {
            config: AgentConfig {
                id: id.to_string(),
                name: format!("Test Agent {}", id),
                lifecycle,
                llm: LLMConfig {
                    provider: "Test".to_string(),
                    model: "test-model".to_string(),
                    temperature: 0.7,
                    max_tokens: 100,
                    is_reasoning: false,
                    context_window: None,
                },
                tools: vec![],
                mcp_servers: vec![],
                skills: vec![],
                folders: vec![],
                require_file_confirmation: true,
                system_prompt: "Test prompt".to_string(),
                max_tool_iterations: 50,
                reasoning_effort: None,
            },
        }
    }
}

#[async_trait]
impl Agent for TestAgent {
    async fn execute(&self, _task: Task) -> anyhow::Result<Report> {
        Ok(Report {
            status: ReportStatus::Success,
            content: "Test report".to_string(),
            response: "Test report".to_string(),
            metrics: ReportMetrics {
                duration_ms: 10,
                tokens_input: 0,
                tokens_output: 0,
                context_tokens: 0,
                cached_tokens: None,
                cache_write_tokens: None,
                tools_used: vec![],
                mcp_calls: vec![],
                tool_executions: vec![],
                reasoning_steps: vec![],
                iteration_metrics: vec![],
                thinking_tokens: None,
                provider_cost_usd: None,
            },
        })
    }

    fn capabilities(&self) -> Vec<String> {
        vec!["test".to_string()]
    }

    fn lifecycle(&self) -> Lifecycle {
        self.config.lifecycle.clone()
    }

    fn tools(&self) -> Vec<String> {
        self.config.tools.clone()
    }

    fn mcp_servers(&self) -> Vec<String> {
        self.config.mcp_servers.clone()
    }

    fn config(&self) -> &AgentConfig {
        &self.config
    }
}

#[tokio::test]
async fn test_resolve_agent_ref_by_id() {
    let registry = AgentRegistry::new();
    let agent = Arc::new(TestAgent::new("agent-uuid-1", Lifecycle::Permanent));
    registry.register("agent-uuid-1".to_string(), agent).await;

    let result = resolve_agent_ref(&registry, "agent-uuid-1").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "agent-uuid-1");
}

#[tokio::test]
async fn test_resolve_agent_ref_by_name() {
    let registry = AgentRegistry::new();
    let agent = Arc::new(TestAgent::new("agent-uuid-2", Lifecycle::Permanent));
    registry.register("agent-uuid-2".to_string(), agent).await;

    let result = resolve_agent_ref(&registry, "Test Agent agent-uuid-2").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "agent-uuid-2");
}

#[tokio::test]
async fn test_resolve_agent_ref_not_found() {
    let registry = AgentRegistry::new();
    let agent = Arc::new(TestAgent::new("agent-uuid-3", Lifecycle::Permanent));
    registry.register("agent-uuid-3".to_string(), agent).await;

    let result = resolve_agent_ref(&registry, "ghost").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ToolError::NotFound(_)));
}

#[tokio::test]
async fn test_resolve_agent_ref_empty_input() {
    let registry = AgentRegistry::new();

    let result = resolve_agent_ref(&registry, "").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));

    // Whitespace-only should also be rejected
    let result = resolve_agent_ref(&registry, "   ").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
}

#[test]
fn test_safe_truncate_utf8_multibyte() {
    // Test with French accented characters
    let text = "Ceci est un texte en francais avec des accents: e, a, o, i, u";
    let truncated = safe_truncate(text, 50, true);
    assert!(truncated.ends_with("..."));
    assert!(!truncated.contains("\\u")); // No escaped unicode

    // Test with text where byte 100 is inside a multi-byte char
    // This is the exact scenario that caused the panic
    let mission_text = "# MISSION\nRechercher sources fiables sur ACTUALITE pour: Mistral AI nouveautes 2025 actualites recentes lancements produits";
    let truncated = safe_truncate(mission_text, 100, true);
    assert!(truncated.ends_with("..."));

    // Test with emojis (4-byte UTF-8)
    let emoji_text = "Test avec emojis X et Y et beaucoup de texte apres pour depasser la limite";
    let truncated = safe_truncate(emoji_text, 30, true);
    assert!(truncated.ends_with("..."));

    // Test short text (no truncation needed)
    let short_text = "Court";
    let not_truncated = safe_truncate(short_text, 100, false);
    assert_eq!(not_truncated, "Court");
}
