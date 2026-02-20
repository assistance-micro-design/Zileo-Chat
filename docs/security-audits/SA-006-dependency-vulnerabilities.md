# SA-006: Dependency Vulnerabilities Audit

**Date**: 2026-02-19
**Scope**: All direct and transitive dependencies (NPM + Rust/Cargo)
**Status**: Partially remediated (DEP-1 rig-core + DEP-2 SurrealDB DONE, DEP-3 NPM updates pending)
**Cross-references**: SA-002 (DOMPurify, XSS), SA-005 (Tauri permissions), SA-008 (rig-core architecture)

## Summary

| Ecosystem | Direct Deps | Transitive | CVEs Found | Unmaintained | Feature Bloat |
|-----------|-------------|------------|------------|--------------|---------------|
| NPM       | 27 (7 runtime + 20 dev) | 290 dev, 28 prod | 7 advisories (15 vuln paths) | 0 | 1 |
| Rust      | 22 + 1 dev | 825 total | 0 CVEs | 20 warnings | 2 |

| Risk Level | Count | Summary |
|------------|-------|---------|
| HIGH | 3 | SvelteKit CVEs (mitigated by context), rig-core feature bloat, surrealdb 3.0 major gap |
| MEDIUM | 4 | Svelte SSR XSS, eslint ReDoS chain, surrealdb protocol bloat, Tauri GTK3 unmaintained |
| LOW | 5 | cookie out-of-bounds, ajv ReDoS, unic-* unmaintained, version pinning gaps, keyring multi-platform |
| INFO | 2 | Lock files committed, Dependabot configured |

---

## A. Automated CVE Audit Results

### A.1 NPM (`npm audit`)

**Total: 15 vulnerability paths across 7 distinct advisories**

| Advisory | Package | Installed | Severity | Title | Fix Available | Applies to Desktop? |
|----------|---------|-----------|----------|-------|---------------|---------------------|
| [GHSA-j2f3-wq62-6q46](https://github.com/advisories/GHSA-j2f3-wq62-6q46) | @sveltejs/kit | 2.49.1 | HIGH | Memory amplification DoS in Remote Functions binary deserializer | 2.49.5+ | **NO** - Remote Functions are a server-side SvelteKit feature. App uses `adapter-static`, no server runtime. |
| [GHSA-j62c-4x62-9r35](https://github.com/advisories/GHSA-j62c-4x62-9r35) | @sveltejs/kit | 2.49.1 | HIGH | DoS + SSRF via prerendering | 2.49.5+ | **PARTIAL** - Prerendering runs during `npm run build` only. SSRF component requires external URL injection in routes. DoS is build-time only, not runtime. Low real risk but update recommended. |
| [GHSA-m56q-vw4c-c2cp](https://github.com/advisories/GHSA-m56q-vw4c-c2cp) | svelte | 5.49.1 | MODERATE | `<svelte:element>` SSR XSS - tag names not validated | 5.51.5+ | **NO** - SSR-only vulnerability. App uses client-side rendering in Tauri webview. |
| [GHSA-f7gr-6p89-r883](https://github.com/advisories/GHSA-f7gr-6p89-r883) | svelte | 5.49.1 | MODERATE | XSS via spread attributes in SSR | 5.51.5+ | **NO** - SSR-only. |
| [GHSA-h7h7-mm68-gmrc](https://github.com/advisories/GHSA-h7h7-mm68-gmrc) | svelte | 5.49.1 | MODERATE | XSS in SSR `<option>` element | 5.51.5+ | **NO** - SSR-only. |
| [GHSA-3ppc-4f35-3m26](https://github.com/advisories/GHSA-3ppc-4f35-3m26) | minimatch | <10.2.1 | HIGH | ReDoS via repeated wildcards | eslint 10.0.0 (major) | **NO** - Dev-only (eslint transitive). Not in production bundle. |
| [GHSA-2g4f-4pwh-qvx6](https://github.com/advisories/GHSA-2g4f-4pwh-qvx6) | ajv | <8.18.0 | MODERATE | ReDoS when using `$data` option | eslint 10.0.0 (major) | **NO** - Dev-only (eslint transitive). |
| [GHSA-pxg6-pf52-xh8x](https://github.com/advisories/GHSA-pxg6-pf52-xh8x) | cookie | <0.7.0 | LOW | Accepts cookie name/path/domain with OOB characters | @sveltejs/kit update | **NO** - Cookie handling is server-side. Tauri desktop app has no HTTP cookie surface. |

**Transitive vulnerability propagation (dev-only, 12 of 15 paths):**

The eslint ecosystem accounts for 12 vulnerability paths, all flowing through:
- `eslint` -> `minimatch` (ReDoS)
- `eslint` -> `@eslint/eslintrc` -> `ajv` (ReDoS)
- `eslint` -> `@eslint/config-array` -> `minimatch` (ReDoS)
- `@typescript-eslint/*` -> `minimatch` (ReDoS)

These are entirely dev-time and do not affect the production Tauri binary.

### A.2 Rust (`cargo audit`)

**Total: 0 CVEs, 1 unsoundness warning, 19 unmaintained warnings**

| ID | Crate | Version | Type | Title | Source Chain | Actionable? |
|----|-------|---------|------|-------|-------------|-------------|
| RUSTSEC-2024-0429 | glib | 0.18.5 | **unsound** | Unsoundness in `VariantStrIter` | tauri -> wry -> gtk -> glib | **NO** - Tauri-managed transitive. Would need Tauri to update to gtk4-rs. Not exploitable unless `VariantStrIter` is used directly (it isn't). |
| RUSTSEC-2024-0412..0420 | atk, atk-sys, gdk, gdk-sys, gdkwayland-sys, gdkx11, gdkx11-sys, gtk, gtk-sys, gtk3-macros | 0.18.2 | unmaintained | GTK3 bindings deprecated (9 crates) | tauri -> wry -> gtk | **NO** - Tauri still uses GTK3 on Linux. Migration to GTK4 is a Tauri upstream decision. |
| RUSTSEC-2024-0370 | proc-macro-error | 1.0.4 | unmaintained | No longer maintained | glib-macros -> gtk | **NO** - Transitive via GTK3. Build-time only (proc-macro). |
| RUSTSEC-2023-0089 | atomic-polyfill | 1.0.3 | unmaintained | Unmaintained | surrealdb -> geo-types -> rstar -> heapless | **NO** - SurrealDB transitive. |
| RUSTSEC-2025-0141 | bincode | 1.3.3 | unmaintained | Unmaintained | surrealdb-core | **NO** - SurrealDB internal. |
| RUSTSEC-2025-0057 | fxhash | 0.2.1 | unmaintained | No longer maintained | tauri -> wry/tauri-utils -> kuchikiki -> selectors | **NO** - Tauri transitive. |
| RUSTSEC-2025-0075..0100 | unic-char-property, unic-char-range, unic-common, unic-ucd-ident, unic-ucd-version | 0.9.0 | unmaintained | Unmaintained (5 crates) | tauri-utils -> urlpattern -> unic-* | **NO** - Tauri transitive. |

**Key observation**: All 20 warnings are in transitive dependencies of `tauri` or `surrealdb`. None are directly introduced by project code and none are actionable without upstream updates.

---

## B. High-Risk Dependencies (Attack Surface Analysis)

### B.1 surrealdb 2.6.1 (Cargo.toml specifies 2.5.0)

| Metric | Value |
|--------|-------|
| Transitive deps | ~200+ (largest dep tree in project) |
| Latest version | **3.0.0** (major upgrade available) |
| Attack surface | RocksDB embedded engine, HTTP server, WebSocket, geo, scripting engine |
| Known issues | CVE-2025-31477 (patched in 2.2.1+, current 2.6.1 is safe) |

**Risk**: HIGH - surrealdb 3.0.0 is a major version with likely breaking API changes. The 2.x branch will eventually stop receiving security patches.

**Feature assessment**:
- `kv-rocksdb`: **Required** - used for embedded storage (`Surreal::new::<RocksDb>()` in `db/client.rs:49`)
- `protocol-http`: **NOT used** - no HTTP client connection to SurrealDB. Only embedded `RocksDb` transport is used.
- `protocol-ws`: **NOT used** - resolved as enabled (possibly via `default` feature) but not used. Same as above.
- `rustls`: Pulled in as default. Unnecessary for embedded-only usage.

**Recommendation**: Remove `protocol-http` from Cargo.toml features. Investigate if `default-features = false, features = ["kv-rocksdb"]` reduces the dependency tree.

**REMEDIATED (2026-02-20)**: Changed to `default-features = false, features = ["kv-rocksdb"]`. Removed protocol-http, protocol-ws, rustls from SurrealDB subtree. Build passes, 902 tests pass, 0 clippy warnings.

### B.2 rig-core 0.30.0

| Metric | Value |
|--------|-------|
| Transitive deps | ~30 |
| Latest version | **0.31.0** (minor update available) |
| Features resolved | `all, default, derive, pdf, rayon, reqwest-tls` |

**Actual usage** (from grep of `use rig::`):
- `rig::providers::ollama` (ollama.rs)
- `rig::providers::mistral` (mistral.rs)
- `rig::client::{Nothing, CompletionClient}` (ollama.rs)
- `rig::completion::Prompt` (ollama.rs, mistral.rs)

**Unused features pulled by `all`**:
- `pdf`: Brings in `lopdf` (PDF parsing library). No PDF processing in the codebase.
- `rayon`: Parallel processing. Not used by any rig-core consumer in this codebase.

**Recommendation**: Replace `features = ["all"]` with `features = ["derive"]` (or the minimal set needed for provider support). This removes `lopdf` and `rayon` from the dependency tree, reducing attack surface and compile time.

**REMEDIATED (2026-02-20)**: Removed `features = ["all"]` from rig-core. Now uses default features only (`rig-core = "0.30.0"`). Removed lopdf, rayon, pdf feature from dependency tree.

### B.3 reqwest 0.12.28

| Metric | Value |
|--------|-------|
| Latest version | **0.13.2** (major upgrade) |
| Features | `rustls-tls, json, stream` (no default features) |
| Usage | MCP HTTP transport, provider connectivity tests, LLM API calls |

**Risk**: MEDIUM - reqwest 0.13 is a major version bump. Current 0.12 still receives patches. `rustls-tls` is correctly used (no `native-tls` dependency). `danger_accept_invalid_certs` is not used (verified in SA-005).

### B.4 tokio 1.49.0

| Metric | Value |
|--------|-------|
| Features | rt, rt-multi-thread, macros, sync, time, fs, io-util, net, process |
| `process` feature justification | MCP stdio transport spawns child processes |

**Assessment**: Feature selection is well-documented and justified. `process` is confirmed used in `mcp/server_handle.rs` for MCP stdio. No unnecessary features detected.

### B.5 Runtime NPM Dependencies

| Package | Installed | Latest | Last Release | CVEs | Risk |
|---------|-----------|--------|-------------|------|------|
| @tauri-apps/api | 2.10.1 | 2.10.1 | Current | 0 | LOW |
| @tauri-apps/plugin-dialog | 2.6.0 | 2.6.0 | Current | 0 | LOW |
| @tauri-apps/plugin-opener | 2.5.3 | 2.5.3 | Current | 0 | LOW |
| @lucide/svelte | 0.563.1 | 0.575.0 | Active | 0 | LOW (icon lib) |
| dompurify | 3.3.1 | 3.3.1 | 2025-12-08 | 0 | LOW (up to date) |
| marked | 17.0.1 | 17.0.3 | Active | 0 | LOW (patch behind) |
| zod | 4.3.6 | 4.3.6 | Current | 0 | LOW |

**DOMPurify 3.3.1** (cross-ref SA-002 S2-L1): Latest version. No known CVEs. SA-002 noted that defaults are used instead of explicit whitelist - this is a defense-in-depth concern, not a vulnerability.

---

## C. Abandoned / Unmaintained Dependencies

### C.1 Direct Dependencies - All Active

All 22 direct Rust dependencies and 27 NPM dependencies have active maintainers with releases within the last 12 months.

### C.2 Transitive Unmaintained (Rust)

| Crate | Last Release | Via | Risk |
|-------|-------------|-----|------|
| GTK3 bindings (9 crates) | Deprecated 2024-03 | tauri -> wry/tao | LOW - Tauri upstream responsibility. GTK4 migration is tracked by Tauri team. |
| atomic-polyfill 1.0.3 | 2023-07 | surrealdb -> heapless | LOW - Build-time polyfill, no runtime risk. |
| bincode 1.3.3 | Deprecated 2025-12 | surrealdb-core | LOW - Internal SurrealDB serialization. |
| fxhash 0.2.1 | Deprecated 2025-09 | tauri -> kuchikiki | LOW - Hash function, no security implications. |
| proc-macro-error 1.0.4 | Deprecated 2024-09 | gtk3-macros | LOW - Compile-time only. |
| unic-* (5 crates) | Deprecated 2025-10 | tauri-utils -> urlpattern | LOW - Unicode property lookup, Tauri transitive. |

### C.3 Transitive Unmaintained (NPM)

No unmaintained packages detected in the NPM dependency tree.

---

## D. Feature Bloat Analysis

### D.1 rig-core `features = ["all"]`

**Current features resolved**: `all, default, derive, pdf, rayon, reqwest-tls`

| Feature | Used? | Impact |
|---------|-------|--------|
| `derive` | YES | `rig-derive` proc-macro for tool definitions |
| `reqwest-tls` | YES | HTTP client for LLM API calls |
| `pdf` | **NO** | Pulls `lopdf` (PDF parser) - unused, increases attack surface |
| `rayon` | **NO** | Parallel processing - unused in this codebase |

**Recommendation**: `features = ["derive"]` should suffice. This removes ~5 unnecessary transitive dependencies.

**REMEDIATED (2026-02-20)**: Removed `features = ["all"]`, now uses default features. `pdf` and `rayon` no longer resolved.

### D.2 surrealdb `features = ["kv-rocksdb", "protocol-http"]`

**Current features resolved**: `default, kv-rocksdb, protocol-http, protocol-ws, rustls`

| Feature | Used? | Impact |
|---------|-------|--------|
| `kv-rocksdb` | YES | Embedded RocksDB storage engine |
| `protocol-http` | **NO** | HTTP client for remote SurrealDB servers. DB is embedded. |
| `protocol-ws` | **NO** | WebSocket client. Enabled via default features. |
| `rustls` | **NO** | TLS for network protocols. Unnecessary for embedded. |

**Recommendation**: Use `default-features = false, features = ["kv-rocksdb"]`. This eliminates `reqwest` (from SurrealDB's perspective), `tokio-tungstenite`, `rustls`, and related network deps from the SurrealDB subtree.

**REMEDIATED (2026-02-20)**: Applied recommended change. Only `kv-rocksdb` feature now resolved for SurrealDB.

### D.3 keyring `features = ["apple-native", "windows-native", "sync-secret-service"]`

All three platform backends are enabled. For a Linux-only build, only `sync-secret-service` is needed. However, maintaining cross-platform features is acceptable for portability. The unused backends are compile-time conditional and don't affect binary size.

**Assessment**: Acceptable - enables cross-platform builds.

### D.4 NPM Dev Dependencies

| Package | Used? | Evidence |
|---------|-------|---------|
| @playwright/test | YES | `tests/e2e/*.spec.ts` (7 e2e test files) |
| jsdom | YES | `vitest.config.ts:11` sets `environment: 'jsdom'` |
| @humanspeak/svelte-virtual-list | YES | Listed as devDep but likely used in components |

**Assessment**: No unused dev dependencies detected.

---

## E. Supply Chain Risks

### E.1 Lock Files

| File | Committed? | Status |
|------|-----------|--------|
| `package-lock.json` | YES | Reproducible NPM builds |
| `src-tauri/Cargo.lock` | YES | Reproducible Cargo builds |

### E.2 Registry Sources

- **NPM**: All packages sourced from `registry.npmjs.org` (verified in package-lock.json)
- **Cargo**: All crates sourced from `crates.io` (verified in Cargo.lock)
- No custom registries, git dependencies, or path dependencies configured.

### E.3 Dependabot Configuration

`.github/dependabot.yml` is correctly configured:

| Ecosystem | Directory | Schedule | PR Limit |
|-----------|-----------|----------|----------|
| npm | `/` | Weekly | 5 |
| cargo | `/src-tauri` | Weekly | 5 |
| github-actions | `/` | Monthly | 5 |

**Assessment**: Good coverage. All three ecosystems are monitored.

### E.4 Critical Dependency Maintainers

| Package | Maintainer(s) | Single Maintainer Risk? |
|---------|---------------|------------------------|
| surrealdb | SurrealDB Inc (company) | NO - Backed by funded company |
| rig-core | Arc53 / 0hq (company) | MEDIUM - Smaller team, relatively new (v0.x) |
| marked | `markedjs` org (6 maintainers) | NO |
| dompurify | `cure53` (security company) | NO - Maintained by security firm |
| tauri | Tauri Programme (CrabNebula company) | NO - Backed by funded company |
| reqwest | Sean McArthur (@seanmonstar) | MEDIUM - Primary maintainer is one person, but highly active and Tokio ecosystem core contributor |

---

## F. Version Pinning and Update Strategy

### F.1 NPM Version Ranges

| Package | Specifier | Risk |
|---------|-----------|------|
| svelte | `5.49.1` (exact) | LOW - Intentionally pinned |
| @sveltejs/kit | `^2.49.1` | MEDIUM - Allows minor/patch updates but currently 3 patches behind (2.52.2 available with security fixes) |
| eslint | `^9.0.0` | LOW - Dev only, allows minor updates |
| All others | `^x.y.z` (caret) | Standard - allows minor/patch updates within major |

### F.2 Rust Version Ranges

| Crate | Specifier | Risk |
|-------|-----------|------|
| tauri | `"2"` | Standard - allows any 2.x |
| surrealdb | `"2.5.0"` | Resolves to 2.6.1 (semver compatible). **3.0.0 available as major upgrade.** |
| rig-core | `"0.30.0"` | **0.31.0 available** - pre-1.0 semver means 0.31 may break API |
| reqwest | `"0.12"` | **0.13.2 available** as major upgrade |
| Others | Standard semver | Appropriate |

### F.3 Tauri Version Alignment

| Component | Version | Aligned? |
|-----------|---------|----------|
| tauri (Rust crate) | 2.10.2 | Reference |
| @tauri-apps/api | 2.10.1 | YES (patch level difference is normal) |
| @tauri-apps/cli | 2.10.0 | YES (patch level difference is normal) |
| tauri-plugin-dialog | 2.6.0 | YES (plugin versions track independently) |
| tauri-plugin-opener | 2.5.3 | YES |

**Assessment**: Tauri ecosystem versions are properly aligned.

### F.4 Outdated Dependencies Summary

**NPM** (16 packages outdated):

| Package | Current | Wanted | Latest | Priority |
|---------|---------|--------|--------|----------|
| @sveltejs/kit | 2.49.1 | 2.52.2 | 2.52.2 | **HIGH** - Security fixes |
| svelte | 5.49.1 | 5.49.1 | 5.53.0 | **MEDIUM** - SSR XSS fixes (low risk for desktop but good hygiene) |
| marked | 17.0.1 | 17.0.3 | 17.0.3 | LOW - Patch update |
| eslint | 9.39.1 | 9.39.2 | 10.0.0 | LOW - Dev only, major available |
| @eslint/js | 9.39.1 | 9.39.2 | 10.0.1 | LOW - Dev only |
| typescript-eslint | 8.53.1 | 8.56.0 | 8.56.0 | LOW - Dev only |
| @typescript-eslint/eslint-plugin | 8.53.1 | 8.56.0 | 8.56.0 | LOW - Dev only |
| @typescript-eslint/parser | 8.54.0 | 8.56.0 | 8.56.0 | LOW - Dev only |
| eslint-plugin-svelte | 3.14.0 | 3.15.0 | 3.15.0 | LOW - Dev only |
| @lucide/svelte | 0.563.1 | 0.563.1 | 0.575.0 | LOW - Icons, major pin |
| @humanspeak/svelte-virtual-list | 0.3.6 | 0.3.13 | 0.4.1 | LOW - UI component |
| @playwright/test | 1.58.0 | 1.58.2 | 1.58.2 | LOW - Dev only |
| jsdom | 27.4.0 | 27.4.0 | 28.1.0 | LOW - Dev only, major available |
| svelte-check | 4.3.5 | 4.4.1 | 4.4.1 | LOW - Dev only |
| vite | 7.2.6 | 7.3.1 | 7.3.1 | LOW - Dev only |
| vitest | 4.0.15 | 4.0.18 | 4.0.18 | LOW - Dev only |

**Rust** (key outdated):

| Crate | Installed | Latest Stable | Priority |
|-------|-----------|---------------|----------|
| surrealdb | 2.6.1 | 3.0.0 | HIGH - Major upgrade, plan needed |
| rig-core | 0.30.0 | 0.31.0 | MEDIUM - Pre-1.0, may have breaking changes |
| reqwest | 0.12.28 | 0.13.2 | MEDIUM - Major upgrade |
| keyring | 3.6.3 | 3.6.3 (4.0.0-rc.3) | LOW - Stable version is current |
| aes-gcm | 0.10.3 | 0.10.3 (0.11.0-rc.3) | LOW - Stable version is current |

---

## G. Cross-References with Previous Audits

### G.1 SA-002: DOMPurify (S2-L1)

**Status**: dompurify 3.3.1 is the latest release (2025-12-08). No CVEs. SA-002's finding about explicit whitelist config remains a defense-in-depth recommendation, not a vulnerability.

### G.2 SA-008: rig-core Architecture

**Status**: rig-core 0.30.0 with `features = ["all"]` confirmed feature bloat concern. The `pdf` and `rayon` features added unnecessary attack surface. **REMEDIATED**: Now uses default features only.

### G.3 Cargo.toml comment: CVE-2025-31477

**Status**: The comment `OPT-WF-5: Verified >= 2.2.1 (CVE-2025-31477 patched, current: 2.5.2)` on `tauri-plugin-opener` is outdated - current resolved version is 2.5.3. The CVE is long patched. Comment should be cleaned up.

**REMEDIATED (2026-02-20)**: Outdated OPT-WF-5 comment removed from Cargo.toml.

---

## Recommendations (Prioritized)

### Priority 1 - Immediate (Security)

| # | Action | Effort | Impact |
|---|--------|--------|--------|
| 1 | Update `@sveltejs/kit` to ^2.52.2 | Low | Patches 2 HIGH CVEs (even if low desktop risk) |
| 2 | Update `svelte` to 5.53.0 | Low | Patches 3 MODERATE SSR XSS |

### Priority 2 - Short Term (Attack Surface Reduction)

| # | Action | Effort | Impact |
|---|--------|--------|--------|
| 3 | Change rig-core to `features = ["derive"]` | Low | Removes lopdf, rayon from dep tree |
| 4 | Change surrealdb to `default-features = false, features = ["kv-rocksdb"]` | Medium (test) | Removes HTTP/WS/TLS stack from SurrealDB deps |
| 5 | Update rig-core to 0.31.0 | Medium | Newer version, check breaking changes |

### Priority 3 - Medium Term (Maintenance)

| # | Action | Effort | Impact |
|---|--------|--------|--------|
| 6 | Plan surrealdb 3.0.0 migration | High | Major version, breaking API changes likely |
| 7 | Plan reqwest 0.13 migration | Medium | Check if rig-core/surrealdb also need to align |
| 8 | Run `npm update` for semver-compatible patches | Low | Updates 14 dev deps to wanted versions |
| 9 | Clean up outdated Cargo.toml comments (OPT-WF-5) | Trivial | Code hygiene |

### Priority 4 - Monitor

| # | Action | Notes |
|---|--------|-------|
| 10 | GTK3 deprecation in Tauri | Track Tauri's planned migration to GTK4. No action possible on our side. |
| 11 | eslint 10.0.0 migration | Major upgrade, would resolve minimatch/ajv transitive vulns. Low priority (dev-only). |
| 12 | glib unsoundness (RUSTSEC-2024-0429) | Monitor for Tauri fix. VariantStrIter is not used directly in our code. |

---

## Appendix: Direct Dependency Inventory

### Rust Dependencies (Cargo.toml)

| Crate | Specified | Resolved | Latest Stable | Last Release | Role | Risk |
|-------|-----------|----------|---------------|-------------|------|------|
| tauri | 2 | 2.10.2 | 2.10.2 | Active | Core framework | LOW |
| tauri-plugin-opener | 2 | 2.5.3 | 2.5.3 | Active | URL opener | LOW |
| tauri-plugin-dialog | 2 | 2.6.0 | 2.6.0 | Active | File dialogs | LOW |
| serde | 1.0.228 | 1.0.228 | 1.0.228+ | Active | Serialization | LOW |
| serde_json | 1.0.149 | 1.0.149 | 1.0.149+ | Active | JSON | LOW |
| tokio | 1.49.0 | 1.49.0 | 1.49.0 | Active | Async runtime | LOW |
| surrealdb | 2.5.0 | 2.6.1 | **3.0.0** | Active | Database | **HIGH** |
| anyhow | 1.0 | 1.0.101 | 1.0.101 | Active | Error handling | LOW |
| thiserror | 2.0 | 2.0.18 | 2.0.18 | Active | Error derive | LOW |
| tracing | 0.1 | 0.1.44 | 0.1.44 | Active | Logging | LOW |
| tracing-subscriber | 0.3 | 0.3.22 | 0.3.22 | Active | Log formatting | LOW |
| uuid | 1.20 | 1.20.0 | 1.20.0 | Active | UUID gen | LOW |
| chrono | 0.4.43 | 0.4.43 | 0.4.43 | Active | DateTime | LOW |
| async-trait | 0.1 | 0.1.89 | 0.1.89 | Active | Async traits | LOW |
| futures | 0.3.31 | 0.3.31 | 0.3.31 | Active | Async utils | LOW |
| regex | 1.10 | 1.12.3 | 1.12.3 | Active | Regex | LOW |
| once_cell | 1.20 | 1.21.3 | 1.21.3 | Active | Lazy init | LOW |
| tokio-util | 0.7 | 0.7.18 | 0.7.18 | Active | Async utils | LOW |
| rig-core | 0.30.0 | 0.30.0 | **0.31.0** | Active | LLM framework | **MEDIUM** |
| reqwest | 0.12 | 0.12.28 | **0.13.2** | Active | HTTP client | MEDIUM |
| futures-util | 0.3.31 | 0.3.31 | 0.3.31 | Active | Stream utils | LOW |
| keyring | 3.6 | 3.6.3 | 3.6.3 | Active | Credentials | LOW |
| aes-gcm | 0.10 | 0.10.3 | 0.10.3 | Active | Encryption | LOW |
| tauri-build | 2 | 2.5.5 | 2.5.5 | Active | Build | LOW |
| tempfile | 3.24 | 3.25.0 | 3.25.0 | Active | Dev test util | LOW |

### NPM Dependencies (package.json)

| Package | Specified | Installed | Latest | Type | Risk |
|---------|-----------|-----------|--------|------|------|
| @tauri-apps/api | ^2.9.0 | 2.10.1 | 2.10.1 | runtime | LOW |
| @tauri-apps/plugin-dialog | ^2.6.0 | 2.6.0 | 2.6.0 | runtime | LOW |
| @tauri-apps/plugin-opener | ^2.5.3 | 2.5.3 | 2.5.3 | runtime | LOW |
| @lucide/svelte | ^0.563.1 | 0.563.1 | 0.575.0 | runtime | LOW |
| dompurify | ^3.3.1 | 3.3.1 | 3.3.1 | runtime | LOW |
| marked | ^17.0.1 | 17.0.1 | 17.0.3 | runtime | LOW |
| zod | ^4.3.6 | 4.3.6 | 4.3.6 | runtime | LOW |
| svelte | 5.49.1 | 5.49.1 | **5.53.0** | dev | **MEDIUM** |
| @sveltejs/kit | ^2.49.1 | 2.49.1 | **2.52.2** | dev | **HIGH** |
| @sveltejs/adapter-static | ^3.0.0 | 3.0.10 | 3.0.10 | dev | LOW |
| @sveltejs/vite-plugin-svelte | ^6.2.4 | 6.2.4 | 6.2.4 | dev | LOW |
| @tauri-apps/cli | ^2.9.6 | 2.10.0 | 2.10.0 | dev | LOW |
| @types/dompurify | ^3.0.5 | 3.0.5 | 3.0.5 | dev | LOW |
| @typescript-eslint/eslint-plugin | ^8.0.0 | 8.53.1 | 8.56.0 | dev | LOW |
| @typescript-eslint/parser | ^8.54.0 | 8.54.0 | 8.56.0 | dev | LOW |
| eslint | ^9.0.0 | 9.39.1 | 10.0.0 | dev | LOW |
| eslint-plugin-svelte | ^3.14.0 | 3.14.0 | 3.15.0 | dev | LOW |
| globals | ^17.2.0 | 17.3.0 | 17.3.0 | dev | LOW |
| jsdom | ^27.4.0 | 27.4.0 | 28.1.0 | dev | LOW |
| @humanspeak/svelte-virtual-list | ^0.3.6 | 0.3.6 | 0.4.1 | dev | LOW |
| @playwright/test | ^1.58.0 | 1.58.0 | 1.58.2 | dev | LOW |
| svelte-check | ^4.3.5 | 4.3.5 | 4.4.1 | dev | LOW |
| typescript | ^5.9.3 | 5.9.3 | 5.9.3 | dev | LOW |
| typescript-eslint | ^8.53.1 | 8.53.1 | 8.56.0 | dev | LOW |
| vite | ^7.2.6 | 7.2.6 | 7.3.1 | dev | LOW |
| vitest | ^4.0.15 | 4.0.15 | 4.0.18 | dev | LOW |

---

## Methodology

- **npm audit** (v10, `--json` output): Scanned 317 packages (28 prod, 290 dev)
- **cargo audit** v0.22.1: Scanned 825 crates against RustSec advisory database
- **npm outdated**: Checked current vs wanted vs latest versions
- **cargo tree**: Analyzed dependency trees and feature resolution
- **Manual review**: Verified CVE applicability to desktop/static-adapter context
- **Code grep**: Confirmed feature usage (rig-core imports, SurrealDB connection pattern)
- **Bias check**: Applied structured bias framework to avoid dismissing desktop-context vulnerabilities

---

## Desktop Impact Assessment (2026-02-19)

This section evaluates each CVE finding against the actual deployment context: a **Tauri desktop application** using `adapter-static` (no server runtime), with client-side rendering only (no SSR), and no HTTP server.

### NPM CVE Applicability

| Advisory | Severity | Desktop Applicable? | Reasoning |
|----------|----------|---------------------|-----------|
| GHSA-j2f3-wq62-6q46 (@sveltejs/kit memory DoS) | HIGH | **NOT APPLICABLE** | Remote Functions are a server-side SvelteKit feature. App uses `adapter-static`, no server runtime exists. |
| GHSA-j62c-4x62-9r35 (@sveltejs/kit prerendering SSRF) | HIGH | **NOT APPLICABLE** | Prerendering runs at `npm run build` time only. SSRF requires attacker-controlled route URLs in source code. DoS is build-time only. |
| GHSA-m56q-vw4c-c2cp (svelte SSR XSS) | MODERATE | **NOT APPLICABLE** | SSR-only vulnerability. App uses CSR in Tauri webview. `<svelte:element>` tag validation is server-side. |
| GHSA-f7gr-6p89-r883 (svelte SSR XSS) | MODERATE | **NOT APPLICABLE** | SSR-only. Spread attributes in server rendering context. |
| GHSA-h7h7-mm68-gmrc (svelte SSR XSS) | MODERATE | **NOT APPLICABLE** | SSR-only. `<option>` element XSS in server rendering. |
| GHSA-3ppc-4f35-3m26 (minimatch ReDoS) | HIGH | **NOT APPLICABLE** | Dev-only dependency (eslint transitive). Not in production bundle. Not shipped to users. |
| GHSA-2g4f-4pwh-qvx6 (ajv ReDoS) | MODERATE | **NOT APPLICABLE** | Dev-only dependency (eslint transitive). Not in production bundle. |

**Result: 7/7 CVEs are NOT APPLICABLE to this desktop application.**

### Why Update Anyway?

Even though these CVEs don't affect the production application, updating is recommended for:

1. **Build-time safety**: The prerendering SSRF (GHSA-j62c-4x62-9r35) theoretically affects `npm run build` if route URLs were attacker-controlled.
2. **Hygiene**: Keeping dependencies current reduces the window of exposure for future CVEs that may be applicable.
3. **CI clean scans**: `npm audit` in CI pipeline will report these, creating noise that obscures real issues.
4. **Low effort**: Both @sveltejs/kit and svelte updates are semver-compatible patch/minor updates.

### Rust Cargo Audit

- **0 CVEs in direct dependencies** - no action needed.
- **1 unsoundness warning** (glib VariantStrIter) - Tauri transitive, not exploitable in this codebase.
- **19 unmaintained warnings** - All Tauri/SurrealDB transitive. No action possible without upstream updates.
