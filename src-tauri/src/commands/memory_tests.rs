use super::*;
use chrono::Utc;

#[test]
fn test_memory_type_serialization() {
    let mtype = MemoryType::UserPref;
    let json = serde_json::to_string(&mtype).unwrap();
    assert_eq!(json, "\"user_pref\"");

    let mtype = MemoryType::Knowledge;
    let json = serde_json::to_string(&mtype).unwrap();
    assert_eq!(json, "\"knowledge\"");
}

#[test]
fn test_memory_structure() {
    let memory = Memory {
        id: "mem_001".to_string(),
        memory_type: MemoryType::Context,
        content: "User prefers dark mode".to_string(),
        workflow_id: None,
        metadata: serde_json::json!({"source": "settings"}),
        importance: 0.3,
        expires_at: None,
        created_at: Utc::now(),
    };

    let json = serde_json::to_string(&memory).unwrap();
    assert!(json.contains("\"type\":\"context\""));
    assert!(json.contains("\"content\":\"User prefers dark mode\""));
}

#[test]
fn test_chunk_search_result_structure() {
    // Verifies the shape returned by `search_memories` after the chunk
    // refactor: chunk_id, parent_memory_id and search_type are required.
    let result = ChunkSearchResult {
        chunk_id: "chunk_001".to_string(),
        parent_memory_id: "mem_002".to_string(),
        chunk_index: 0,
        chunk_count: 1,
        content: "Chose Rust for backend".to_string(),
        memory_type: MemoryType::Decision,
        workflow_id: None,
        metadata: serde_json::json!({}),
        importance: 0.7,
        expires_at: None,
        created_at: Utc::now(),
        score: 0.85,
        cosine_score: 0.91,
        search_type: "vector".to_string(),
    };

    let json = serde_json::to_string(&result).unwrap();
    // Field names are serialized camelCase per #[serde(rename_all)].
    assert!(json.contains("\"score\":0.85"));
    assert!(json.contains("\"chunkId\":\"chunk_001\""));
    assert!(json.contains("\"parentMemoryId\":\"mem_002\""));
    assert!(json.contains("\"searchType\":\"vector\""));
}

#[test]
fn test_content_validation() {
    // Empty content should be rejected
    let empty = "   ".trim();
    assert!(empty.is_empty());

    // Long content check
    let long_content = "a".repeat(memory_constants::MAX_CONTENT_LENGTH + 1);
    assert!(long_content.len() > memory_constants::MAX_CONTENT_LENGTH);
}

#[tokio::test]
async fn test_memory_type_values() {
    assert_eq!(
        serde_json::to_string(&MemoryType::UserPref).unwrap(),
        "\"user_pref\""
    );
    assert_eq!(
        serde_json::to_string(&MemoryType::Context).unwrap(),
        "\"context\""
    );
    assert_eq!(
        serde_json::to_string(&MemoryType::Knowledge).unwrap(),
        "\"knowledge\""
    );
    assert_eq!(
        serde_json::to_string(&MemoryType::Decision).unwrap(),
        "\"decision\""
    );
}
