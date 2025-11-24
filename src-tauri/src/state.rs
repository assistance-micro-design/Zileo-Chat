// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::db::DBClient;
use std::sync::Arc;

/// Application state shared across Tauri commands
pub struct AppState {
    /// Database client
    pub db: Arc<DBClient>,
    /// Agent registry
    pub registry: Arc<AgentRegistry>,
    /// Agent orchestrator
    pub orchestrator: Arc<AgentOrchestrator>,
}

impl AppState {
    /// Creates new application state
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        // Initialize database
        let db = Arc::new(DBClient::new(db_path).await?);
        db.initialize_schema().await?;

        // Initialize agent registry and orchestrator
        let registry = Arc::new(AgentRegistry::new());
        let orchestrator = Arc::new(AgentOrchestrator::new(registry.clone()));

        Ok(Self {
            db,
            registry,
            orchestrator,
        })
    }
}
