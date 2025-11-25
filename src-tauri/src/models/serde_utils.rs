// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Serde utilities for SurrealDB compatibility.
//!
//! SurrealDB returns record IDs as `Thing` objects in format `table:id`,
//! but our Rust structs expect plain strings. These utilities handle
//! the conversion transparently during deserialization.

use serde::{Deserialize, Deserializer};

/// Deserializes a SurrealDB record ID (Thing) to a String.
///
/// SurrealDB returns IDs in the format `{ "tb": "table", "id": "uuid" }`
/// or as a string `"table:uuid"`. This function handles both formats
/// and extracts just the ID portion.
///
/// # Examples
///
/// ```ignore
/// #[derive(Deserialize)]
/// struct Record {
///     #[serde(deserialize_with = "deserialize_thing_id")]
///     id: String,
/// }
/// ```
pub fn deserialize_thing_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    // Use an untagged enum to handle multiple formats
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ThingOrString {
        // SurrealDB Thing object format
        Thing { id: ThingId },
        // Plain string (table:id or just id)
        String(String),
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ThingId {
        // String ID
        String(String),
        // Object with inner string (field name must match JSON key)
        #[allow(non_snake_case)]
        Object { String: String },
    }

    match ThingOrString::deserialize(deserializer)? {
        ThingOrString::Thing { id } => match id {
            ThingId::String(s) => Ok(s),
            ThingId::Object { String: s } => Ok(s),
        },
        ThingOrString::String(s) => {
            // If string contains ':', extract the ID part
            if let Some((_table, id)) = s.split_once(':') {
                Ok(id.to_string())
            } else {
                Ok(s)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    struct TestRecord {
        #[serde(deserialize_with = "deserialize_thing_id")]
        id: String,
        #[allow(dead_code)]
        name: String,
    }

    #[test]
    fn test_deserialize_plain_string() {
        let json = r#"{"id": "abc-123", "name": "test"}"#;
        let record: TestRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.id, "abc-123");
    }

    #[test]
    fn test_deserialize_table_colon_id() {
        let json = r#"{"id": "workflow:abc-123", "name": "test"}"#;
        let record: TestRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.id, "abc-123");
    }

    #[test]
    fn test_deserialize_thing_object_string() {
        let json = r#"{"id": {"id": "abc-123"}, "name": "test"}"#;
        let record: TestRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.id, "abc-123");
    }

    #[test]
    fn test_deserialize_thing_object_nested() {
        let json = r#"{"id": {"id": {"String": "abc-123"}}, "name": "test"}"#;
        let record: TestRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.id, "abc-123");
    }
}
