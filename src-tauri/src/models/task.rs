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

//! Task model for workflow decomposition and Todo Tool support.
//!
//! Tasks break down complex workflows into trackable units with:
//! - Priority levels (1=critical to 5=low)
//! - Status tracking (pending/in_progress/completed/blocked)
//! - Agent assignment for multi-agent coordination
//! - Dependency management between tasks
//! - Duration tracking for metrics

use super::serde_utils::deserialize_thing_id;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Task status for workflow decomposition.
///
/// Represents the current state of a task in its lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is waiting to be started
    #[default]
    Pending,
    /// Task is currently being worked on
    InProgress,
    /// Task has been finished successfully
    Completed,
    /// Task is blocked by dependencies or external factors
    Blocked,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Completed => write!(f, "completed"),
            Self::Blocked => write!(f, "blocked"),
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "in_progress" => Ok(Self::InProgress),
            "completed" => Ok(Self::Completed),
            "blocked" => Ok(Self::Blocked),
            _ => Err(format!("Invalid task status: {}", s)),
        }
    }
}

/// Task priority level (1=critical, 5=low).
///
/// Priority determines the order of task execution:
/// - 1: Critical - must be done immediately
/// - 2: High - should be done soon
/// - 3: Medium - normal priority (default)
/// - 4: Low - can wait
/// - 5: Minimal - do when time permits
pub type TaskPriority = u8;

/// Default priority for new tasks.
fn default_priority() -> TaskPriority {
    3 // Medium priority
}

/// Task entity for workflow decomposition.
///
/// Represents a single unit of work within a workflow, with support for:
/// - Agent assignment for multi-agent coordination
/// - Priority-based scheduling
/// - Dependency tracking between tasks
/// - Execution metrics (duration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier (deserialized from SurrealDB Thing type)
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub id: String,

    /// Associated workflow ID
    pub workflow_id: String,

    /// Task name (short identifier, max 128 chars)
    pub name: String,

    /// Detailed description (max 1000 chars)
    pub description: String,

    /// Agent responsible for this task (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_assigned: Option<String>,

    /// Priority level (1-5, 1=critical)
    #[serde(default = "default_priority")]
    pub priority: TaskPriority,

    /// Current status
    #[serde(default)]
    pub status: TaskStatus,

    /// Task dependencies (other task IDs that must complete first)
    #[serde(default)]
    pub dependencies: Vec<String>,

    /// Execution duration in milliseconds (if completed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    /// Creation timestamp
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,

    /// Completion timestamp (if completed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
}

/// Task creation payload - only fields needed for creation.
///
/// ID is passed separately to db.create() using table:id format.
/// Timestamps are handled by database defaults.
/// Enum fields are converted to strings for SurrealDB compatibility.
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)] // Will be used in Phase 2: Tauri Commands
pub struct TaskCreate {
    /// Associated workflow ID
    pub workflow_id: String,

    /// Task name (short identifier)
    pub name: String,

    /// Detailed description
    pub description: String,

    /// Agent responsible for this task (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_assigned: Option<String>,

    /// Priority level (1-5, 1=critical)
    pub priority: TaskPriority,

    /// Current status (as string for SurrealDB)
    pub status: String,

    /// Task dependencies (other task IDs)
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[allow(dead_code)] // Will be used in Phase 2: Tauri Commands
impl TaskCreate {
    /// Creates a new TaskCreate with the given parameters.
    ///
    /// # Arguments
    /// * `workflow_id` - Associated workflow ID
    /// * `name` - Task name (max 128 chars)
    /// * `description` - Task description (max 1000 chars)
    /// * `priority` - Priority level 1-5 (1=critical)
    ///
    /// # Examples
    /// ```ignore
    /// let task = TaskCreate::new(
    ///     "wf_001".to_string(),
    ///     "Analyze code".to_string(),
    ///     "Deep analysis of the codebase".to_string(),
    ///     1, // Critical priority
    /// );
    /// ```
    pub fn new(
        workflow_id: String,
        name: String,
        description: String,
        priority: TaskPriority,
    ) -> Self {
        Self {
            workflow_id,
            name,
            description,
            agent_assigned: None,
            priority,
            status: TaskStatus::Pending.to_string(),
            dependencies: Vec::new(),
        }
    }

    /// Assigns an agent to the task.
    ///
    /// # Arguments
    /// * `agent_id` - The ID of the agent to assign
    pub fn with_agent(mut self, agent_id: String) -> Self {
        self.agent_assigned = Some(agent_id);
        self
    }

    /// Sets task dependencies.
    ///
    /// # Arguments
    /// * `deps` - List of task IDs that must complete before this task
    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = deps;
        self
    }
}

/// Task update payload for partial updates.
///
/// All fields are optional - only provided fields will be updated.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskUpdate {
    /// New task name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// New description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// New agent assignment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_assigned: Option<String>,

    /// New priority
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<TaskPriority>,

    /// New status (as string)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    /// New dependencies list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<Vec<String>>,

    /// Execution duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

#[allow(dead_code)] // Will be used in Phase 2: Tauri Commands
impl TaskUpdate {
    /// Creates a new empty TaskUpdate.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the name field.
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the description field.
    pub fn description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Sets the agent_assigned field.
    pub fn agent_assigned(mut self, agent_id: String) -> Self {
        self.agent_assigned = Some(agent_id);
        self
    }

    /// Sets the priority field.
    pub fn priority(mut self, priority: TaskPriority) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Sets the status field.
    pub fn status(mut self, status: TaskStatus) -> Self {
        self.status = Some(status.to_string());
        self
    }

    /// Sets the dependencies field.
    pub fn dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = Some(deps);
        self
    }

    /// Sets the duration_ms field.
    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = Some(duration);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_serialization() {
        let status = TaskStatus::InProgress;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"in_progress\"");

        let deserialized: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, TaskStatus::InProgress);
    }

    #[test]
    fn test_task_status_display() {
        assert_eq!(TaskStatus::Pending.to_string(), "pending");
        assert_eq!(TaskStatus::InProgress.to_string(), "in_progress");
        assert_eq!(TaskStatus::Completed.to_string(), "completed");
        assert_eq!(TaskStatus::Blocked.to_string(), "blocked");
    }

    #[test]
    fn test_task_status_from_str() {
        assert_eq!(
            "pending".parse::<TaskStatus>().unwrap(),
            TaskStatus::Pending
        );
        assert_eq!(
            "in_progress".parse::<TaskStatus>().unwrap(),
            TaskStatus::InProgress
        );
        assert_eq!(
            "completed".parse::<TaskStatus>().unwrap(),
            TaskStatus::Completed
        );
        assert_eq!(
            "blocked".parse::<TaskStatus>().unwrap(),
            TaskStatus::Blocked
        );
        assert!("invalid".parse::<TaskStatus>().is_err());
    }

    #[test]
    fn test_task_status_default() {
        let status: TaskStatus = Default::default();
        assert_eq!(status, TaskStatus::Pending);
    }

    #[test]
    fn test_task_create_builder() {
        let task = TaskCreate::new(
            "wf_001".to_string(),
            "Analyze code".to_string(),
            "Deep analysis of the codebase".to_string(),
            1, // Critical priority
        )
        .with_agent("db_agent".to_string())
        .with_dependencies(vec!["task_001".to_string()]);

        assert_eq!(task.workflow_id, "wf_001");
        assert_eq!(task.name, "Analyze code");
        assert_eq!(task.priority, 1);
        assert_eq!(task.agent_assigned, Some("db_agent".to_string()));
        assert_eq!(task.dependencies.len(), 1);
        assert_eq!(task.status, "pending");
    }

    #[test]
    fn test_task_create_serialization() {
        let task = TaskCreate::new(
            "wf_001".to_string(),
            "Test task".to_string(),
            "A test task".to_string(),
            3,
        );

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"workflow_id\":\"wf_001\""));
        assert!(json.contains("\"name\":\"Test task\""));
        assert!(json.contains("\"priority\":3"));
        assert!(json.contains("\"status\":\"pending\""));
        // agent_assigned should be skipped when None
        assert!(!json.contains("agent_assigned"));
    }

    #[test]
    fn test_task_update_builder() {
        let update = TaskUpdate::new()
            .priority(2)
            .status(TaskStatus::InProgress)
            .agent_assigned("api_agent".to_string());

        assert_eq!(update.priority, Some(2));
        assert_eq!(update.status, Some("in_progress".to_string()));
        assert_eq!(update.agent_assigned, Some("api_agent".to_string()));
        assert!(update.name.is_none());
        assert!(update.description.is_none());
    }

    #[test]
    fn test_task_update_serialization() {
        let update = TaskUpdate::new().priority(1).status(TaskStatus::Completed);

        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("\"priority\":1"));
        assert!(json.contains("\"status\":\"completed\""));
        // None fields should be skipped
        assert!(!json.contains("name"));
        assert!(!json.contains("description"));
        assert!(!json.contains("agent_assigned"));
    }

    #[test]
    fn test_priority_range() {
        // Priority should be 1-5
        let valid_priorities: Vec<TaskPriority> = vec![1, 2, 3, 4, 5];
        for p in valid_priorities {
            let task = TaskCreate::new("wf".to_string(), "test".to_string(), "desc".to_string(), p);
            assert!(task.priority >= 1 && task.priority <= 5);
        }
    }

    #[test]
    fn test_default_priority() {
        let task = TaskCreate::new(
            "wf".to_string(),
            "test".to_string(),
            "desc".to_string(),
            default_priority(),
        );
        assert_eq!(task.priority, 3);
    }

    #[test]
    fn test_task_serialization() {
        let task = Task {
            id: "task_001".to_string(),
            workflow_id: "wf_001".to_string(),
            name: "Test task".to_string(),
            description: "A test task description".to_string(),
            agent_assigned: Some("db_agent".to_string()),
            priority: 2,
            status: TaskStatus::InProgress,
            dependencies: vec!["task_000".to_string()],
            duration_ms: None,
            created_at: Utc::now(),
            completed_at: None,
        };

        let json = serde_json::to_string(&task).unwrap();
        let deserialized: Task = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, task.id);
        assert_eq!(deserialized.name, task.name);
        assert_eq!(deserialized.priority, task.priority);
        assert_eq!(deserialized.status, task.status);
        assert_eq!(deserialized.agent_assigned, task.agent_assigned);
        assert_eq!(deserialized.dependencies.len(), 1);
    }

    #[test]
    fn test_task_with_duration() {
        let task = Task {
            id: "task_002".to_string(),
            workflow_id: "wf_001".to_string(),
            name: "Completed task".to_string(),
            description: "A completed task".to_string(),
            agent_assigned: None,
            priority: 3,
            status: TaskStatus::Completed,
            dependencies: vec![],
            duration_ms: Some(5000),
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
        };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"duration_ms\":5000"));
        assert!(json.contains("\"status\":\"completed\""));
        assert!(json.contains("completed_at"));
    }

    #[test]
    fn test_task_skip_none_fields() {
        let task = Task {
            id: "task_003".to_string(),
            workflow_id: "wf_001".to_string(),
            name: "Minimal task".to_string(),
            description: "Minimal".to_string(),
            agent_assigned: None,
            priority: 3,
            status: TaskStatus::Pending,
            dependencies: vec![],
            duration_ms: None,
            created_at: Utc::now(),
            completed_at: None,
        };

        let json = serde_json::to_string(&task).unwrap();
        // None fields should be skipped
        assert!(!json.contains("agent_assigned"));
        assert!(!json.contains("duration_ms"));
        assert!(!json.contains("completed_at"));
    }
}
