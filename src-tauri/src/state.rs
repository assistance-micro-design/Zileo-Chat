/// AppState - to be implemented in Phase 4
pub struct AppState {
    // Phase 1: Database client
    // Phase 3: Agent registry and orchestrator
}

impl AppState {
    /// Creates new AppState - to be implemented
    pub async fn new(_db_path: &str) -> anyhow::Result<Self> {
        Ok(Self {})
    }
}
