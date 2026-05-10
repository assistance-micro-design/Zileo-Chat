/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


/**
 * Block service for loading persisted execution blocks from the backend.
 *
 * @module services/block
 */

import { tauriInvoke as invoke } from '$lib/tauri';
import type { ChatBlock } from '$types/chat-block';
import type { Message } from '$types/message';

/**
 * Service for loading persisted ChatBlocks from the database.
 */
export const BlockService = {
	/**
	 * Load execution blocks for every assistant message of a workflow in a
	 * single Tauri round-trip via the batched `load_workflow_blocks` command.
	 *
	 * Falls back to an empty map when the workflow id cannot be derived (no
	 * messages or invalid input).
	 *
	 * @param messages - Array of messages of a single workflow
	 * @returns Map of message ID to ChatBlock array
	 */
	async loadForMessages(messages: Message[]): Promise<Map<string, ChatBlock[]>> {
		const result = new Map<string, ChatBlock[]>();
		if (messages.length === 0) return result;

		const workflowId = messages[0]?.workflow_id;
		if (!workflowId) return result;

		try {
			const grouped = await invoke<Record<string, ChatBlock[]>>('load_workflow_blocks', {
				workflowId
			});
			for (const [id, blocks] of Object.entries(grouped)) {
				if (blocks.length > 0) {
					result.set(id, blocks);
				}
			}
		} catch {
			// Non-blocking: caller renders the conversation without prior blocks.
		}

		return result;
	}
};
