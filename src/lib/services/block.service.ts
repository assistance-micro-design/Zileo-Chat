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

// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Block service for loading persisted execution blocks from the backend.
 *
 * @module services/block
 */

import { invoke } from '@tauri-apps/api/core';
import type { ChatBlock } from '$types/chat-block';
import type { Message } from '$types/message';

/**
 * Service for loading persisted ChatBlocks from the database.
 */
export class BlockService {
	/**
	 * Load execution blocks for a single message.
	 *
	 * @param messageId - The message ID to load blocks for
	 * @returns Array of ChatBlocks ordered by sequence
	 */
	static async loadForMessage(messageId: string): Promise<ChatBlock[]> {
		return invoke('load_message_blocks', { messageId });
	}

	/**
	 * Load execution blocks for all assistant messages in a list.
	 * Returns a Map of message_id to blocks array.
	 *
	 * @param messages - Array of messages to load blocks for
	 * @returns Map of message ID to ChatBlock array
	 */
	static async loadForMessages(messages: Message[]): Promise<Map<string, ChatBlock[]>> {
		const assistantMessages = messages.filter((m) => m.role === 'assistant');
		const result = new Map<string, ChatBlock[]>();

		// Load blocks for all assistant messages in parallel
		const entries = await Promise.all(
			assistantMessages.map(async (msg): Promise<[string, ChatBlock[]]> => {
				try {
					const blocks = await BlockService.loadForMessage(msg.id);
					return [msg.id, blocks];
				} catch {
					return [msg.id, []];
				}
			})
		);

		for (const [id, blocks] of entries) {
			if (blocks.length > 0) {
				result.set(id, blocks);
			}
		}

		return result;
	}
}
