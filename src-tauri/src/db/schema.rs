pub const SCHEMA_SQL: &str = r#"
-- Namespace et Database
DEFINE NAMESPACE zileo;
USE NS zileo;
DEFINE DATABASE chat;
USE DB chat;

-- Table: workflow
DEFINE TABLE workflow SCHEMAFULL;
DEFINE FIELD id ON workflow TYPE string;
DEFINE FIELD name ON workflow TYPE string;
DEFINE FIELD agent_id ON workflow TYPE string;
DEFINE FIELD status ON workflow TYPE string ASSERT $value IN ['idle', 'running', 'completed', 'error'];
DEFINE FIELD created_at ON workflow TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON workflow TYPE datetime DEFAULT time::now();
DEFINE FIELD completed_at ON workflow TYPE option<datetime>;

-- Table: agent_state
DEFINE TABLE agent_state SCHEMAFULL;
DEFINE FIELD agent_id ON agent_state TYPE string;
DEFINE FIELD lifecycle ON agent_state TYPE string ASSERT $value IN ['permanent', 'temporary'];
DEFINE FIELD config ON agent_state TYPE object;
DEFINE FIELD metrics ON agent_state TYPE object;
DEFINE FIELD last_active ON agent_state TYPE datetime DEFAULT time::now();
DEFINE INDEX unique_agent_id ON agent_state FIELDS agent_id UNIQUE;

-- Table: message
DEFINE TABLE message SCHEMAFULL;
DEFINE FIELD id ON message TYPE string;
DEFINE FIELD workflow_id ON message TYPE string;
DEFINE FIELD role ON message TYPE string ASSERT $value IN ['user', 'assistant', 'system'];
DEFINE FIELD content ON message TYPE string;
DEFINE FIELD tokens ON message TYPE int;
DEFINE FIELD timestamp ON message TYPE datetime DEFAULT time::now();

-- Table: memory (vectoriel)
DEFINE TABLE memory SCHEMAFULL;
DEFINE FIELD id ON memory TYPE string;
DEFINE FIELD type ON memory TYPE string ASSERT $value IN ['user_pref', 'context', 'knowledge', 'decision'];
DEFINE FIELD content ON memory TYPE string;
DEFINE FIELD embedding ON memory TYPE array<float>;
DEFINE FIELD metadata ON memory TYPE object;
DEFINE FIELD created_at ON memory TYPE datetime DEFAULT time::now();

-- Index HNSW pour vector search (1536D OpenAI/Mistral embeddings)
DEFINE INDEX memory_vec_idx ON memory FIELDS embedding HNSW DIMENSION 1536 DIST COSINE;

-- Table: validation_request
DEFINE TABLE validation_request SCHEMAFULL;
DEFINE FIELD id ON validation_request TYPE string;
DEFINE FIELD workflow_id ON validation_request TYPE string;
DEFINE FIELD type ON validation_request TYPE string ASSERT $value IN ['tool', 'sub_agent', 'mcp', 'file_op', 'db_op'];
DEFINE FIELD operation ON validation_request TYPE string;
DEFINE FIELD details ON validation_request TYPE object;
DEFINE FIELD risk_level ON validation_request TYPE string ASSERT $value IN ['low', 'medium', 'high'];
DEFINE FIELD status ON validation_request TYPE string DEFAULT 'pending' ASSERT $value IN ['pending', 'approved', 'rejected'];
DEFINE FIELD created_at ON validation_request TYPE datetime DEFAULT time::now();

-- Table: task (décomposition workflows)
DEFINE TABLE task SCHEMAFULL;
DEFINE FIELD id ON task TYPE string;
DEFINE FIELD workflow_id ON task TYPE string;
DEFINE FIELD description ON task TYPE string;
DEFINE FIELD status ON task TYPE string DEFAULT 'pending' ASSERT $value IN ['pending', 'in_progress', 'completed', 'blocked'];
DEFINE FIELD dependencies ON task TYPE array<string>;
DEFINE FIELD created_at ON task TYPE datetime DEFAULT time::now();
DEFINE FIELD completed_at ON task TYPE option<datetime>;

-- Relations graph
DEFINE TABLE workflow_agent SCHEMAFULL;
DEFINE FIELD in ON workflow_agent TYPE record<workflow>;
DEFINE FIELD out ON workflow_agent TYPE record<agent_state>;
DEFINE FIELD created_by ON workflow_agent TYPE bool DEFAULT true;
"#;
