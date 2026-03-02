/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 *
 * TypeScript types for the FileManager tool.
 */

/** File operation types supported by FileManagerTool */
export type FileOperation =
	| 'list'
	| 'read'
	| 'write'
	| 'replace'
	| 'create'
	| 'delete'
	| 'move'
	| 'rename'
	| 'search_glob'
	| 'search_content';

/** Destructive operations that may require confirmation */
export const DESTRUCTIVE_OPS: FileOperation[] = ['write', 'replace', 'delete', 'move', 'rename'];

/** Entry in the trash directory */
export interface TrashEntry {
	trash_path: string;
	original_relative_path: string;
	deleted_at: string;
	size_bytes: number;
}

/** File info returned by list operation */
export interface FileInfo {
	name: string;
	path: string;
	is_dir: boolean;
	size: number;
	modified_at: string;
}
