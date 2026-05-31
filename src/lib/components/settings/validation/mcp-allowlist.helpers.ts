import type { McpToolAllowlistEntry } from '$types/agent';

/**
 * Pure, framework-agnostic helpers that turn an editable per-server arming
 * state into the complete `mcp_tool_allowlist` payload (R-SEC-4 / R1: arm MCP
 * tools for UNATTENDED detached runs, with an optional per-server
 * `allow_in_delegated_runs` flag). Used by `AgentMcpAllowlist.svelte` on the
 * Settings → Validation authorization page (the AgentForm no longer owns the
 * allowlist). Labels live under the `validation_mcp_allowlist_*` i18n keys.
 */

/**
 * Editable arming state for ONE MCP server in the allowlist UI.
 *
 * - `tools`: tool names the user has armed (auto-approved in a detached run);
 * - `allowInDelegatedRuns`: per-server flag (R1) — when true, the armed tools
 *   are also auto-approved when ANOTHER autonomous agent delegates to this one.
 */
export interface McpServerArming {
	tools: string[];
	allowInDelegatedRuns: boolean;
}

/** Arming state keyed by immutable MCP `server_id` (enumerable servers only). */
export type McpArmingState = Record<string, McpServerArming>;

/**
 * Seeds the editable arming state from an agent's existing allowlist, keeping
 * ONLY the entries whose server is currently enumerable (running, with listable
 * tools). Stopped/absent servers are handled separately by
 * {@link preservedMcpAllowlistEntries} (shown read-only) so the UI never
 * pretends to edit tools it cannot enumerate.
 *
 * Seeding from the existing allowlist is what makes an open+save WITHOUT
 * changes reproduce the allowlist identically (anti-involuntary-disarm, R1
 * point 6): the rebuilt payload equals the input.
 */
export function seedMcpArmingState(
	existing: McpToolAllowlistEntry[],
	enumerableServerIds: string[]
): McpArmingState {
	const enumerable = new Set(enumerableServerIds);
	const state: McpArmingState = {};
	for (const e of existing) {
		if (enumerable.has(e.server_id)) {
			state[e.server_id] = {
				tools: [...e.tools],
				allowInDelegatedRuns: e.allow_in_delegated_runs
			};
		}
	}
	return state;
}

/**
 * Returns the existing allowlist entries whose server is NOT enumerable
 * (stopped or deleted): these cannot be edited (their tools are not listable)
 * but MUST be preserved — dropping them would silently disarm tools the user
 * approved (Piège 2). The UI shows them read-only and they are re-emitted in
 * the payload verbatim by {@link buildMcpToolAllowlist}.
 */
export function preservedMcpAllowlistEntries(
	existing: McpToolAllowlistEntry[],
	enumerableServerIds: string[]
): McpToolAllowlistEntry[] {
	const enumerable = new Set(enumerableServerIds);
	return existing.filter((e) => !enumerable.has(e.server_id));
}

/**
 * Builds the complete `mcp_tool_allowlist` payload from the editable arming
 * state plus the preserved (non-enumerable) entries.
 *
 * - Enumerable servers: one entry per server with ≥1 armed tool (tools sorted
 *   for a stable shape); a server with nothing armed is PRUNED.
 * - Non-enumerable servers (stopped/absent): their existing entries are
 *   preserved verbatim — even if their tool list would look "empty" from the
 *   running set, they are never pruned (Piège 2).
 *
 * The result is always a complete array (possibly empty = explicit disarm),
 * matching the backend tri-state contract on `update_agent` (Piège 1).
 */
export function buildMcpToolAllowlist(
	armed: McpArmingState,
	enumerableServerIds: string[],
	existing: McpToolAllowlistEntry[]
): McpToolAllowlistEntry[] {
	const edited: McpToolAllowlistEntry[] = enumerableServerIds
		.map((serverId) => {
			const arming = armed[serverId];
			const tools = arming ? [...arming.tools].sort() : [];
			return {
				server_id: serverId,
				tools,
				allow_in_delegated_runs: arming?.allowInDelegatedRuns ?? false
			};
		})
		.filter((e) => e.tools.length > 0);

	return [...edited, ...preservedMcpAllowlistEntries(existing, enumerableServerIds)];
}
