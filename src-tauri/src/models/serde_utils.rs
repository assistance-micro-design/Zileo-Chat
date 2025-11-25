// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Serde utilities for SurrealDB compatibility.
//!
//! SurrealDB returns record IDs as `Thing` objects in format `table:id`,
//! but our Rust structs expect plain strings. These utilities handle
//! the conversion transparently during deserialization.

use serde::{de, Deserialize, Deserializer};
use std::fmt;

/// Deserializes a SurrealDB record ID (Thing) to a String.
///
/// SurrealDB returns IDs in various formats:
/// - Plain string: `"uuid"` or `"table:uuid"`
/// - Thing object: `{ "tb": "table", "id": "uuid" }` or similar structures
///
/// This function handles all these formats and extracts just the ID portion.
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
    struct ThingIdVisitor;

    impl<'de> de::Visitor<'de> for ThingIdVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string, SurrealDB Thing, or record ID")
        }

        // Handle plain string input
        fn visit_str<E>(self, value: &str) -> Result<String, E>
        where
            E: de::Error,
        {
            // If string contains ':', extract the ID part (table:id format)
            if let Some((_table, id)) = value.split_once(':') {
                Ok(id.to_string())
            } else {
                Ok(value.to_string())
            }
        }

        fn visit_string<E>(self, value: String) -> Result<String, E>
        where
            E: de::Error,
        {
            self.visit_str(&value)
        }

        // Handle map/object input (SurrealDB Thing format)
        fn visit_map<M>(self, mut map: M) -> Result<String, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            let mut id_value: Option<String> = None;
            let mut tb_value: Option<String> = None;

            while let Some(key) = map.next_key::<String>()? {
                match key.as_str() {
                    "id" => {
                        // The id field can be a string or another nested structure
                        let value: serde_json::Value = map.next_value()?;
                        id_value = Some(extract_id_from_value(&value));
                    }
                    "tb" => {
                        tb_value = Some(map.next_value()?);
                    }
                    _ => {
                        // Skip unknown fields
                        let _: serde_json::Value = map.next_value()?;
                    }
                }
            }

            // Return the ID if found
            if let Some(id) = id_value {
                Ok(id)
            } else if let Some(tb) = tb_value {
                // Fallback: if only tb is present, use it as the ID
                Ok(tb)
            } else {
                Err(de::Error::custom("Missing 'id' field in Thing object"))
            }
        }

        // Handle newtype struct (SurrealDB might wrap the value)
        fn visit_newtype_struct<D2>(self, deserializer: D2) -> Result<String, D2::Error>
        where
            D2: Deserializer<'de>,
        {
            // Try to deserialize the inner value
            let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
            Ok(extract_id_from_value(&value))
        }

        // Handle enum input (SurrealDB internal format)
        fn visit_enum<A>(self, data: A) -> Result<String, A::Error>
        where
            A: de::EnumAccess<'de>,
        {
            use serde::de::VariantAccess;

            let (variant, accessor): (String, _) = data.variant()?;

            // The variant name might be the type, and the data contains the ID
            match variant.as_str() {
                "String" | "Id" | "Thing" => {
                    let value: serde_json::Value = accessor.newtype_variant()?;
                    Ok(extract_id_from_value(&value))
                }
                _ => {
                    // Try to get the value anyway
                    let value: serde_json::Value = accessor.newtype_variant()?;
                    Ok(extract_id_from_value(&value))
                }
            }
        }
    }

    deserializer.deserialize_any(ThingIdVisitor)
}

/// Extracts an ID string from a serde_json::Value
fn extract_id_from_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => {
            // If string contains ':', extract the ID part
            if let Some((_table, id)) = s.split_once(':') {
                id.to_string()
            } else {
                s.clone()
            }
        }
        serde_json::Value::Object(map) => {
            // Try to find 'id' or 'String' key
            if let Some(id) = map.get("id") {
                extract_id_from_value(id)
            } else if let Some(s) = map.get("String") {
                extract_id_from_value(s)
            } else {
                // Return the object as a JSON string as fallback
                value.to_string()
            }
        }
        serde_json::Value::Number(n) => n.to_string(),
        _ => value.to_string(),
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

    #[test]
    fn test_deserialize_thing_with_tb() {
        let json = r#"{"id": {"tb": "workflow", "id": "abc-123"}, "name": "test"}"#;
        let record: TestRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.id, "abc-123");
    }

    #[test]
    fn test_extract_id_from_string_value() {
        let value = serde_json::json!("test-id");
        assert_eq!(extract_id_from_value(&value), "test-id");
    }

    #[test]
    fn test_extract_id_from_colon_string() {
        let value = serde_json::json!("workflow:test-id");
        assert_eq!(extract_id_from_value(&value), "test-id");
    }

    #[test]
    fn test_extract_id_from_nested_object() {
        let value = serde_json::json!({"id": "test-id"});
        assert_eq!(extract_id_from_value(&value), "test-id");
    }
}
