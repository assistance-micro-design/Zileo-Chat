// Prompt Library Types
// Synchronized with src-tauri/src/models/prompt.rs

// ===== Core Types =====

export type PromptCategory = 'system' | 'user' | 'analysis' | 'generation' | 'coding' | 'custom';

export interface PromptVariable {
  name: string;           // Variable name (e.g., "user_name")
  description?: string;   // Optional description for form label
  defaultValue?: string;  // Optional default value
}

// Full entity (from backend)
export interface Prompt {
  id: string;
  name: string;              // max 128 chars, unique
  description: string;       // max 1000 chars
  category: PromptCategory;
  content: string;           // max 50000 chars, with {{variable}} placeholders
  variables: PromptVariable[]; // Auto-detected from content
  created_at: string;        // ISO 8601
  updated_at: string;        // ISO 8601
}

// For list display (lightweight)
export interface PromptSummary {
  id: string;
  name: string;
  description: string;
  category: PromptCategory;
  variables_count: number;
  updated_at: string;
}

// For creation (no id, no timestamps)
export interface PromptCreate {
  name: string;
  description: string;
  category: PromptCategory;
  content: string;
}

// For updates (all optional)
export interface PromptUpdate {
  name?: string;
  description?: string;
  category?: PromptCategory;
  content?: string;
}

// ===== Utility Types =====

export interface PromptPreviewParams {
  content: string;
  variables: Record<string, string>;
}

export interface PromptPreviewResult {
  rendered: string;
  missingVariables: string[];
}

// ===== Store State =====

export interface PromptStoreState {
  prompts: PromptSummary[];
  selectedId: string | null;
  loading: boolean;
  error: string | null;
  formMode: 'create' | 'edit' | null;
  editingPrompt: Prompt | null;
}

// ===== Category Labels (for UI) =====

export const PROMPT_CATEGORY_LABELS: Record<PromptCategory, string> = {
  system: 'System',
  user: 'User',
  analysis: 'Analysis',
  generation: 'Generation',
  coding: 'Coding',
  custom: 'Custom'
};
