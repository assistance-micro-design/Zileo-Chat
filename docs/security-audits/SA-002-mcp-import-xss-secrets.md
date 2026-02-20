# SA-002: MCP + Import/Export + XSS + Secret Management Audit

**Date**: 2026-02-19
**Scope**: External attack surface - MCP servers, Import/Export, Markdown rendering, API key handling
**Status**: Documented (fixes pending)
**Covers planned**: SA-002 (XSS), SA-003 (MCP Input), SA-004 (Secret Management)

## Summary

| Vector | CRITICAL | HIGH | MEDIUM | LOW |
|--------|----------|------|--------|-----|
| MCP Tool Output Flow | 0 | 0 | 1 | 0 |
| MCP Stdio Transport | 0 | 0 | 0 | 0 |
| Import/Export | 0 | 2 | 2 | 1 |
| Markdown / XSS | 0 | 0 | 1 | 1 |
| API Key / Secrets | 1 | 1 | 1 | 0 |
| **Total** | **1** | **3** | **5** | **2** |

---

## Threat Matrix

| ID | Vector | Threat | Severity | Exploitability | File:Line | Status |
|----|--------|--------|----------|----------------|-----------|--------|
| S2-C1 | Secrets | Custom provider base_url accepts HTTP - API keys sent in cleartext | CRITICAL | HIGH - user adds provider with http:// URL | `commands/custom_provider.rs:127` | OPEN |
| S2-H1 | Import | String interpolation of imported fields (lifecycle, provider, model, category) in UPDATE/CREATE queries | HIGH | MEDIUM - requires crafted import file | `commands/import_export.rs:821-823,951,1056,1178` | OPEN |
| S2-H2 | Import | String interpolation of entity IDs in export SELECT queries | HIGH | LOW - IDs come from frontend selection but no bind params | `commands/import_export.rs:78-80,106-108,142-144,167-169` | OPEN |
| S2-H3 | Secrets | MCP HTTP base_url not validated for HTTPS - API_KEY env var sent over HTTP | HIGH | MEDIUM - requires MCP server config with http:// URL | `mcp/http_handle.rs:160-178` | OPEN |
| S2-M1 | Import | Missing `sanitize_for_surrealdb()` on imported data - null bytes can crash DB | MEDIUM | MEDIUM - crafted JSON with \u0000 | `commands/import_export.rs:730-1214` | OPEN |
| S2-M2 | Import | No per-entity count/size limits in import validation | MEDIUM | LOW - 10MB file limit exists but no entity-level limits | `commands/import_export.rs:498` | OPEN |
| S2-M3 | XSS | No URL scheme validation in handleOpenInBrowser() | MEDIUM | LOW - DOMPurify strips javascript: from href, but defense-in-depth missing | `components/ui/MarkdownRenderer.svelte:77` | OPEN |
| S2-M4 | Secrets | MCP env vars with sensitive keys exported in plain text (UI mitigation exists) | MEDIUM | LOW - requires user to skip sanitization dialog | `commands/import_export.rs:240-290` | PARTIAL |
| S2-M5 | MCP | No recursion depth limit in sanitize_for_surrealdb() | MEDIUM | LOW - deeply nested JSON from malicious MCP server could stack overflow | `db/utils.rs:51-68` | OPEN |
| S2-L1 | XSS | DOMPurify uses defaults instead of explicit whitelist | LOW | VERY LOW - defaults are safe, explicit config is defense-in-depth | `components/ui/MarkdownRenderer.svelte:45` | OPEN |
| S2-L2 | Import | serde_json::to_string() used as query escaping for name fields | LOW | VERY LOW - JSON encoding is effective but not proper parameterization | `commands/import_export.rs:548-550,593-595` | OPEN |

---

## CRITICAL Findings

### S2-C1: Custom Provider Base URL Accepts HTTP (API Key Exposure)

- **File**: `commands/custom_provider.rs:127-129`
- **Code**:
  ```rust
  if base_url.trim().is_empty() || base_url.len() > 512 {
      return Err("Base URL must be 1-512 characters".into());
  }
  // No HTTPS validation!
  ```
- **Impact**: API keys transmitted as `Authorization: Bearer {key}` over unencrypted HTTP
- **Attack scenario**:
  1. User creates custom provider with `http://api.example.com/v1`
  2. Every LLM request sends API key in cleartext HTTP header
  3. Network observer (Wi-Fi, ISP, proxy) intercepts key
  4. OR: Attacker-controlled URL `http://evil.com/v1` receives key directly
- **Also affects**: `update_custom_provider()` at line 223-227
- **Downstream**: `openai_compatible.rs:348` sends POST with Bearer token to unvalidated URL
- **Fix**:
  ```rust
  if !normalized_url.starts_with("https://") {
      return Err("Base URL must use HTTPS (https://)".into());
  }
  ```

---

## HIGH Findings

### S2-H1: Import String Interpolation in UPDATE/CREATE Queries

- **File**: `commands/import_export.rs` (multiple locations in `execute_import()`)
- **Vulnerable fields** (directly interpolated, NOT JSON-encoded):

  | Entity | Field | Line |
  |--------|-------|------|
  | Agent | `lifecycle` | 821 |
  | Agent | `llm.provider` | 822 |
  | Agent | `llm.model` | 823 |
  | MCP Server | `command` | 943 |
  | MCP Server | `enabled` | 951 |
  | Model | `provider` | 1056, 1071 |
  | Prompt | `category` | 1178, 1188 |

- **Code example** (agent import):
  ```rust
  format!(
      "UPDATE agent:`{}` SET name = {}, lifecycle = '{}', \
       llm = {{ provider: '{}', model: '{}', ... }}",
      agent_id, name_json,
      agent.lifecycle,    // UNESCAPED
      agent.llm.provider, // UNESCAPED
      agent.llm.model,    // UNESCAPED
  )
  ```
- **Note**: `name_json` IS properly encoded via `serde_json::to_string()`, but other fields are not
- **Attack scenario**: Crafted import file with `lifecycle = "'; DELETE agent WHERE '1'='1"`
- **Fix**: Use `CONTENT $data` bind pattern with full serde_json::to_value()
- **Overlap**: Extends SA-001 C3 findings to additional import paths

### S2-H2: Export Query ID Interpolation

- **File**: `commands/import_export.rs:78-80,106-108,142-144,167-169,238-240,304-306,384-386,425-427`
- **Code**:
  ```rust
  format!(
      "SELECT ... FROM agent WHERE meta::id(id) = '{}'",
      agent_id  // From frontend selection
  )
  ```
- **Repeated 8 times** across `prepare_export_preview()` and `generate_export_file()`
- **Source**: IDs come from frontend entity selection
- **Exploitability**: LOW (frontend generates valid UUIDs) but violates defense-in-depth
- **Fix**: Use `$id` bind parameter

### S2-H3: MCP HTTP Transport Missing HTTPS Validation

- **File**: `mcp/http_handle.rs:160-178`
- **Code**:
  ```rust
  if let Some(api_key) = config.env.get("API_KEY") {
      headers.insert(
          reqwest::header::AUTHORIZATION,
          format!("Bearer {}", api_key).parse()?
      );
  }
  // base_url not validated for HTTPS
  ```
- **Impact**: Same as S2-C1 but for MCP HTTP connections
- **Fix**: Validate `base_url.starts_with("https://")` in `connect()`

---

## MEDIUM Findings

### S2-M1: Missing sanitize_for_surrealdb() on Import Data

- **File**: `commands/import_export.rs:730-1214` (entire `execute_import()`)
- **Issue**: `sanitize_for_surrealdb()` is called for MCP call logs (`mcp/manager.rs:953`) but NOT for import data
- **Impact**: Null bytes in imported JSON can cause SurrealDB Strand type panic
- **Fix**: Add `sanitize_for_surrealdb()` call before any DB insertion in import flow

### S2-M2: No Per-Entity Import Limits

- **File**: `commands/import_export.rs:498`
- **Current**: 10MB file size limit only
- **Missing**:
  - Max entities per type (e.g., 100 agents, 50 servers)
  - Max field sizes (e.g., system_prompt < 100KB)
  - JSON nesting depth limit
- **Impact**: Memory exhaustion during import processing

### S2-M3: Missing URL Scheme Validation in Markdown Link Handler

- **File**: `components/ui/MarkdownRenderer.svelte:77`
- **Code**:
  ```typescript
  await openUrl(linkPopup.url);  // No scheme validation
  ```
- **Context**: DOMPurify strips `javascript:` from href attributes, so the primary vector is blocked
- **Risk**: Defense-in-depth gap if DOMPurify behavior changes or edge cases exist
- **Fix**:
  ```typescript
  const allowedSchemes = ['http://', 'https://'];
  if (!allowedSchemes.some(s => linkPopup.url.toLowerCase().startsWith(s))) return;
  await openUrl(linkPopup.url);
  ```

### S2-M4: Sensitive MCP Env Vars in Export

- **File**: `commands/import_export.rs:240-290`
- **Issue**: API_KEY, tokens, credentials included in export unless user explicitly sanitizes
- **Mitigation**: UI dialog highlights sensitive keys for user review
- **Risk**: User skips dialog, shares export file containing credentials
- **Fix**: Exclude keys matching sensitive patterns by default, require opt-in

### S2-M5: No Recursion Depth Limit in sanitize_for_surrealdb()

- **File**: `db/utils.rs:51-68`
- **Issue**: Recursive function with no depth limit
- **Impact**: Deeply nested JSON (e.g., from malicious MCP server) could cause stack overflow
- **Fix**:
  ```rust
  fn sanitize_with_depth(value: Value, depth: u32, max: u32) -> Value {
      if depth > max { return Value::Null; }
      // ... existing logic with depth + 1
  }
  ```

---

## Secure Areas (No Vulnerabilities Found)

### MCP Stdio Transport
- `Command::new()` used (no shell invocation)
- Commands hardcoded to `docker`, `npx`, `uvx` enum
- Args validated: max 50, max 512 chars each, no null bytes
- Env vars validated: alphanumeric+_ names, shell metachar rejection in values
- 12 unit tests for shell injection prevention
- **Rating: SECURE**

### MCP Tool Output → DB Flow
- `sanitize_for_surrealdb()` called before DB insertion (`mcp/manager.rs:953`)
- Parameterized query with `$data` bind (`mcp/manager.rs:955`)
- UUID generated client-side for record ID
- **Rating: SECURE**

### Markdown Rendering (DOMPurify + CSP)
- DOMPurify 3.3.1 sanitizes all markdown-rendered HTML
- CSP: `script-src 'self'` blocks inline script execution even if DOMPurify bypassed
- Single `{@html}` usage in codebase, properly justified
- Event handlers (onclick, onerror) stripped by DOMPurify
- SVG not in default allowed tags
- **Rating: SECURE** (with defense-in-depth recommendations above)

### API Key Storage
- AES-256-GCM encryption with random nonce per encryption
- Master key in OS keychain (keyring library)
- API keys never logged (verified via grep)
- `Validator::validate_api_key()` called before storage
- Bearer token in HTTP headers only (not query params)
- **Rating: SECURE**

---

## Overlap with SA-001

| SA-002 Finding | SA-001 Finding | Relationship |
|----------------|----------------|--------------|
| S2-H1 (import field interpolation) | C3 (import_memories content) | Same file, different functions. S2-H1 covers `execute_import()`, C3 covers `import_memories()` |
| S2-H2 (export ID interpolation) | Not covered | New finding specific to import_export.rs |
| S2-L2 (serde_json as escaping) | Not covered | New pattern observation |

---

## Priority Fix Order

| Priority | IDs | Description | Effort |
|----------|-----|-------------|--------|
| **P0** | S2-C1, S2-H3 | HTTPS enforcement for custom providers + MCP HTTP | 1h |
| **P1** | S2-H1 | Parameterize import UPDATE/CREATE queries | 3h |
| **P1** | S2-M1 | Add sanitize_for_surrealdb() to import flow | 30min |
| **P2** | S2-H2 | Parameterize export SELECT queries | 2h |
| **P2** | S2-M3 | URL scheme validation in MarkdownRenderer | 15min |
| **P2** | S2-M4 | Default-exclude sensitive env vars from export | 1h |
| **P3** | S2-M2, S2-M5 | Import entity limits + sanitize depth limit | 1h |
| **P4** | S2-L1, S2-L2 | DOMPurify explicit config + proper parameterization | 1h |

**Total estimated effort**: ~10h

---

## Uncertainty Notes

Items where manual testing is recommended:

1. **Tauri openUrl() scheme handling**: Need to verify if `@tauri-apps/plugin-opener` blocks `javascript:` and `data:` schemes natively
2. **DOMPurify data: URL behavior**: Default handling of `data:` scheme in img src attributes needs version-specific testing
3. **serde_json::to_string() as escaping**: While JSON encoding is effective against SurrealQL injection in practice, it's not a documented security guarantee of the function

---

## Desktop Context

This audit was originally written without fully accounting for the desktop application context. Key considerations:

1. **User-configured URLs**: S2-C1 and S2-H3 involve base URLs that the user explicitly configures themselves. In a desktop app, the user is the administrator. An HTTP URL is a poor security choice but not an externally exploitable vulnerability - it's a user decision.
2. **Import files are local**: Import data (S2-H1, S2-M1) comes from files the user explicitly selects via a file dialog. This is still an attack vector (malicious files shared via email/download), but the threat model is different from a web app accepting uploads from anonymous users.
3. **No network-accessible attack surface**: The app has no HTTP server, no open ports. All attacks require either local access or social engineering (tricking the user into importing a malicious file or configuring a malicious URL).
4. **MCP servers are user-installed**: MCP HTTP servers are explicitly configured by the user, similar to browser extensions. The trust boundary is at installation time.

These factors don't eliminate the risks but do reduce exploitability ratings for findings where the user is the attack vector.

---

## Methodology

- **Decomposition**: Problem broken into 5 independent sub-problems analyzed in parallel
- **Adversarial perspective**: Attack scenarios designed from attacker's viewpoint
- **Bias check**: Verified against 10 known LLM biases (anchoring to SA-001, pattern completion)
- **Tools**: Static code analysis via AST-aware search, data flow tracing, grep for security-sensitive patterns
- **Limitation**: Static analysis only, no runtime exploitation testing

---

## Code Verification (2026-02-19)

**Methodology**: 4 exploration agents read the actual code. thinking-mcp bias checks applied. Adversarial reframe considered import files as a real external attack surface despite desktop context.

### Severity Adjustments

| Finding | Original | Adjusted | Justification |
|---------|----------|----------|---------------|
| S2-C1 | CRITICAL | **ADJUSTED MEDIUM** | Desktop user configures their own provider URL. Not externally exploitable. An HTTPS-only policy is good practice but this is a user choice, not an attacker-controlled input. Add UI warning instead of hard block (allow localhost exception for dev). |
| S2-H1 | HIGH | **CONFIRMED HIGH** | Verified: `format!()` with lifecycle, provider, model, category in `import_export.rs`. External file content directly interpolated. Real injection risk via crafted import files. |
| S2-H2 | HIGH | **CONFIRMED HIGH** | Verified: 8 instances of `format!()` with entity IDs from frontend in export queries. Defense-in-depth violation. |
| S2-H3 | HIGH | **ADJUSTED MEDIUM** | Same reasoning as S2-C1. User-configured MCP HTTP URL. Not externally exploitable in desktop context. |
| S2-M1 | MEDIUM | **UPGRADED HIGH** | After adversarial reframe: imported files ARE a real external attack surface. A user downloading a shared config file with null bytes can crash the DB. `sanitize_for_surrealdb()` is missing at this entry point despite being used elsewhere. Inconsistent protection = real gap. |
| S2-M2 | MEDIUM | **CONFIRMED MEDIUM** | Per-entity limits are defense-in-depth. |
| S2-M3 | MEDIUM | **CONFIRMED MEDIUM** | DOMPurify strips `javascript:` but defense-in-depth missing. |
| S2-M4 | MEDIUM | **CONFIRMED MEDIUM** | UI mitigation exists but default-exclude would be safer. |
| S2-M5 | MEDIUM | **CONFIRMED MEDIUM** | Deeply nested JSON from malicious MCP server could stack overflow. |
| S2-L1 | LOW | **CONFIRMED LOW** | DOMPurify defaults are safe. |
| S2-L2 | LOW | **CONFIRMED LOW** | serde_json encoding is effective in practice. |

### Secure Areas Confirmation

- **MCP stdio transport**: CONFIRMED SECURE. `Command::new()`, no shell invocation, validated args.
- **MCP tool output flow**: CONFIRMED SECURE. `sanitize_for_surrealdb()` + `$data` bind.
- **Markdown rendering**: CONFIRMED SECURE. DOMPurify + CSP + link confirmation popup.
- **API key storage**: CONFIRMED SECURE. AES-256-GCM + OS keychain.

### Summary After Verification

| Severity | Original Count | Adjusted Count | Change |
|----------|---------------|----------------|--------|
| CRITICAL | 1 | 0 | -1 (S2-C1->MEDIUM) |
| HIGH | 3 | 3 | Net zero (S2-H3->MEDIUM, S2-M1->HIGH) |
| MEDIUM | 5 | 6 | +1 (S2-C1 + S2-H3 moved here, S2-M1 moved out) |
| LOW | 2 | 2 | No change |
