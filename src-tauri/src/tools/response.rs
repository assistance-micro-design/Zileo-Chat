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

//! Type-safe JSON response builder for tools.

use serde::Serialize;
use serde_json::{json, Map, Value};

/// Fluent builder for standardized JSON responses.
pub struct ResponseBuilder {
    data: Map<String, Value>,
}

impl ResponseBuilder {
    pub fn new() -> Self {
        Self { data: Map::new() }
    }

    /// Adds success field.
    pub fn success(mut self, value: bool) -> Self {
        self.data.insert("success".to_string(), json!(value));
        self
    }

    /// Adds a message.
    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.data.insert("message".to_string(), json!(msg.into()));
        self
    }

    /// Adds an ID with custom key (e.g., "memory_id", "task_id").
    pub fn id(mut self, key: &str, id: impl Into<String>) -> Self {
        self.data.insert(key.to_string(), json!(id.into()));
        self
    }

    /// Adds a custom field.
    pub fn field(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.data.insert(key.to_string(), value.into());
        self
    }

    /// Adds a count.
    pub fn count(mut self, n: usize) -> Self {
        self.data.insert("count".to_string(), json!(n));
        self
    }

    /// Adds serializable data.
    pub fn data<T: Serialize>(mut self, key: &str, value: T) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.data.insert(key.to_string(), v);
        }
        self
    }

    /// Builds the final Value.
    pub fn build(self) -> Value {
        Value::Object(self.data)
    }
}

impl Default for ResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// Helper methods for common responses
impl ResponseBuilder {
    /// Standard success response.
    pub fn ok(id_key: &str, id: impl Into<String>, msg: impl Into<String>) -> Value {
        Self::new()
            .success(true)
            .id(id_key, id)
            .message(msg)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_builder_success() {
        let response = ResponseBuilder::new()
            .success(true)
            .id("memory_id", "abc123")
            .message("Memory created")
            .build();

        assert_eq!(response["success"], true);
        assert_eq!(response["memory_id"], "abc123");
        assert_eq!(response["message"], "Memory created");
    }

    #[test]
    fn test_response_builder_ok_helper() {
        let response = ResponseBuilder::ok("task_id", "task-1", "Task created");

        assert_eq!(response["success"], true);
        assert_eq!(response["task_id"], "task-1");
        assert_eq!(response["message"], "Task created");
    }

    #[test]
    fn test_response_builder_with_custom_data() {
        #[derive(Serialize)]
        struct Info {
            name: String,
            count: u32,
        }

        let info = Info {
            name: "test".to_string(),
            count: 42,
        };

        let response = ResponseBuilder::new()
            .success(true)
            .data("info", info)
            .build();

        assert_eq!(response["info"]["name"], "test");
        assert_eq!(response["info"]["count"], 42);
    }
}
