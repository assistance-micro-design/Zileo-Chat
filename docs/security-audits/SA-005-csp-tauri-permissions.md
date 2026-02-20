# SA-005: CSP & Tauri Permissions Audit

**Date**: 2026-02-19
**Scope**: `tauri.conf.json`, `capabilities/default.json`, `main.rs`, all frontend imports of `@tauri-apps/*`
**Status**: Documented (fixes pending)
**Cross-references**: SA-001 (injection), SA-002 (XSS/secrets)

## Summary

| Severity | Count | Pattern |
|----------|-------|---------|
| CRITICAL | 1 | Arbitrary file read via registered but unused command |
| HIGH | 3 | Missing CSP directives, missing sanitization on import, unguarded migration |
| MEDIUM | 4 | Over-broad plugin permissions, no IPC deny patterns |
| LOW | 3 | Defense-in-depth improvements |

## Secure Areas (No Issues Found)

- **Reqwest TLS**: All 4 HTTP client instances use default rustls-tls with system certificate validation. No `danger_accept_invalid_certs`. Timeouts configured (30s-300s).
- **No backend event listeners**: Backend only emits events, never listens to frontend-emitted events. No attack surface via `core:event:allow-emit`.
- **No frontend HTTP calls**: Zero `fetch()` / `XMLHttpRequest` in frontend. All network I/O goes through Tauri IPC to Rust backend.
- **No Web Workers**: No `Worker()` or `SharedWorker()` usage. `worker-src` directive is unnecessary.
- **DOMPurify on markdown**: `MarkdownRenderer.svelte:45` sanitizes all LLM output with DOMPurify (default config) before `{@html}`. Link clicks intercepted with confirmation popup before `openUrl()`.
- **External links**: Static `<a target="_blank">` links use `rel="noopener noreferrer"` correctly.

---

## CRITICAL Findings

### C1 - read_import_file: arbitrary filesystem read

- **File**: `commands/import_export.rs:1276-1280`
- **Code**:
  ```rust
  pub async fn read_import_file(path: String) -> Result<String, String> {
      let path = PathBuf::from(&path);
      let content = std::fs::read_to_string(&path)
          .map_err(|e| format!("Failed to read file: {}", e))?;
      Ok(content)
  }
  ```
- **Source**: Frontend (arbitrary `path` string parameter)
- **Exploitability**: HIGH - registered in `generate_handler!`, callable from webview via `invoke('read_import_file', { path: '/etc/passwd' })`. No path validation, no directory confinement, no allowlist.
- **Current mitigation**: The frontend import wizard (`ImportPanel.svelte:139`) uses `FileReader` API (`file.text()`) instead of this command. The command is unused but exposed.
- **Risk**: If a future XSS or CSP bypass allows arbitrary `invoke()` calls, this command reads any file the process can access. Combined with `save_export_to_file` (same file, line 1251) which writes to arbitrary paths, this is a full read/write filesystem primitive.
- **Fix**: Either (a) remove the command entirely since it's unused, or (b) add path validation:
  ```rust
  // Option A: Remove from generate_handler! macro
  // Option B: Restrict to dialog-selected paths
  pub async fn read_import_file(path: String) -> Result<String, String> {
      let path = PathBuf::from(&path);
      let path = path.canonicalize()
          .map_err(|e| format!("Invalid path: {}", e))?;
      // Only allow files in user data directory or temp
      let allowed_prefixes = [dirs::data_dir(), dirs::download_dir(), std::env::temp_dir()];
      if !allowed_prefixes.iter().any(|p| path.starts_with(p)) {
          return Err("Path not in allowed directory".to_string());
      }
      std::fs::read_to_string(&path)
          .map_err(|e| format!("Failed to read file: {}", e))
  }
  ```

---

## HIGH Findings

### H1 - CSP missing directives for Google Fonts (functional + security gap)

- **File**: `tauri.conf.json:22` (CSP) and `routes/+layout.svelte:70-74` (font loading)
- **Current CSP**:
  ```
  default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline';
  frame-ancestors 'none'; object-src 'none'; base-uri 'self'; form-action 'self'
  ```
- **Issue**: The layout loads Google Fonts at runtime:
  ```html
  <link href="https://fonts.googleapis.com/css2?family=Signika:wght@400;500;600;700&family=JetBrains+Mono&display=swap" rel="stylesheet" />
  ```
  The CSP has no `font-src` or `style-src https://fonts.googleapis.com` directive. Under the current CSP, `default-src 'self'` blocks the font stylesheet fetch, and the fonts silently fail to load (or Tauri may bypass this depending on custom-protocol behavior).
- **Exploitability**: N/A - this is a CSP correctness gap, not directly exploitable. But it means either (a) fonts are broken and nobody noticed, or (b) Tauri's custom-protocol bypasses CSP enforcement for external resources, which would be a much larger concern.
- **Fix**: Either self-host the fonts (recommended for a desktop app) or add explicit CSP directives:
  ```
  style-src 'self' 'unsafe-inline' https://fonts.googleapis.com;
  font-src https://fonts.gstatic.com;
  ```

### H2 - execute_import: missing sanitize_for_surrealdb()

- **File**: `commands/import_export.rs:730+`
- **Issue**: `execute_import` inserts external data (from user-provided JSON files) into SurrealDB without calling `sanitize_for_surrealdb()`. Specific gaps:
  - Agent fields (`name`, `system_prompt`, etc.) are JSON-encoded but not null-byte stripped
  - `server.command` (line 951, 976) interpolated **without JSON encoding**: `command: '{}'`
  - `model.provider` (lines 1056, 1086) interpolated without JSON encoding
- **Source**: External file import (fully user-controlled content)
- **Exploitability**: MEDIUM-HIGH - requires user to import a crafted JSON file. Null bytes can crash SurrealDB. Unescaped strings in `command` and `provider` fields bypass the `ASSERT` constraint if schema migration hasn't been applied.
- **Cross-reference**: SA-001 C3 (same pattern in `import_memories`), `mcp/manager.rs:953` correctly uses `sanitize_for_surrealdb()`
- **Fix**: Apply `sanitize_for_surrealdb()` to all imported data before DB insertion. Use `$data` bind parameters instead of string interpolation for `command` and `provider` fields.

### H3 - migrate_memory_schema: embedding destruction without guard

- **File**: `commands/migration.rs:77`
- **Issue**: `migrate_memory_schema` contains `UPDATE memory SET embedding = NONE WHERE embedding IS NOT NONE` which destroys all computed embedding vectors. The command:
  - Has no "already applied" tracking (no migration version table)
  - Is registered in `generate_handler!` and callable from webview
  - Is idempotent for schema changes (`IF NOT EXISTS`) but **destructive for embeddings**
- **Exploitability**: LOW direct (requires malicious invoke call), HIGH impact (wipes all embeddings, requires expensive recomputation)
- **Fix**: Add a migration status check:
  ```rust
  let status: Option<Value> = db.query(
      "SELECT * FROM migration_log WHERE name = 'memory_schema_v1'"
  ).await?;
  if status.is_some() {
      return Ok("Migration already applied".to_string());
  }
  // ... run migration ...
  db.query("CREATE migration_log:memory_schema_v1 SET applied_at = time::now()").await?;
  ```

---

## MEDIUM Findings

### M1 - opener:default allows any http/https URL without scope

- **File**: `capabilities/default.json:11`
- **Issue**: `opener:default` grants `allow-open-url` + `allow-default-urls` which permits opening any `http://`, `https://`, `mailto:`, `tel:` URL via the OS default handler. No URL allowlist is configured. The `gen/schemas/desktop-schema.json` defines a scope mechanism for URL-level allow/deny lists, but it's unused.
- **Current usage**: Only `MarkdownRenderer.svelte:77` calls `openUrl()`, after user confirmation popup. URLs originate from LLM-generated markdown (DOMPurify-sanitized).
- **Risk**: If an attacker achieves invoke access (XSS/CSP bypass), they can open arbitrary URLs including potentially dangerous scheme handlers. In the current architecture, the user confirmation popup in MarkdownRenderer is a UI-level guard, not an IPC-level guard.
- **Fix**: Add URL scope restriction in capabilities:
  ```json
  {
    "identifier": "opener:allow-open-url",
    "allow": [
      { "url": "https://*" },
      { "url": "http://*" },
      { "url": "mailto:*" }
    ],
    "deny": [
      { "url": "file://*" },
      { "url": "tel:*" }
    ]
  }
  ```

### M2 - dialog:default grants all dialog types without path scope

- **File**: `capabilities/default.json:12`
- **Issue**: `dialog:default` enables all 5 dialog types (ask, confirm, message, save, open) with no path restrictions. The `open` dialog can browse any directory on the filesystem.
- **Current usage**: Only 2 files use dialog (`ActivitySidebar.svelte`, `ExportPanel.svelte`), both for `save()` only. No code uses the `open()` dialog for file browsing.
- **Risk**: The `open()` capability is unused but granted. If exploited, it enables browsing the full filesystem.
- **Fix**: Replace `dialog:default` with only the needed permissions:
  ```json
  "dialog:allow-save",
  "dialog:allow-message",
  "dialog:allow-confirm"
  ```

### M3 - No IPC deny patterns for sensitive commands

- **File**: `capabilities/default.json`, `main.rs:192-337`
- **Issue**: All 122 Tauri commands are implicitly allowed. Tauri 2 supports granular command permissions with deny patterns, but none are configured. Sensitive commands accessible from webview include:
  - **Destructive**: `clear_memories_by_type` (wipes ALL memories of a type globally), `clear_workflow_*` (5 commands)
  - **Migration**: `migrate_memory_schema`, `migrate_mcp_http_schema`, `migrate_memory_v2_schema`
  - **Credential access**: `get_api_key`, `delete_api_key`
  - **File I/O**: `read_import_file` (C1), `save_export_to_file`
  - **Resource-intensive**: `regenerate_embeddings` (rewrites all vectors)
- **Risk**: All commands are callable from any JavaScript in the webview. The CSP mitigates external script injection, but a successful XSS grants access to the full command surface.
- **Fix**: Define deny patterns for commands that should never be called from the webview without explicit UI flow, or create a restricted capability set. At minimum, consider removing unused commands like `read_import_file` from `generate_handler!`.

### M4 - window.open() bypasses opener plugin

- **File**: `components/onboarding/steps/StepImport.svelte:34`
- **Code**:
  ```typescript
  window.open(EXTERNAL_URL, '_blank', 'noopener,noreferrer');
  ```
- **Issue**: Uses `window.open()` directly instead of `openUrl()` from `@tauri-apps/plugin-opener`. In Tauri 2, `window.open()` behavior depends on `allowNavigation` in config (absent = blocked by default). If it works, it bypasses any URL scope restrictions configured on the opener plugin.
- **Fix**: Replace with `openUrl()` for consistent URL handling:
  ```typescript
  import { openUrl } from '@tauri-apps/plugin-opener';
  await openUrl('https://assistancemicrodesign.net/');
  ```

---

## LOW Findings

### L1 - 'unsafe-inline' in style-src is necessary but should be documented

- **File**: `tauri.conf.json:22`
- **Issue**: `style-src 'self' 'unsafe-inline'` is required because:
  - Svelte 5 injects scoped component styles as `<style>` tags at runtime
  - 7 components use inline `style=` attribute bindings (progress bars, spinners, popups)
  - `ChatInput.svelte:95-96` uses `.style.height` DOM manipulation
- **Risk**: `unsafe-inline` for styles is generally acceptable (no script execution), but it allows injected HTML to apply arbitrary CSS (data exfiltration via CSS selectors, UI redress). DOMPurify mitigates this for LLM content.
- **Fix**: No code change needed. Document the justification in `tauri.conf.json` as a comment in a companion file, since JSON doesn't support comments.

### L2 - blob: URI for memory export may be blocked by CSP

- **File**: `components/settings/memory/MemoryList.svelte:248`
- **Code**:
  ```typescript
  const url = URL.createObjectURL(blob);
  ```
- **Issue**: `URL.createObjectURL` creates a `blob:` URI. The current CSP's `default-src 'self'` may block `blob:` URIs for navigation/download. If the download fails silently, this is a functionality bug.
- **Fix**: Add `blob:` to the CSP if download functionality is needed:
  ```
  default-src 'self' blob:;
  ```
  Or replace with Tauri's `save_export_to_file` command flow (which uses the native dialog and avoids the blob URI entirely).

### L3 - CSP missing img-src directive

- **File**: `tauri.conf.json:22`
- **Issue**: No explicit `img-src` directive. Falls back to `default-src 'self'`. If LLM-generated markdown contains `<img>` tags, DOMPurify (default config) preserves them with their `src` attributes. External image URLs would be blocked by `default-src 'self'`, which is correct. However, `data:` URIs for images are also blocked, which may affect markdown rendering of inline images.
- **Risk**: LOW - the current behavior (blocking external images) is actually the secure default. Only add `img-src` if image rendering from markdown is a desired feature.
- **Fix**: No change needed unless image support is required. If so:
  ```
  img-src 'self' data:;
  ```

---

## Compliance Summary

### CSP Directives Analysis

| Directive | Current | Status | Action |
|-----------|---------|--------|--------|
| `default-src` | `'self'` | OK | Consider adding `blob:` (L2) |
| `script-src` | `'self'` | OK | Secure - no inline scripts |
| `style-src` | `'self' 'unsafe-inline'` | Acceptable | Required by Svelte (L1) |
| `frame-ancestors` | `'none'` | OK | Prevents embedding |
| `object-src` | `'none'` | OK | Blocks plugins |
| `base-uri` | `'self'` | OK | Prevents base tag injection |
| `form-action` | `'self'` | OK | Prevents form hijacking |
| `connect-src` | (absent) | OK | No frontend HTTP calls |
| `font-src` | (absent) | **GAP** | Google Fonts blocked (H1) |
| `img-src` | (absent) | OK | Falls back to secure default |
| `worker-src` | (absent) | OK | No workers used |

### Tauri Permissions Analysis

| Permission | Current | Needed | Action |
|------------|---------|--------|--------|
| `core:default` | Granted | Yes | OK |
| `core:event:default` | Granted | Yes | OK |
| `core:event:allow-listen` | Granted | Yes | OK |
| `core:event:allow-emit` | Granted | Partial | OK (no backend listeners) |
| `opener:default` | Granted | Partial | Scope URLs (M1) |
| `dialog:default` | Granted | Partial | Reduce to save+message+confirm (M2) |
| IPC command deny | Not configured | Recommended | Add deny for sensitive commands (M3) |

---

## Priority Fix Order

1. **P0** (CRITICAL): C1 - Remove or restrict `read_import_file` (and `save_export_to_file`)
2. **P1** (HIGH): H2 - Add `sanitize_for_surrealdb()` to `execute_import`
3. **P1** (HIGH): H1 - Self-host Google Fonts or add CSP directives
4. **P1** (HIGH): H3 - Add migration status tracking to prevent re-triggering
5. **P2** (MEDIUM): M1, M2 - Scope opener and dialog permissions
6. **P2** (MEDIUM): M3 - Define deny patterns for sensitive commands
7. **P2** (MEDIUM): M4 - Replace `window.open()` with `openUrl()`
8. **P3** (LOW): L1, L2, L3 - Document CSP decisions, add `blob:` if needed

## Estimation

- P0 fix: ~30min (remove unused command or add path validation)
- P1 fixes: ~3h (sanitization, font self-hosting, migration tracking)
- P2 fixes: ~2h (capability JSON restructuring, window.open replacement)
- P3 fixes: ~30min (documentation, minor CSP tweaks)
- **Total: ~6h for complete remediation**

---

## Desktop Context

Key considerations for this audit in a Tauri desktop application:

1. **Filesystem access**: The desktop user already has full filesystem access via their OS. C1 (`read_import_file`) is not a privilege escalation in itself - but it becomes dangerous if combined with a webview compromise (XSS/CSP bypass), as it provides a programmatic filesystem primitive to injected JavaScript.
2. **CSP enforcement**: Tauri's custom protocol (`tauri://`) may handle CSP differently than a standard web server. H1 (Google Fonts) may silently fail rather than creating a security gap.
3. **Plugin permissions**: Over-broad permissions (M1, M2) are lower risk in a single-user desktop app than in a multi-tenant web app. The threat requires webview compromise first.
4. **No remote attack surface**: All findings require either local access, social engineering, or a prior webview compromise to exploit.

---

## Code Verification (2026-02-19)

**Methodology**: 4 exploration agents read the actual code. thinking-mcp bias checks applied.

### Severity Adjustments

| Finding | Original | Adjusted | Justification |
|---------|----------|----------|---------------|
| C1 | CRITICAL | **ADJUSTED HIGH** | `read_import_file` accepts arbitrary paths and IS registered in `generate_handler!`. However: (1) the frontend doesn't use it (uses FileReader API), (2) exploitation requires a prior XSS/CSP bypass to call `invoke()` with arbitrary paths, (3) the desktop user already has filesystem access. This is a chain-of-attack dependency, not a direct exploit. Still HIGH because it provides an unnecessary attack primitive. |
| H1 | HIGH | **CONFIRMED (if Google Fonts used)** | Verified: `+layout.svelte` loads Google Fonts via `<link>`. CSP lacks `font-src`. Either fonts silently fail (functionality bug) or Tauri bypasses CSP for external resources (larger concern). Self-hosting fonts is the correct fix for a desktop app. |
| H2 | HIGH | **CONFIRMED HIGH** | Same finding as SA-002 S2-M1 (upgraded to HIGH there too). Missing `sanitize_for_surrealdb()` on import data. |
| H3 | HIGH | **CONFIRMED HIGH** | Migration destroys embeddings without guard. Verified at `migration.rs:77`. |
| M1-M4 | MEDIUM | **CONFIRMED MEDIUM** | Hardening recommendations valid for defense-in-depth. |
| L1 | LOW | **CONFIRMED - NON ACTIONABLE** | `unsafe-inline` required by Svelte 5 for component styles. Framework constraint. |
| L2 | LOW | **CONFIRMED LOW** | `blob:` URI for memory export may be blocked. |
| L3 | LOW | **CONFIRMED LOW** | `img-src` falls back to secure default. |

### Summary After Verification

| Severity | Original Count | Adjusted Count | Change |
|----------|---------------|----------------|--------|
| CRITICAL | 1 | 0 | -1 (C1->HIGH) |
| HIGH | 3 | 4 | +1 (C1 moved here) |
| MEDIUM | 4 | 4 | No change |
| LOW | 3 | 3 | No change |
