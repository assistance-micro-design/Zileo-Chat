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

//! # Database Utilities
//!
//! Utility functions for SurrealDB data handling.
//!
//! ## Overview
//!
//! SurrealDB has specific requirements for string data:
//! - Null characters (`\0`) cause panics in the Strand type
//! - These functions sanitize data before database insertion

use serde_json::Value;

/// Sanitizes a JSON value for SurrealDB by removing null characters.
///
/// SurrealDB's Strand type panics on strings containing `\0` characters.
/// This function recursively removes null characters from all string values
/// in the JSON structure.
///
/// # Arguments
///
/// * `value` - The JSON value to sanitize
///
/// # Returns
///
/// A new JSON value with all null characters removed from strings.
///
/// # Example
///
/// ```ignore
/// use serde_json::json;
/// use zileo_chat::db::sanitize_for_surrealdb;
///
/// let dirty = json!({"text": "hello\0world"});
/// let clean = sanitize_for_surrealdb(dirty);
/// assert_eq!(clean["text"], "helloworld");
/// ```
pub fn sanitize_for_surrealdb(value: Value) -> Value {
    match value {
        Value::String(s) => {
            // Remove null characters from strings
            Value::String(s.replace('\0', ""))
        }
        Value::Array(arr) => {
            // Recursively sanitize array elements
            Value::Array(arr.into_iter().map(sanitize_for_surrealdb).collect())
        }
        Value::Object(obj) => {
            // Recursively sanitize object values
            Value::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, sanitize_for_surrealdb(v)))
                    .collect(),
            )
        }
        // Other types (Null, Bool, Number) are passed through unchanged
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_sanitize_simple_string() {
        let value = json!("hello\0world");
        let result = sanitize_for_surrealdb(value);
        assert_eq!(result, json!("helloworld"));
    }

    #[test]
    fn test_sanitize_string_without_null() {
        let value = json!("hello world");
        let result = sanitize_for_surrealdb(value);
        assert_eq!(result, json!("hello world"));
    }

    #[test]
    fn test_sanitize_nested_object() {
        let value = json!({
            "text": "hello\0world",
            "nested": {
                "inner": "foo\0bar"
            }
        });
        let result = sanitize_for_surrealdb(value);
        assert_eq!(result["text"], "helloworld");
        assert_eq!(result["nested"]["inner"], "foobar");
    }

    #[test]
    fn test_sanitize_array() {
        let value = json!(["hello\0", "world\0test"]);
        let result = sanitize_for_surrealdb(value);
        assert_eq!(result, json!(["hello", "worldtest"]));
    }

    #[test]
    fn test_sanitize_mixed_types() {
        let value = json!({
            "string": "test\0value",
            "number": 42,
            "bool": true,
            "null": null,
            "array": ["item\0one", 123]
        });
        let result = sanitize_for_surrealdb(value);
        assert_eq!(result["string"], "testvalue");
        assert_eq!(result["number"], 42);
        assert_eq!(result["bool"], true);
        assert!(result["null"].is_null());
        assert_eq!(result["array"][0], "itemone");
        assert_eq!(result["array"][1], 123);
    }

    #[test]
    fn test_sanitize_multiple_null_chars() {
        let value = json!("\0\0hello\0\0world\0\0");
        let result = sanitize_for_surrealdb(value);
        assert_eq!(result, json!("helloworld"));
    }

    #[test]
    fn test_sanitize_empty_string() {
        let value = json!("");
        let result = sanitize_for_surrealdb(value);
        assert_eq!(result, json!(""));
    }

    #[test]
    fn test_sanitize_only_null_chars() {
        let value = json!("\0\0\0");
        let result = sanitize_for_surrealdb(value);
        assert_eq!(result, json!(""));
    }
}
