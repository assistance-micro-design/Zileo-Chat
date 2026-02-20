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

/// Maximum nesting depth for sanitization to prevent stack overflow
/// on maliciously crafted deeply nested JSON from external sources.
const MAX_SANITIZE_DEPTH: usize = 64;

/// Sanitizes a JSON value for SurrealDB by removing null characters.
///
/// SurrealDB's Strand type panics on strings containing `\0` characters.
/// This function recursively removes null characters from all string values
/// in the JSON structure. Nesting depth is limited to prevent stack overflow
/// on malicious input.
///
/// # Arguments
///
/// * `value` - The JSON value to sanitize
///
/// # Returns
///
/// A new JSON value with all null characters removed from strings.
/// Values nested beyond `MAX_SANITIZE_DEPTH` are replaced with `null`.
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
    sanitize_recursive(value, 0)
}

fn sanitize_recursive(value: Value, depth: usize) -> Value {
    if depth > MAX_SANITIZE_DEPTH {
        return Value::Null;
    }
    match value {
        Value::String(s) => {
            // Remove null characters from strings
            Value::String(s.replace('\0', ""))
        }
        Value::Array(arr) => {
            // Recursively sanitize array elements
            Value::Array(
                arr.into_iter()
                    .map(|v| sanitize_recursive(v, depth + 1))
                    .collect(),
            )
        }
        Value::Object(obj) => {
            // Recursively sanitize object values
            Value::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, sanitize_recursive(v, depth + 1)))
                    .collect(),
            )
        }
        // Other types (Null, Bool, Number) are passed through unchanged
        other => other,
    }
}

/// Maximum number of entities allowed in a single import file.
/// Prevents DoS via extremely large import files.
pub const MAX_IMPORT_ENTITIES: usize = 100;

/// Extracts a count value from a `SELECT count() ... GROUP ALL` query result.
///
/// SurrealDB's `SELECT count() ... GROUP ALL` returns a single-element array
/// with a JSON object containing a `"count"` field. This helper extracts
/// the numeric value, returning 0 if the result is empty or malformed.
///
/// # Arguments
///
/// * `results` - The query result as a `Vec<serde_json::Value>`
///
/// # Returns
///
/// The count as `u64`, or 0 if the result is empty/malformed.
///
/// # Example
///
/// ```ignore
/// let results: Vec<serde_json::Value> = db.query("SELECT count() FROM memory GROUP ALL").await?;
/// let total = extract_count(&results);
/// ```
pub fn extract_count(results: &[Value]) -> u64 {
    results
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0)
}

/// Checks whether a `SELECT count() ... GROUP ALL` result indicates at least one record exists.
///
/// Convenience wrapper over [`extract_count`] for existence checks.
///
/// # Example
///
/// ```ignore
/// let results: Vec<serde_json::Value> = db.query(
///     "SELECT count() FROM llm_model WHERE api_name = $name GROUP ALL"
/// ).await?;
/// if count_exists(&results) {
///     return Err("Model already exists".to_string());
/// }
/// ```
pub fn count_exists(results: &[Value]) -> bool {
    extract_count(results) > 0
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

    #[test]
    fn test_sanitize_deeply_nested_json_truncated() {
        // Build JSON with 100 levels of nesting (exceeds MAX_SANITIZE_DEPTH of 64)
        let mut value = json!("leaf\0value");
        for _ in 0..100 {
            value = json!({"nested": value});
        }
        let result = sanitize_for_surrealdb(value);
        // Should not stack overflow, and deep levels should be truncated to null
        assert!(result.is_object());

        // Walk down to verify truncation at depth limit
        // depth 0 = outermost object, depth > 64 = truncated to null
        let mut current = &result;
        for _ in 0..65 {
            current = &current["nested"];
        }
        // At depth 65+ (depth > MAX_SANITIZE_DEPTH), values become null
        assert!(
            current.is_null(),
            "Values beyond MAX_SANITIZE_DEPTH should be null"
        );
    }

    #[test]
    fn test_sanitize_normal_depth_preserved() {
        // Build JSON with 10 levels (well within limit)
        let mut value = json!("leaf\0text");
        for _ in 0..10 {
            value = json!({"nested": value});
        }
        let result = sanitize_for_surrealdb(value);

        // Walk down 10 levels - leaf should be preserved and sanitized
        let mut current = &result;
        for _ in 0..10 {
            current = &current["nested"];
        }
        assert_eq!(current, &json!("leaftext"));
    }

    // extract_count tests
    #[test]
    fn test_extract_count_valid_result() {
        let results = vec![json!({"count": 42})];
        assert_eq!(extract_count(&results), 42);
    }

    #[test]
    fn test_extract_count_zero() {
        let results = vec![json!({"count": 0})];
        assert_eq!(extract_count(&results), 0);
    }

    #[test]
    fn test_extract_count_empty_vec() {
        let results: Vec<serde_json::Value> = vec![];
        assert_eq!(extract_count(&results), 0);
    }

    #[test]
    fn test_extract_count_missing_field() {
        let results = vec![json!({"other": 10})];
        assert_eq!(extract_count(&results), 0);
    }

    #[test]
    fn test_extract_count_non_numeric() {
        let results = vec![json!({"count": "not_a_number"})];
        assert_eq!(extract_count(&results), 0);
    }

    #[test]
    fn test_extract_count_large_value() {
        let results = vec![json!({"count": 999_999})];
        assert_eq!(extract_count(&results), 999_999);
    }

    // count_exists tests
    #[test]
    fn test_count_exists_true() {
        let results = vec![json!({"count": 3})];
        assert!(count_exists(&results));
    }

    #[test]
    fn test_count_exists_false_zero() {
        let results = vec![json!({"count": 0})];
        assert!(!count_exists(&results));
    }

    #[test]
    fn test_count_exists_false_empty() {
        let results: Vec<serde_json::Value> = vec![];
        assert!(!count_exists(&results));
    }
}
