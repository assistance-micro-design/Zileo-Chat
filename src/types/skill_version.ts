// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

/** Mirror of `src-tauri/src/models/skill_version.rs`. */

export interface SkillVersion {
	id: string;
	skill_id: string;
	version: number;
	name: string;
	description: string;
	category: string;
	content: string;
	edited_by: string;
	edit_summary?: string;
	edited_at: string;
}

export interface SkillVersionSummary {
	id: string;
	skill_id: string;
	version: number;
	edited_by: string;
	edit_summary?: string;
	edited_at: string;
}
