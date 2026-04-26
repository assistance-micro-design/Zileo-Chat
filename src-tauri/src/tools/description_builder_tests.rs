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

//! Tests for `ToolDescriptionBuilder`.

use super::ToolDescriptionBuilder;
use serde_json::json;

#[test]
fn test_build_complete_description() {
    let desc = ToolDescriptionBuilder::new("Manages tasks for workflow decomposition.")
        .use_when(&[
            "Breaking down a complex task into smaller steps",
            "Tracking progress across multiple steps",
        ])
        .do_not_use(&[
            "Single-step task (just do it directly)",
            "Tracking conversation state (use MemoryTool)",
        ])
        .operations(&[
            ("create", "Create a new task"),
            ("complete", "Mark task as completed"),
        ])
        .examples(&[
            json!({"operation": "create", "name": "Analyze schema"}),
            json!({"operation": "complete", "task_id": "uuid"}),
        ])
        .build();

    let expected = "Manages tasks for workflow decomposition.\n\
        \n\
        USE THIS TOOL WHEN:\n\
        - Breaking down a complex task into smaller steps\n\
        - Tracking progress across multiple steps\n\
        \n\
        DO NOT USE THIS TOOL WHEN:\n\
        - Single-step task (just do it directly)\n\
        - Tracking conversation state (use MemoryTool)\n\
        \n\
        OPERATIONS:\n\
        - create: Create a new task\n\
        - complete: Mark task as completed\n\
        \n\
        EXAMPLES:\n\
        1. {\"operation\":\"create\",\"name\":\"Analyze schema\"}\n\
        2. {\"operation\":\"complete\",\"task_id\":\"uuid\"}";

    assert_eq!(desc, expected);
}

#[test]
fn test_do_not_use_section_omitted_when_absent() {
    let desc = ToolDescriptionBuilder::new("Simple summary.")
        .use_when(&["Some condition"])
        .operations(&[("act", "Do action")])
        .examples(&[json!({"operation": "act"})])
        .build();

    assert!(desc.contains("USE THIS TOOL WHEN:"));
    assert!(!desc.contains("DO NOT USE THIS TOOL WHEN:"));
    assert!(desc.contains("OPERATIONS:"));
    assert!(desc.contains("EXAMPLES:"));
}

#[test]
fn test_operations_raw_branch() {
    let desc = ToolDescriptionBuilder::new("Calculator.")
        .use_when(&["Math needed"])
        .operations_raw(
            "- Unary (require \"value\"): sin, cos\n\
             - Binary (require \"a\" and \"b\"): add, subtract\n\
             - Constants (require \"name\"): pi, e",
        )
        .examples(&[json!({"operation": "sin", "value": 1.5})])
        .build();

    assert!(desc.contains("OPERATIONS:\n- Unary"));
    assert!(desc.contains("- Binary"));
    assert!(desc.contains("- Constants"));
    // operations_raw should NOT prepend "- " before each line (it's raw)
    assert!(!desc.contains("OPERATIONS:\n- - Unary"));
}

#[test]
fn test_primary_agent_constraint_appended() {
    let desc = ToolDescriptionBuilder::new("Spawn helper.")
        .use_when(&["Sub-agent needed"])
        .operations(&[("spawn", "Create sub-agent")])
        .examples(&[json!({"operation": "spawn"})])
        .primary_agent_constraint(15)
        .build();

    assert!(desc.contains("PRIMARY AGENT ONLY:"));
    assert!(desc.contains("Only the primary/root agent can use this tool (max depth: 1)"));
    assert!(desc.contains("Maximum 15 sub-agent operations per workflow"));
    // Must come at the end
    let constraint_pos = desc
        .find("PRIMARY AGENT ONLY:")
        .expect("constraint section");
    let examples_pos = desc.find("EXAMPLES:").expect("examples section");
    assert!(constraint_pos > examples_pos);
}

#[test]
fn test_note_inserted_between_operations_and_examples() {
    let desc = ToolDescriptionBuilder::new("Tool.")
        .use_when(&["Need it"])
        .operations(&[("op", "Operation")])
        .note("Note: extra context here.")
        .examples(&[json!({"operation": "op"})])
        .build();

    let ops_pos = desc.find("OPERATIONS:").expect("ops section");
    let note_pos = desc.find("Note: extra context here.").expect("note");
    let ex_pos = desc.find("EXAMPLES:").expect("examples section");
    assert!(ops_pos < note_pos);
    assert!(note_pos < ex_pos);
}

#[test]
fn test_examples_section_omitted_when_empty() {
    let desc = ToolDescriptionBuilder::new("No-example tool.")
        .use_when(&["Always"])
        .operations(&[("act", "Do")])
        .build();

    assert!(!desc.contains("EXAMPLES:"));
    assert!(desc.contains("OPERATIONS:"));
}

#[test]
#[should_panic(expected = "summary cannot be empty")]
fn test_panic_on_empty_summary() {
    ToolDescriptionBuilder::new("")
        .use_when(&["Has condition"])
        .operations(&[("op", "Do")])
        .build();
}

#[test]
#[should_panic(expected = "use_when cannot be empty")]
fn test_panic_on_missing_use_when() {
    ToolDescriptionBuilder::new("Summary.")
        .operations(&[("op", "Do")])
        .build();
}

#[test]
#[should_panic(expected = "operations cannot be empty")]
fn test_panic_on_missing_operations() {
    ToolDescriptionBuilder::new("Summary.")
        .use_when(&["Cond"])
        .build();
}

#[test]
fn test_multiple_examples_numbered_sequentially() {
    let desc = ToolDescriptionBuilder::new("Tool.")
        .use_when(&["Cond"])
        .operations(&[("op", "Do")])
        .examples(&[json!({"a": 1}), json!({"b": 2}), json!({"c": 3})])
        .build();

    assert!(desc.contains("\n1. {\"a\":1}"));
    assert!(desc.contains("\n2. {\"b\":2}"));
    assert!(desc.contains("\n3. {\"c\":3}"));
}

#[test]
fn test_multiple_notes_concatenated() {
    let desc = ToolDescriptionBuilder::new("Tool.")
        .use_when(&["Cond"])
        .operations(&[("op", "Do")])
        .note("First note.")
        .note("Second note.")
        .examples(&[json!({"x": 1})])
        .build();

    assert!(desc.contains("First note."));
    assert!(desc.contains("Second note."));
    let first_pos = desc.find("First note.").unwrap();
    let second_pos = desc.find("Second note.").unwrap();
    assert!(first_pos < second_pos);
}
