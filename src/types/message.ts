// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Message role in the conversation
 */
export type MessageRole = 'user' | 'assistant' | 'system';

/**
 * Message entity representing a conversation message
 */
export interface Message {
  /** Unique identifier */
  id: string;
  /** Associated workflow ID */
  workflow_id: string;
  /** Message role */
  role: MessageRole;
  /** Message content */
  content: string;
  /** Token count */
  tokens: number;
  /** Message timestamp */
  timestamp: Date;
}
