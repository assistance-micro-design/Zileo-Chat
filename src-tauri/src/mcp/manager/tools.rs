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

//! MCP Manager tool operations
//!
//! Tool invocation with retry logic, tool listing, and cache management.

use super::{MCPManager, MCP_INITIAL_RETRY_DELAY_MS, MCP_MAX_RETRY_ATTEMPTS, TOOL_CACHE_TTL};
use crate::mcp::{MCPError, MCPResult};
use crate::models::mcp::{MCPCallLogCreate, MCPTool, MCPToolCallRequest, MCPToolCallResult};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use uuid::Uuid;

impl MCPManager {
    /// Calls a tool on a specific server
    ///
    /// Uses circuit breaker pattern to prevent cascade failures.
    /// If the circuit is open (server unhealthy), the call will fail fast.
    ///
    /// # Arguments
    ///
    /// * `server_name` - The NAME of the MCP server (not ID)
    /// * `tool_name` - Name of the tool to invoke
    /// * `arguments` - Tool arguments as JSON value
    ///
    /// # Returns
    ///
    /// Returns the tool call result.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Server or tool doesn't exist
    /// - Circuit breaker is open (server unhealthy)
    /// - The call itself fails after all retry attempts
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> MCPResult<MCPToolCallResult> {
        debug!(
            server_name = %server_name,
            tool_name = %tool_name,
            "Calling MCP tool"
        );

        // Check circuit breaker before making the call
        {
            let mut breakers = self.circuit_breakers.write().await;
            if let Some(breaker) = breakers.get_mut(server_name) {
                if !breaker.allow_request() {
                    let remaining = breaker
                        .remaining_cooldown()
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    return Err(MCPError::CircuitBreakerOpen {
                        server: server_name.to_string(),
                        cooldown_remaining_secs: remaining,
                    });
                }
            }
        }

        let start = Instant::now();
        let mut last_error: Option<MCPError> = None;

        // Retry loop with exponential backoff
        for attempt in 0..=MCP_MAX_RETRY_ATTEMPTS {
            let result = {
                let mut clients = self.clients.write().await;
                // Clients are keyed by server NAME
                let client = clients
                    .get_mut(server_name)
                    .ok_or(MCPError::ServerNotFound {
                        server: server_name.to_string(),
                    })?;

                client.call_tool(tool_name, arguments.clone()).await
            };

            match result {
                Ok(call_result) => {
                    let duration_ms = start.elapsed().as_millis() as u64;

                    // Update circuit breaker on success
                    {
                        let mut breakers = self.circuit_breakers.write().await;
                        if let Some(breaker) = breakers.get_mut(server_name) {
                            breaker.record_success();
                        }
                    }

                    // Log successful call
                    let log_entry = MCPCallLogCreate {
                        id: Uuid::new_v4().to_string(),
                        workflow_id: None,
                        server_name: server_name.to_string(),
                        tool_name: tool_name.to_string(),
                        params: arguments.clone(),
                        result: call_result.content.clone(),
                        success: call_result.success,
                        duration_ms,
                    };

                    if let Err(e) = self.log_call(log_entry).await {
                        warn!(error = %e, "Failed to log MCP call to database");
                    }

                    if attempt > 0 {
                        info!(
                            server_name = %server_name,
                            tool_name = %tool_name,
                            attempt = attempt + 1,
                            "MCP tool call succeeded on retry"
                        );
                    }

                    return Ok(call_result);
                }
                Err(e) => {
                    // Check if error is retryable
                    let is_retryable = Self::is_retryable_error(&e);

                    if !is_retryable || attempt >= MCP_MAX_RETRY_ATTEMPTS {
                        // Non-retryable error or exhausted retries
                        let duration_ms = start.elapsed().as_millis() as u64;

                        // Update circuit breaker on failure
                        {
                            let mut breakers = self.circuit_breakers.write().await;
                            if let Some(breaker) = breakers.get_mut(server_name) {
                                breaker.record_failure();
                            }
                        }

                        // Invalidate tool cache on failure
                        self.invalidate_tool_cache(server_name).await;

                        // Log failed call
                        let log_entry = MCPCallLogCreate {
                            id: Uuid::new_v4().to_string(),
                            workflow_id: None,
                            server_name: server_name.to_string(),
                            tool_name: tool_name.to_string(),
                            params: arguments.clone(),
                            result: serde_json::Value::Null,
                            success: false,
                            duration_ms,
                        };

                        if let Err(log_err) = self.log_call(log_entry).await {
                            warn!(error = %log_err, "Failed to log MCP call to database");
                        }

                        if attempt > 0 {
                            return Err(MCPError::RetryExhausted {
                                server: server_name.to_string(),
                                attempts: attempt + 1,
                                last_error: e.to_string(),
                            });
                        }

                        return Err(e);
                    }

                    // Retryable error - wait and retry
                    let delay_ms = MCP_INITIAL_RETRY_DELAY_MS * 2_u64.pow(attempt);
                    warn!(
                        server_name = %server_name,
                        tool_name = %tool_name,
                        attempt = attempt + 1,
                        max_attempts = MCP_MAX_RETRY_ATTEMPTS + 1,
                        delay_ms = delay_ms,
                        error = %e,
                        "Retrying MCP tool call after transient error"
                    );

                    last_error = Some(e);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }

        // Should not reach here, but just in case
        Err(last_error.unwrap_or_else(|| MCPError::IoError {
            context: "unexpected retry state".to_string(),
            message: "No error recorded during retry loop".to_string(),
        }))
    }

    /// Determines if an MCP error is retryable.
    fn is_retryable_error(error: &MCPError) -> bool {
        matches!(
            error,
            MCPError::Timeout { .. } | MCPError::ConnectionFailed { .. } | MCPError::IoError { .. }
        )
    }

    /// Calls a tool using a request object
    ///
    /// Convenience method that extracts parameters from `MCPToolCallRequest`.
    pub async fn call_tool_request(
        &self,
        request: MCPToolCallRequest,
    ) -> MCPResult<MCPToolCallResult> {
        self.call_tool(&request.server_name, &request.tool_name, request.arguments)
            .await
    }

    /// Lists tools available on a specific server by NAME.
    ///
    /// Uses a cache with 1-hour TTL to avoid redundant calls.
    /// Cache is automatically invalidated on tool call errors.
    pub async fn list_server_tools(&self, server_name: &str) -> Vec<MCPTool> {
        // Check cache first
        {
            let cache = self.tool_cache.read().await;
            if let Some((tools, cached_at)) = cache.get(server_name) {
                if cached_at.elapsed() < TOOL_CACHE_TTL {
                    debug!(server = %server_name, "Tool cache hit");
                    return tools.clone();
                }
            }
        }

        // Cache miss or expired - fetch from client
        debug!(server = %server_name, "Tool cache miss, fetching from client");
        let clients = self.clients.read().await;
        let tools = clients
            .get(server_name)
            .map(|c| c.tools().to_vec())
            .unwrap_or_default();

        // Update cache
        if !tools.is_empty() {
            let mut cache = self.tool_cache.write().await;
            cache.insert(server_name.to_string(), (tools.clone(), Instant::now()));
        }

        tools
    }

    /// Invalidates the tool cache for a specific server.
    pub async fn invalidate_tool_cache(&self, server_name: &str) {
        let mut cache = self.tool_cache.write().await;
        cache.remove(server_name);
        debug!(server = %server_name, "Tool cache invalidated");
    }
}
