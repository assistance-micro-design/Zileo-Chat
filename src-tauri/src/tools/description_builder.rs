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

//! `ToolDescriptionBuilder` — typed builder for the `description` field of
//! [`ToolDefinition`](crate::tools::ToolDefinition).
//!
//! Enforces the project convention documented in `tools/mod.rs`:
//! 1. Summary line
//! 2. `USE THIS TOOL WHEN:`
//! 3. `DO NOT USE THIS TOOL WHEN:` (optional)
//! 4. `OPERATIONS:`
//! 5. Optional notes
//! 6. `EXAMPLES:` (optional)
//! 7. `PRIMARY AGENT ONLY:` block (optional, sub-agent tools only)
//!
//! `summary`, `use_when`, and `operations` are mandatory. `build()` panics
//! when any of them is empty so misconfiguration surfaces at the first
//! `Lazy::force` (i.e. agent boot) rather than silently producing a
//! malformed prompt at runtime.

use serde_json::Value;

/// Operations section content.
///
/// Most tools enumerate `(name, description)` pairs (`List`). The
/// `CalculatorTool` groups its 40+ operations into a free-form text block
/// (`Raw`).
enum OperationsSection {
    List(Vec<(String, String)>),
    Raw(String),
}

/// Builder for the structured `description` of a tool definition.
pub struct ToolDescriptionBuilder {
    summary: String,
    use_when: Vec<String>,
    do_not_use: Vec<String>,
    operations: Option<OperationsSection>,
    notes: Vec<String>,
    examples: Vec<Value>,
    primary_agent_max: Option<usize>,
}

impl ToolDescriptionBuilder {
    /// Creates a new builder with the given summary line.
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            use_when: Vec::new(),
            do_not_use: Vec::new(),
            operations: None,
            notes: Vec::new(),
            examples: Vec::new(),
            primary_agent_max: None,
        }
    }

    /// Adds the `USE THIS TOOL WHEN` bullets (mandatory section).
    pub fn use_when(mut self, items: &[&str]) -> Self {
        self.use_when = items.iter().map(|s| (*s).to_string()).collect();
        self
    }

    /// Adds the `DO NOT USE THIS TOOL WHEN` bullets (optional).
    pub fn do_not_use(mut self, items: &[&str]) -> Self {
        self.do_not_use = items.iter().map(|s| (*s).to_string()).collect();
        self
    }

    /// Defines `OPERATIONS:` as a list of `(name, description)` pairs.
    pub fn operations(mut self, ops: &[(&str, &str)]) -> Self {
        self.operations = Some(OperationsSection::List(
            ops.iter()
                .map(|(n, d)| ((*n).to_string(), (*d).to_string()))
                .collect(),
        ));
        self
    }

    /// Defines `OPERATIONS:` as a free-form block.
    ///
    /// Use this for tools whose operations don't fit a flat
    /// `(name, description)` list (e.g. `CalculatorTool`'s grouped ops).
    pub fn operations_raw(mut self, raw: impl Into<String>) -> Self {
        self.operations = Some(OperationsSection::Raw(raw.into()));
        self
    }

    /// Adds a free-form note inserted between `OPERATIONS:` and `EXAMPLES:`.
    ///
    /// Multiple notes are appended in call order, separated by blank lines.
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Sets the `EXAMPLES:` JSON entries (optional).
    pub fn examples(mut self, examples: &[Value]) -> Self {
        self.examples = examples.to_vec();
        self
    }

    /// Appends the `PRIMARY AGENT ONLY` block at the end of the description.
    ///
    /// `max` is the maximum number of sub-agent operations per workflow.
    pub fn primary_agent_constraint(mut self, max: usize) -> Self {
        self.primary_agent_max = Some(max);
        self
    }

    /// Builds the final description string.
    ///
    /// # Panics
    ///
    /// Panics when any mandatory section is missing or empty:
    /// - `summary` (passed to `new`) is empty
    /// - `use_when` is empty
    /// - `operations` is unset or contains an empty list / raw block
    pub fn build(self) -> String {
        assert!(
            !self.summary.trim().is_empty(),
            "ToolDescriptionBuilder: summary cannot be empty"
        );
        assert!(
            !self.use_when.is_empty(),
            "ToolDescriptionBuilder: use_when cannot be empty"
        );

        let operations = self
            .operations
            .as_ref()
            .expect("ToolDescriptionBuilder: operations cannot be empty");
        match operations {
            OperationsSection::List(items) => assert!(
                !items.is_empty(),
                "ToolDescriptionBuilder: operations cannot be empty"
            ),
            OperationsSection::Raw(text) => assert!(
                !text.trim().is_empty(),
                "ToolDescriptionBuilder: operations cannot be empty"
            ),
        }

        let mut out = String::new();
        out.push_str(self.summary.trim_end());
        out.push_str("\n\n");

        out.push_str("USE THIS TOOL WHEN:\n");
        for item in &self.use_when {
            out.push_str("- ");
            out.push_str(item);
            out.push('\n');
        }

        if !self.do_not_use.is_empty() {
            out.push('\n');
            out.push_str("DO NOT USE THIS TOOL WHEN:\n");
            for item in &self.do_not_use {
                out.push_str("- ");
                out.push_str(item);
                out.push('\n');
            }
        }

        out.push('\n');
        out.push_str("OPERATIONS:\n");
        match operations {
            OperationsSection::List(items) => {
                for (name, desc) in items {
                    out.push_str("- ");
                    out.push_str(name);
                    out.push_str(": ");
                    out.push_str(desc);
                    out.push('\n');
                }
            }
            OperationsSection::Raw(text) => {
                out.push_str(text.trim_end());
                out.push('\n');
            }
        }

        for note in &self.notes {
            out.push('\n');
            out.push_str(note.trim_end());
            out.push('\n');
        }

        if !self.examples.is_empty() {
            out.push('\n');
            out.push_str("EXAMPLES:\n");
            for (idx, example) in self.examples.iter().enumerate() {
                let json = serde_json::to_string(example).unwrap_or_else(|_| "{}".to_string());
                out.push_str(&format!("{}. {}", idx + 1, json));
                if idx + 1 < self.examples.len() {
                    out.push('\n');
                }
            }
        }

        if let Some(max) = self.primary_agent_max {
            out.push_str("\n\n");
            out.push_str("PRIMARY AGENT ONLY:\n");
            out.push_str("- Only the primary/root agent can use this tool (max depth: 1)\n");
            out.push_str(&format!(
                "- Maximum {} sub-agent operations per workflow",
                max
            ));
        }

        // Trim trailing newline if no constraint and no examples added the final flourish.
        while out.ends_with('\n') {
            out.pop();
        }

        out
    }
}

#[cfg(test)]
#[path = "description_builder_tests.rs"]
mod tests;
