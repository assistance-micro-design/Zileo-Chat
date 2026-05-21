// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

/** Mirror of `src-tauri/src/models/prompt_version.rs`. */

export interface PromptVersion {
	id: string;
	prompt_id: string;
	version: number;
	name: string;
	description: string;
	category: string;
	content: string;
	/** JSON-stringified `PromptVariable[]`. */
	variables_json: string;
	edited_by: string;
	edit_summary?: string;
	edited_at: string;
}

export interface PromptVersionSummary {
	id: string;
	prompt_id: string;
	version: number;
	edited_by: string;
	edit_summary?: string;
	edited_at: string;
}
