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

//! Tests for the search-side helpers. The DB-touching paths
//! (`vector_search_core`, `text_search_core`, `search_memories_core`) are
//! exercised via integration tests against a live SurrealDB embedded
//! instance from `commands/memory_tests.rs`. Here we cover the pure tag
//! filter builder.

use super::build_tags_filter_clause;
use serde_json::json;

#[test]
fn build_tags_filter_clause_none_returns_none() {
    assert!(build_tags_filter_clause(None, "memory_id").is_none());
}

#[test]
fn build_tags_filter_clause_empty_slice_returns_none() {
    let empty: &[String] = &[];
    assert!(build_tags_filter_clause(Some(empty), "memory_id").is_none());
}

#[test]
fn build_tags_filter_clause_single_tag_returns_clause_and_bind() {
    let tags = vec!["backend".to_string()];
    let (clause, bind) = build_tags_filter_clause(Some(&tags), "memory_id").unwrap();
    assert_eq!(clause, "memory_id.metadata.tags CONTAINSANY $tags_filter");
    assert_eq!(bind.0, "tags_filter");
    assert_eq!(bind.1, json!(["backend"]));
}

#[test]
fn build_tags_filter_clause_multiple_tags_returns_array_bind() {
    let tags = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let (clause, bind) = build_tags_filter_clause(Some(&tags), "memory_id").unwrap();
    assert!(clause.contains("CONTAINSANY $tags_filter"));
    assert_eq!(bind.1, json!(["a", "b", "c"]));
}

#[test]
fn build_tags_filter_clause_uses_supplied_field_prefix() {
    // Legacy text path uses the bare `metadata.tags` field (no record link).
    let tags = vec!["x".to_string()];
    let (clause, _) = build_tags_filter_clause(Some(&tags), "memory").unwrap();
    assert!(clause.starts_with("memory.metadata.tags"));
}
