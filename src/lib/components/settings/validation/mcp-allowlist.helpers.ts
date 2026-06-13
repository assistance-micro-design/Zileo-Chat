import type { McpToolAllowlistEntry } from '$types/agent';
import type { MCPServerStatus } from '$types/mcp';

/**
 * Pure, framework-agnostic helpers that turn an editable per-server arming
 * state into the complete `mcp_tool_allowlist` payload (arm MCP
 * tools for UNATTENDED detached runs, with an optional per-server
 * `allow_in_delegated_runs` flag). Used by `AgentMcpAllowlist.svelte` on the
 * Settings → Validation authorization page (the AgentForm no longer owns the
 * allowlist). Labels live under the `validation_mcp_allowlist_*` i18n keys.
 */

/**
 * Editable arming state for ONE MCP server in the allowlist UI.
 *
 * - `tools`: tool names the user has armed (auto-approved in a detached run);
 * - `allowInDelegatedRuns`: per-server flag — when true, the armed tools
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
 * changes reproduce the allowlist identically (anti-involuntary-disarm):
 * the rebuilt payload equals the input.
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

/**
 * The MCP servers that are EDITABLE for an agent in the allowlist UI: those that
 * are currently RUNNING (tools listable) AND assigned to the agent
 * (`agent.mcp_servers`, which stores server NAMES — matched against
 * `MCPServer.name`). Everything else (stopped/removed servers, or running
 * servers de-selected from the agent) falls outside this set and is therefore
 * handled read-only by {@link preservedMcpAllowlistEntries} — its armed tools
 * are preserved verbatim, never silently disarmed (Piège 2).
 *
 * Generic over the server shape so callers keep their concrete `MCPServer` type
 * on the result (used by the component to derive `enumerableIds`).
 */
export function filterEditableMcpServers<T extends { name: string; status: MCPServerStatus }>(
	servers: T[],
	assignedNames: string[]
): T[] {
	const assigned = new Set(assignedNames);
	return servers.filter((s) => s.status === 'running' && assigned.has(s.name));
}

/**
 * Explicitly REVOKES a preserved (non-editable) server's entry: returns the
 * allowlist without the entry for `serverId`, leaving every OTHER entry intact.
 * This is the deliberate, user-initiated counterpart to the anti-silent-disarm
 * guarantee — only the explicitly removed server disappears.
 */
export function removePreservedEntry(
	value: McpToolAllowlistEntry[],
	serverId: string
): McpToolAllowlistEntry[] {
	return value.filter((e) => e.server_id !== serverId);
}

/**
 * Whether EVERY displayed tool of a server is currently auto-approved — drives
 * the disabled state of the per-server "arm all" header action. An empty
 * displayed list is treated as "not all armed" so the action never looks
 * satisfied with nothing to arm.
 */
export function areAllToolsArmed(displayedNames: string[], armedNames: string[]): boolean {
	if (displayedNames.length === 0) return false;
	const armed = new Set(armedNames);
	return displayedNames.every((n) => armed.has(n));
}

/** A tool row shown for an editable server: exposed, or an armed orphan. */
export interface DisplayedTool {
	name: string;
	/** True when the tool is armed but the server NO LONGER exposes it. */
	orphan: boolean;
}

/**
 * Merges a running server's currently-exposed tools with any ARMED tools the
 * server no longer exposes (orphans), so nothing armed stays hidden. Exposed
 * tools come first (orphan = false), then armed orphans (orphan = true). Both
 * remain toggleable in the UI, closing the "armed-but-invisible" transparency
 * gap.
 */
export function mergeServerTools(exposedNames: string[], armedNames: string[]): DisplayedTool[] {
	const exposedSet = new Set(exposedNames);
	const exposed: DisplayedTool[] = exposedNames.map((name) => ({ name, orphan: false }));
	const orphans: DisplayedTool[] = armedNames
		.filter((n) => !exposedSet.has(n))
		.map((name) => ({ name, orphan: true }));
	return [...exposed, ...orphans];
}
