/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 */

/**
 * @fileoverview Pure detection of the configuration state of a global Kanban
 * supervisor role (compose / analyze).
 *
 * The "effective agent" resolution lives in the backend
 * (`resolve_role_agent_id`); the frontend only needs to know which of three
 * advisory states a configured id is in, so it can show the right nudge:
 *   - `unset`    — no agent configured (a VALID state: the flow falls back to
 *                  the card's own agent). Surfaced as an info notice in the
 *                  creation modal only.
 *   - `dangling` — an id IS configured but no longer matches an existing
 *                  Kanban-kind agent (deleted or demoted). Surfaced as a warning
 *                  in the modal AND a banner on the board, because it silently
 *                  breaks re-analyze / boot catch-up for cards already in review.
 *   - `ok`       — the configured id matches a live Kanban-kind agent.
 */

/** Configuration state of a single supervisor role. */
export type SupervisorRoleState = 'unset' | 'dangling' | 'ok';

/**
 * Classifies a configured supervisor id against the set of live Kanban-kind
 * agent ids.
 *
 * @param configuredId - The id stored in settings (`composeAgentId` /
 *   `analyzeAgentId`), or `undefined`/`null`/empty when none is set.
 * @param kanbanAgentIds - Ids of the agents currently known to be `kind=kanban`.
 * @returns The advisory state for this role.
 */
export function supervisorRoleState(
	configuredId: string | null | undefined,
	kanbanAgentIds: ReadonlySet<string>
): SupervisorRoleState {
	const trimmed = configuredId?.trim() ?? '';
	if (!trimmed) {
		return 'unset';
	}
	return kanbanAgentIds.has(trimmed) ? 'ok' : 'dangling';
}
