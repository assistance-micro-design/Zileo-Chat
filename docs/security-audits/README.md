# Security Audits

This directory contains the findings from security audits conducted on Zileo-Chat-3.

Each audit is run in a dedicated session for focused attention and thoroughness.

## Audit Index

| ID | Audit | Date | Findings (verified) | Remediation |
|----|-------|------|---------------------|-------------|
| SA-001 | [SurrealQL Injection](./SA-001-surrealql-injection.md) | 2026-02-19 | **3C**, 9H, 5M, 18L | **3C DONE, 9H DONE, 5M DONE**, 18L deferred |
| SA-002 | [MCP + Import + XSS + Secrets](./SA-002-mcp-import-xss-secrets.md) | 2026-02-19 | **0C**, 3H, 6M, 2L | **3H DONE, 4M DONE**, 1M open, 2L deferred |
| ~~SA-003~~ | ~~MCP Input Sanitization~~ | - | Merged into SA-002 | - |
| ~~SA-004~~ | ~~Secret Management / Logging~~ | - | Merged into SA-002 | - |
| SA-005 | [CSP / Tauri Permissions](./SA-005-csp-tauri-permissions.md) | 2026-02-19 | **0C**, 4H, 4M, 3L | **4H DONE, 2M DONE**, 2M open, 3L deferred |
| SA-006 | [Dependency Vulnerabilities](./SA-006-dependency-vulnerabilities.md) | 2026-02-19 | 3H, 4M, 5L, 2I (7 CVEs N/A) | **2H DONE** (DEP-1 rig-core + DEP-2 SurrealDB), 1H open (DEP-3 NPM) |
| SA-007 | [Commands Control Flow](./SA-007-commands-control-flow.md) | 2026-02-19 | Quality audit | **ALL DONE** (DUP-1/2/3 dedup, F1-F13 decomposition, F14 error messages) |
| SA-008 | [Agent System Quality](./SA-008-agent-system-quality.md) | 2026-02-19 | Quality audit | **PERF-1 DONE, PERF-2 DONE**, duplication deferred |
| SA-009 | [Svelte Stores Quality](./SA-009-stores-quality-audit.md) | 2026-02-19 | Quality audit | **F1/F2/F4 DONE, dead code DONE** |
| SA-010 | [Settings & Forms Quality](./SA-010-settings-forms-quality.md) | 2026-02-19 | Quality audit | **Error handling DONE (ERR-1+ERR-2), 4 a11y DONE, DUP-1/2/3 DONE** (template dedup) |
| SA-011 | [Chat & Workflow Components](./SA-011-chat-workflow-components.md) | 2026-02-19 | 0C, 3H, 12M, 26L | **H-001 DONE, H-002 DONE, H-003 DONE** |
| SA-012 | [DB Layer & Migrations](./SA-012-db-layer-migrations.md) | 2026-02-19 | 4H, 2M, 5L | **ALL DONE** (4H, 2M, 5L) |
| SA-013 | [Tools + Types Coherence](./SA-013-types-tools-coherence.md) | 2026-02-19 | **1C**, 4H, 7M, 2L | **1C DONE, 4H DONE, 2M DONE** (#14-15 ChunkType alignment + #16-20 console.*) |
| SA-014 | [Data Persistence & Restart](./SA-014-data-persistence-restart.md) | 2026-02-20 | 0C, 3H, 4M, 6L | **3H DONE, 4M DONE** (1M documented), 6L (4 DONE, 2 documented) |
| SA-015 | [Dead Code Cleanup](./SA-015-dead-code-cleanup.md) | 2026-02-21 | Quality audit | **ALL PHASES DONE**: 22 items deleted, 6 tests migrated, 5 tests deleted. 171 remaining annotations all verified legitimate. |
| SA-016 | [Agent Page UX Remediation](./SA-016-agent-page-ux-remediation.md) | 2026-02-22 | UX audit | **7 PHASES DONE**, Phase 8 out of scope |
| SA-017 | [Settings Page Optimization](./SA-017-settings-page-optimization.md) | 2026-02-22 | Quality audit | **ALL 5 PHASES DONE** (PERF-1-5, OPT-1-10) |
| SA-018 | [Hardcoded Elements Audit](./SA-018-hardcoded-elements-audit.md) | 2026-02-23 | Quality audit | **ALL 3 PHASES DONE** |
| SA-019 | [Agent Chat Refactoring](./SA-019-agent-chat-refactoring.md) | 2026-02-23 | Quality audit | **ALL 6 PHASES DONE** |
| SA-020 | [Agent Name Resolution](./SA-020-agent-name-resolution.md) | 2026-02-25 | Quality audit | **ALL 7 PHASES DONE** |
| SA-021 | [Report Enforcement](./SA-021-report-enforcement.md) | 2026-02-25 | Quality audit | **DONE**: report enforcement mechanism for agents without reports |
| SA-022 | [Frontend Structure & Naming](./SA-022-frontend-structure-naming.md) | 2026-02-26 | Quality audit | **ALL 7 PHASES DONE**: naming normalization, dead code removal, barrel exports, modal consolidation, provider components |
| SA-023 | [Backend Structure & Naming](./SA-023-backend-structure-naming.md) | 2026-02-26 | Quality audit | **P1 DONE** (H1 ProviderType consolidated). P2-P4 pending. |

**Evaluation report**: [EVALUATION-2026-02-19.md](./EVALUATION-2026-02-19.md)
**Remediation status**: [REMEDIATION-STATUS.md](./REMEDIATION-STATUS.md) (2026-02-20, branch `security/audit-remediation-tdd`)

## Severity Definitions

| Level | Definition |
|-------|------------|
| **CRITICAL** | Direct exploitation possible from user/external input. Fix immediately. |
| **HIGH** | Known anti-pattern with exploitable potential. Fix before next release. |
| **MEDIUM** | Internal data in sensitive positions. Defense-in-depth recommended. |
| **LOW** | Validated data, no realistic exploit path. Fix during normal maintenance. |

## Cross-Session Totals (After Verification)

**Security audits only** (SA-001, SA-002, SA-005, SA-006, SA-012, SA-013):

| Severity | Original | Verified | Delta |
|----------|----------|----------|-------|
| CRITICAL | 10 | **4** | -6 |
| HIGH | 27 | **27** | 0 (reshuffled) |
| MEDIUM | 30 | **34** | +4 |
| LOW | 13 | 13 | 0 |
| INFO | 2 | 2 | 0 |

**Quality audits** (SA-007, SA-008, SA-009, SA-010, SA-011): Confirmed as correctly categorized. No security severity ratings changed.

## Action Plan

The cross-session analysis and prioritized remediation plan is in **[ACTION-PLAN.md](./ACTION-PLAN.md)**.

| Priority | Items | Est. Hours | Impact |
|----------|-------|------------|--------|
| **P0** | 5 groups | ~10h | Eliminates injection + import pipeline security |
| **P1** | 8 groups | ~18h | Fixes type crashes, races, perf, CVE patches, CSP |
| **P2** | 9 groups | ~29h | -700 lines duplication, permissions hardening, feature bloat |
| **P3** | 6 groups | ~11h | -720 lines templates, a11y, CSP docs, dep planning |

Each P0 and P1 item includes a ready-to-run `/Fix_Zileo` or `/Build_zileo` prompt.

## CSP Policy Notes

The Content Security Policy is defined in `src-tauri/tauri.conf.json`:

| Directive | Value | Justification |
|-----------|-------|---------------|
| `default-src` | `'self' blob:` | `blob:` required for `URL.createObjectURL()` in memory/export downloads |
| `style-src` | `'self' 'unsafe-inline'` | `unsafe-inline` required by SvelteKit 5 scoped CSS injection at runtime. Cannot be removed without build-time CSS extraction. |
| `script-src` | `'self'` | No inline scripts needed |
| `frame-ancestors` | `'none'` | Prevents framing (clickjacking protection) |
| `object-src` | `'none'` | Blocks Flash/Java plugins |

## Workflow

1. Run audit in dedicated Claude Code session
2. Document findings in `SA-XXX-{name}.md`
3. Update this index
4. **Verify findings against real code** (added 2026-02-19)
5. Implement fixes (can be batched across audits)
6. Re-audit to verify fixes
