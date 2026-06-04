import { describe, expect, it } from 'vitest';
import type { McpToolAllowlistEntry } from '$types/agent';
import type { MCPServerStatus } from '$types/mcp';
import {
	areAllToolsArmed,
	buildMcpToolAllowlist,
	filterEditableMcpServers,
	mergeServerTools,
	preservedMcpAllowlistEntries,
	removePreservedEntry,
	seedMcpArmingState,
	toggleAllServerTools,
	type McpArmingState
} from '../mcp-allowlist.helpers';

function entry(
	server_id: string,
	tools: string[],
	allow_in_delegated_runs = false
): McpToolAllowlistEntry {
	return { server_id, tools, allow_in_delegated_runs };
}

describe('buildMcpToolAllowlist (payload from arming state)', () => {
	it('builds entries with sorted tools and the per-server delegation flag', () => {
		const armed: McpArmingState = {
			'srv-1': { tools: ['read', 'list'], allowInDelegatedRuns: true }
		};
		const out = buildMcpToolAllowlist(armed, ['srv-1'], []);
		expect(out).toEqual([entry('srv-1', ['list', 'read'], true)]);
	});

	it('returns [] when every tool is unchecked (explicit disarm, not undefined)', () => {
		const armed: McpArmingState = {
			'srv-1': { tools: [], allowInDelegatedRuns: false }
		};
		// Existing had an armed tool; clearing it must yield [] (running server,
		// nothing checked → pruned, and no stopped entry to preserve).
		const out = buildMcpToolAllowlist(armed, ['srv-1'], [entry('srv-1', ['read'])]);
		expect(out).toEqual([]);
	});

	it('PRESERVES an entry whose server is not enumerable (stopped/absent)', () => {
		// Piège 2 (the worst miss): a stopped server has no enumerable tools, so
		// its existing entry must NOT be silently dropped.
		const armed: McpArmingState = {
			'srv-running': { tools: ['ok'], allowInDelegatedRuns: false }
		};
		const out = buildMcpToolAllowlist(
			armed,
			['srv-running'],
			[entry('srv-stopped', ['exec'], true)]
		);
		expect(out).toContainEqual(entry('srv-stopped', ['exec'], true));
		expect(out).toContainEqual(entry('srv-running', ['ok'], false));
		expect(out).toHaveLength(2);
	});

	it('prunes an empty running-server entry but keeps a stopped-server entry', () => {
		const armed: McpArmingState = {
			'srv-a': { tools: [], allowInDelegatedRuns: false } // running, nothing checked
		};
		const out = buildMcpToolAllowlist(
			armed,
			['srv-a', 'srv-b'],
			[entry('srv-stopped', ['z'], false)]
		);
		expect(out).toEqual([entry('srv-stopped', ['z'], false)]);
	});

	it('round-trips an existing allowlist unchanged when seeded then rebuilt (anti-disarm)', () => {
		// Point 6: open + save WITHOUT touching the section must reproduce the
		// existing allowlist identically. The component seeds the arming state
		// from the existing entries of enumerable servers; rebuilding from that
		// seed plus the preserved (stopped) entries must equal the input set.
		const enumerable = ['srv-1', 'srv-2'];
		const existing = [
			entry('srv-1', ['read', 'list'], true),
			entry('srv-2', ['exec'], false),
			entry('srv-stopped', ['danger'], true) // not enumerable → preserved
		];
		const seeded = seedMcpArmingState(existing, enumerable);
		const rebuilt = buildMcpToolAllowlist(seeded, enumerable, existing);

		// Same set of servers, same tools (as a set) and flags — no disarm.
		expect(new Set(rebuilt.map((e) => e.server_id))).toEqual(
			new Set(['srv-1', 'srv-2', 'srv-stopped'])
		);
		for (const original of existing) {
			const got = rebuilt.find((e) => e.server_id === original.server_id);
			expect(got).toBeDefined();
			expect([...(got?.tools ?? [])].sort()).toEqual([...original.tools].sort());
			expect(got?.allow_in_delegated_runs).toBe(original.allow_in_delegated_runs);
		}
	});
});

describe('seedMcpArmingState / preservedMcpAllowlistEntries', () => {
	it('seeds editable arming only for enumerable servers (pre-checked tools + flag)', () => {
		const existing = [entry('srv-1', ['read'], true), entry('srv-stopped', ['x'], false)];
		const seeded = seedMcpArmingState(existing, ['srv-1']);
		expect(seeded).toEqual({ 'srv-1': { tools: ['read'], allowInDelegatedRuns: true } });
		// The stopped server is NOT seeded into the editable state.
		expect('srv-stopped' in seeded).toBe(false);
	});

	it('returns the non-enumerable (stopped/absent) entries to preserve read-only', () => {
		const existing = [entry('srv-1', ['read'], true), entry('srv-stopped', ['x'], false)];
		expect(preservedMcpAllowlistEntries(existing, ['srv-1'])).toEqual([
			entry('srv-stopped', ['x'], false)
		]);
		// A legacy agent with no allowlist preserves nothing.
		expect(preservedMcpAllowlistEntries([], ['srv-1'])).toEqual([]);
	});
});

describe('filterEditableMcpServers', () => {
	function srv(name: string, status: MCPServerStatus, id = `${name}-id`) {
		return { id, name, status };
	}

	it('keeps only RUNNING servers ASSIGNED to the agent (intersection by name)', () => {
		const servers = [
			srv('alpha', 'running'),
			srv('beta', 'running'),
			srv('gamma', 'stopped'), // assigned but not running -> excluded (preserved instead)
			srv('delta', 'running') // running but not assigned -> excluded (preserved instead)
		];
		const out = filterEditableMcpServers(servers, ['alpha', 'beta', 'gamma']);
		expect(out.map((s) => s.name)).toEqual(['alpha', 'beta']);
		// The immutable id is preserved on the returned objects (used for enumerableIds).
		expect(out.map((s) => s.id)).toEqual(['alpha-id', 'beta-id']);
	});

	it('returns [] when the agent has no assigned MCP servers', () => {
		expect(filterEditableMcpServers([srv('alpha', 'running')], [])).toEqual([]);
	});

	it('returns [] when none of the assigned servers are running', () => {
		const servers = [srv('alpha', 'stopped'), srv('beta', 'error')];
		expect(filterEditableMcpServers(servers, ['alpha', 'beta'])).toEqual([]);
	});
});

describe('removePreservedEntry (explicit revocation)', () => {
	it('removes ONLY the entry for the given server_id, keeping all others intact', () => {
		const value = [entry('a', ['x']), entry('b', ['y'], true), entry('c', ['z'])];
		expect(removePreservedEntry(value, 'b')).toEqual([entry('a', ['x']), entry('c', ['z'])]);
	});

	it('returns an equivalent list when the server_id is absent (no silent change)', () => {
		const value = [entry('a', ['x']), entry('c', ['z'])];
		expect(removePreservedEntry(value, 'zzz')).toEqual(value);
	});
});

describe('mergeServerTools (orphan transparency)', () => {
	it('lists exposed tools first (orphan:false), then armed-but-not-exposed (orphan:true)', () => {
		expect(mergeServerTools(['read', 'list'], ['read', 'gone'])).toEqual([
			{ name: 'read', orphan: false },
			{ name: 'list', orphan: false },
			{ name: 'gone', orphan: true }
		]);
	});

	it('marks no orphan when every armed tool is still exposed', () => {
		expect(mergeServerTools(['a', 'b'], ['a'])).toEqual([
			{ name: 'a', orphan: false },
			{ name: 'b', orphan: false }
		]);
	});

	it('returns only exposed tools when nothing is armed', () => {
		expect(mergeServerTools(['a'], [])).toEqual([{ name: 'a', orphan: false }]);
	});
});

describe('areAllToolsArmed / toggleAllServerTools (select all-or-none)', () => {
	it('reports all-armed only when every displayed tool is armed', () => {
		expect(areAllToolsArmed(['a', 'b'], ['a', 'b'])).toBe(true);
		expect(areAllToolsArmed(['a', 'b'], ['a'])).toBe(false);
		expect(areAllToolsArmed(['a', 'b'], [])).toBe(false);
	});

	it('treats an empty displayed list as NOT all-armed (toggle stays an arm-all)', () => {
		expect(areAllToolsArmed([], [])).toBe(false);
		expect(areAllToolsArmed([], ['orphan'])).toBe(false);
	});

	it('arms every displayed tool when not all are armed yet', () => {
		expect(toggleAllServerTools(['a', 'b', 'c'], ['a']).sort()).toEqual(['a', 'b', 'c']);
	});

	it('keeps armed orphans when arming all (union, never drops them)', () => {
		// "gone" is armed but no longer displayed; arming all must preserve it.
		expect(toggleAllServerTools(['a', 'b'], ['gone']).sort()).toEqual(['a', 'b', 'gone']);
	});

	it('clears the whole selection when all displayed tools are already armed', () => {
		expect(toggleAllServerTools(['a', 'b'], ['a', 'b'])).toEqual([]);
	});
});
