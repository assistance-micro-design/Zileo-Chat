# Plan d'Optimisation - Security

## Metadata
- **Date**: 2025-12-06
- **Domaine**: security
- **Stack**: Rust 1.91.1 + Tauri 2.9.4 + SurrealDB 2.3.10 + AES-256-GCM
- **Impact estime**: Securite (critique), Maintenabilite (moyen)

## Resume Executif

Ce plan adresse 10 vulnerabilites de securite identifiees dans les dependances (SurrealDB) et renforce les protections existantes (CSP, validation, injection). L'architecture de securite actuelle est solide (keystore AES-256-GCM, validation multi-niveau) mais necessite des mises a jour critiques et des ameliorations incrementales.

## Etat Actuel

### Analyse du Code

| Fichier | Complexite | Points d'attention |
|---------|------------|-------------------|
| `src-tauri/src/security/validation.rs:245` | Faible | Accepte newlines dans API keys |
| `src-tauri/src/commands/mcp.rs:191-240` | Moyenne | Env vars non validees pour injection shell |
| `src-tauri/src/commands/security.rs:124,156` | Faible | Enumeration providers via messages erreur |
| `src-tauri/tauri.conf.json:22` | Faible | CSP minimal sans directives critiques |
| `src-tauri/Cargo.toml:17` | CRITIQUE | SurrealDB 2.3.10 avec 10 CVEs |

### Patterns Identifies

- **Pattern Validation**: Utilisation systematique de `Validator::validate_*()` dans toutes les commandes Tauri
- **Pattern Encryption**: Double couche OS Keychain + AES-256-GCM pour API keys
- **Pattern Logging**: `#[instrument(skip(api_key))]` pour eviter leak credentials
- **Pattern Query**: Backtick-escaping et `.bind()` pour SurrealDB (protection injection)

### Metriques Actuelles

- Tests unitaires validation: 20+ cas couverts
- Couverture security/: ~85%
- Vulnerabilites dependencies: 10 (SurrealDB)

## Best Practices (2024-2025)

### Sources Consultees

- [Tauri Security Documentation](https://v2.tauri.app/security/) - Capabilities, CSP, Isolation
- [SurrealDB Security Best Practices](https://surrealdb.com/docs/surrealdb/reference-guide/security-best-practices) - Parameterized queries
- [OWASP Top 10 for LLMs 2025](https://genai.owasp.org/llmrisk/llm01-prompt-injection/) - Prompt Injection #1
- [GitHub SurrealDB Security Advisories](https://github.com/surrealdb/surrealdb/security/advisories) - 10 CVEs identifies

### Patterns Recommandes

1. **Tauri 2 Capabilities**: Permissions granulaires par fenetre via fichiers JSON dans `capabilities/`
2. **CSP Robuste**: `default-src 'self'; script-src 'self'; frame-ancestors 'none'; object-src 'none'`
3. **SurrealDB Parametrise**: Toujours utiliser `.bind()` pour inputs utilisateur
4. **Validation Declarative**: Crates `validator` ou `garde` pour validation structuree

### Anti-Patterns a Eviter

1. **Interpolation SQL**: `format!("SELECT * FROM {} WHERE id = '{}'", table, id)` - Injection
2. **CSP Permissif**: `default-src *; script-src 'unsafe-inline' 'unsafe-eval'` - XSS
3. **Erreurs Revelant Etat**: Messages distincts "not found" vs "error" - Enumeration

## Contraintes du Projet

- **Decision**: Production-ready des v1 - Source: `docs/ARCHITECTURE_DECISIONS.md`
- **Decision**: API keys jamais en clair, encryption AES-256-GCM - Source: `CLAUDE.md`
- **Decision**: Validation tous inputs utilisateur - Source: `docs/ARCHITECTURE_DECISIONS.md`
- **Contrainte**: Tauri v2 capability model requis - Source: `docs/TECH_STACK.md`

## Plan d'Optimisation

### Quick Wins (Impact haut, Effort faible)

#### OPT-1: Upgrade SurrealDB 2.3.10 vers 2.4.0 [CRITIQUE]

- **Fichiers**: `src-tauri/Cargo.toml`
- **Changement**:
  ```toml
  # Avant
  surrealdb = { version = "2.3.10", ... }
  # Apres
  surrealdb = { version = "2.4.0", ... }
  ```
- **Benefice**: Corrige 10 vulnerabilites dont 3 CRITIQUES:
  - GHSA-ccj3-5p93-8p42: Server-Takeover via SurrealQL Injection
  - GHSA-pxw4-94j3-v9pf: CPU Exhaustion DoS via boucles imbriquees
  - GHSA-rq86-9m6r-cm3g: Crash DB via null byte HTTP
- **Risque regression**: Moyen (verifier compatibilite API)
- **Validation**: `cargo test`, tester CRUD sur toutes les tables

#### OPT-2: Verifier tauri-plugin-opener >= 2.2.1

- **Fichiers**: `src-tauri/Cargo.toml`
- **Changement**: Verifier version, upgrader si < 2.2.1
- **Benefice**: Corrige CVE-2025-31477 (bypass validation protocole file://, smb://)
- **Risque regression**: Faible
- **Validation**: `cargo tree -p tauri-plugin-opener`, tester dialogs

#### OPT-3: Renforcer CSP Configuration

- **Fichiers**: `src-tauri/tauri.conf.json`
- **Changement**:
  ```json
  // Avant
  "csp": "default-src 'self'; style-src 'self' 'unsafe-inline'"

  // Apres
  "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'; object-src 'none'; base-uri 'self'; form-action 'self'"
  ```
- **Benefice**:
  - `frame-ancestors 'none'`: Prevention clickjacking
  - `object-src 'none'`: Bloque plugins malveillants
  - `base-uri 'self'`: Prevention base tag injection
- **Risque regression**: Faible (config only)
- **Validation**: DevTools > Security tab, verifier aucune violation

#### OPT-4: Rejeter newlines dans API keys

- **Fichiers**: `src-tauri/src/security/validation.rs`
- **Changement** (ligne 242-250):
  ```rust
  // Ajouter apres ligne 245
  if api_key.contains('\n') || api_key.contains('\r') {
      return Err(ValidationError::InvalidCharacters {
          details: "API key cannot contain newline characters".to_string(),
      });
  }
  ```
- **Benefice**: Prevention HTTP header injection
- **Risque regression**: Faible (validation plus stricte)
- **Validation**: Ajouter test unitaire, verifier tests existants passent

#### OPT-5: Normaliser messages erreur API key

- **Fichiers**: `src-tauri/src/commands/security.rs`
- **Changement** (lignes 124, 156):
  ```rust
  // Avant (ligne 124)
  error!(error = %e, "Failed to retrieve API key");

  // Apres - message generique
  warn!("API key operation failed for provider");
  ```
- **Benefice**: Prevention enumeration providers configures
- **Risque regression**: Faible
- **Validation**: Verifier logs ne revelent pas existence/absence

### Optimisations Strategiques (Impact haut, Effort eleve)

#### OPT-6: Valider env vars MCP contre injection shell

- **Fichiers**: `src-tauri/src/commands/mcp.rs`
- **Changement** (fonction `validate_mcp_env`, lignes 191-240):
  ```rust
  fn validate_mcp_env(env: &HashMap<String, String>) -> Result<HashMap<String, String>, String> {
      const FORBIDDEN_SHELL_CHARS: &[char] = &['|', ';', '`', '$', '(', ')', '<', '>', '&', '\\', '"', '\''];

      let mut validated = HashMap::new();
      for (key, value) in env {
          // Validation existante du key...

          // Ajouter: Validation value contre shell metacharacters
          if value.chars().any(|c| FORBIDDEN_SHELL_CHARS.contains(&c)) {
              return Err(format!(
                  "Environment variable value for '{}' contains forbidden shell characters",
                  key
              ));
          }
          validated.insert(key.clone(), value.clone());
      }
      Ok(validated)
  }
  ```
- **Benefice**: Prevention command injection via MCP server spawn
- **Risque regression**: Moyen (peut bloquer env vars legitimes avec $)
- **Validation**:
  - Ajouter tests: env vars avec |;`$() rejetees
  - Verifier MCP servers existants fonctionnent toujours
- **Prerequis**: Documenter limitation pour utilisateurs

#### OPT-7: Rate Limiting sur Operations Sensibles [DIFFERE]

- **Fichiers**: Nouveau module `src-tauri/src/security/rate_limiter.rs`
- **Changement**: Implementer rate limiting pour:
  - `save_api_key`: Max 10/min
  - `get_api_key`: Max 30/min
  - `call_mcp_tool`: Max 100/min
- **Benefice**: Prevention brute force et DoS
- **Risque regression**: Moyen (peut bloquer usage legitime intensif)
- **Effort**: 4h (design middleware + tests)
- **Statut**: DIFFERE - Necessaire pour v1.0, pas critique pour beta

#### OPT-8: Detection Prompt Injection LLM [DIFFERE]

- **Fichiers**: Nouveau module `src-tauri/src/security/prompt_guard.rs`
- **Changement**: Pattern matching sur inputs utilisateur:
  ```rust
  const INJECTION_PATTERNS: &[&str] = &[
      "ignore previous instructions",
      "forget everything",
      "reveal system prompt",
      "disregard all",
  ];
  ```
- **Benefice**: Mitigation partielle prompt injection (#1 OWASP 2025)
- **Risque regression**: Moyen (faux positifs possibles)
- **Effort**: 6h (patterns + integration + tuning)
- **Statut**: DIFFERE - Complexite elevee, necessite recherche approfondie

### Nice to Have (Impact faible, Effort faible)

#### OPT-9: Activer sanitize_for_logging()

- **Fichiers**: `src-tauri/src/commands/llm.rs:224`, autres
- **Changement**: Utiliser `Validator::sanitize_for_logging()` pour API response bodies
- **Benefice**: Prevention leak credentials dans logs
- **Effort**: 20min
- **Validation**: Verifier logs ne contiennent pas API keys

#### OPT-10: Upgrades Dependencies Mineures

- **Fichiers**: `src-tauri/Cargo.toml`
- **Changement**:
  - `aes-gcm`: 0.10 vers 0.10.3
  - `reqwest`: 0.12 vers 0.12.24
- **Benefice**: Patches securite mineurs
- **Effort**: 10min
- **Validation**: `cargo test`

### Differe (Impact faible, Effort eleve)

| Optimisation | Raison du Report |
|--------------|------------------|
| Migration vers crate `validator` | Systeme actuel fonctionne bien, refactoring majeur |
| Tauri per-window capabilities | Necessite restructuration UI (une seule fenetre actuellement) |
| HSM support pour master key | Overkill pour application desktop |
| Key rotation automatique | Acceptable manuellement pour v1 |

## Dependencies

### Mises a Jour Recommandees

| Package/Crate | Actuel | Recommande | Breaking Changes | Priorite |
|---------------|--------|------------|------------------|----------|
| surrealdb | 2.3.10 | 2.4.0 | Possible (verifier changelog) | CRITIQUE |
| tauri-plugin-opener | 2.x | >= 2.2.1 | Non | Haute |
| aes-gcm | 0.10 | 0.10.3 | Non | Faible |
| reqwest | 0.12 | 0.12.24 | Non | Faible |
| keyring | 2.0 | 3.0 | Oui (API changes) | Optionnel |

### Nouvelles Dependencies (si justifie)

| Package/Crate | Raison | Impact bundle | Statut |
|---------------|--------|---------------|--------|
| governor | Rate limiting | +50kb | DIFFERE |
| regex | Prompt injection patterns | Deja present | DIFFERE |

## Verification Non-Regression

### Tests Existants

- [x] `cargo test` - 20+ tests validation, keystore, MCP
- [x] `cargo clippy` - Aucun warning securite
- [x] `npm run check` - TypeScript strict mode

### Tests a Ajouter

- [ ] Test OPT-4: `validate_api_key("sk-key\nInjection")` retourne erreur
- [ ] Test OPT-6: `validate_mcp_env({"VAR": "value|cmd"})` retourne erreur
- [ ] Test OPT-6: `validate_mcp_env({"VAR": "normal_value"})` passe

### Verification Manuelle

```bash
# Apres OPT-1 (SurrealDB upgrade)
cargo test
npm run tauri dev
# Tester: creer agent, workflow, executer, supprimer

# Apres OPT-3 (CSP)
npm run tauri dev
# DevTools > Application > Frames > Security > CSP
# Verifier: aucune violation, styles chargent correctement
```

## Estimation

| Optimisation | Effort | Impact | Priorite |
|--------------|--------|--------|----------|
| OPT-1 SurrealDB upgrade | 15min | CRITIQUE | P0 |
| OPT-2 tauri-plugin-opener | 5min | Haute | P0 |
| OPT-3 CSP enhancement | 10min | Haute | P1 |
| OPT-4 API key newlines | 5min | Moyenne | P1 |
| OPT-5 Error normalization | 10min | Moyenne | P1 |
| OPT-6 MCP env validation | 30min | Haute | P1 |
| OPT-9 sanitize_for_logging | 20min | Faible | P2 |
| OPT-10 Minor upgrades | 10min | Faible | P2 |

**Total P0 (Critique)**: 20 minutes
**Total P1 (Haute)**: 55 minutes
**Total P2 (Nice to have)**: 30 minutes
**Total immediat**: ~1h45

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|--------|-------------|--------|------------|
| SurrealDB 2.4.0 breaking changes | Moyenne | Eleve | Lire changelog, tester CRUD complet |
| CSP bloque fonctionnalite | Faible | Moyen | Test manuel avant merge |
| MCP env validation trop stricte | Moyenne | Moyen | Documenter limitation, permettre escape |
| Rate limiting bloque usage legitime | Faible | Moyen | Limites genereuses, configurable |

## Prochaines Etapes

1. [x] Valider ce plan avec l'utilisateur
2. [ ] Executer OPT-1: SurrealDB upgrade (CRITIQUE)
3. [ ] Executer OPT-2: Verifier tauri-plugin-opener
4. [ ] Executer OPT-3: CSP enhancement
5. [ ] Executer OPT-4 + OPT-5: Validation improvements
6. [ ] Executer OPT-6: MCP env validation
7. [ ] Mesurer impact (aucune regression)
8. [ ] Planifier OPT-7, OPT-8 pour v1.0

## References

### Code Analyse
- `src-tauri/src/security/validation.rs` (498 lignes)
- `src-tauri/src/security/keystore.rs` (456 lignes)
- `src-tauri/src/commands/security.rs` (269 lignes)
- `src-tauri/src/commands/mcp.rs` (900+ lignes)
- `src-tauri/tauri.conf.json` (36 lignes)
- `src-tauri/capabilities/default.json` (14 lignes)

### Documentation Consultee
- `docs/ARCHITECTURE_DECISIONS.md` - Questions 7-10 securite
- `docs/TECH_STACK.md` - Stack et versions
- `docs/API_REFERENCE.md` - Commandes sensibles
- `CLAUDE.md` - Security Considerations

### Sources Externes
- [Tauri v2 Security](https://v2.tauri.app/security/)
- [SurrealDB Security Advisories](https://github.com/surrealdb/surrealdb/security/advisories)
- [OWASP Top 10 LLM 2025](https://genai.owasp.org/)
- [CVE-2025-31477 tauri-plugin-shell](https://security.snyk.io/vuln/SNYK-RUST-TAURIPLUGINSHELL-9697751)
