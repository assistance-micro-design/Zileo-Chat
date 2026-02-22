# SA-018: Hardcoded Elements Audit

## Metadata
- **Date**: 2026-02-22
- **Domaine**: Codebase entier (frontend + backend)
- **Branch**: security/audit-remediation-tdd
- **Stack**: SvelteKit 2.49.1 + Svelte 5.49.1 | Rust 1.93.0 + Tauri 2.10.2 | SurrealDB 2.5.0
- **Impact**: Maintenabilite / i18n / Code mort
- **Status**: TERMINE (P1+P2+P3 FAIT)

## Resume Executif

Audit exhaustif des elements hardcodes dans Zileo-Chat-3. L'analyse couvre les secrets, URLs, model IDs, magic numbers, pricing, messages i18n, et configuration dispersee.

**Verdict global**: Le codebase est **securise** (zero secret reel expose) et **bien organise** pour les constantes metier (tools/constants.rs, schema.rs). Trois problemes reels identifies :

1. **Model IDs hardcodes** (~30) : code mort car les utilisateurs creent leurs modeles directement dans le frontend
2. **URLs dupliquees** (4x Ollama) : violation DRY, risque de desynchronisation
3. **Messages i18n manquants** (~20) : textes anglais en dur dans les settings

### Decisions de remediation

| Trouvaille | Decision | Justification |
|------------|----------|---------------|
| Model IDs hardcodes (~30) | **SUPPRIMER** | Code mort - modeles geres en DB via frontend |
| Pricing hardcode (7 constantes) | **SUPPRIMER** | Code mort (`#[allow(dead_code)]`) - pricing en DB |
| Thinking/reasoning detection | **REMPLACER** par `is_reasoning: bool` depuis DB | Deja disponible dans le schema |
| Ollama URL dupliquee 4x | **CENTRALISER** dans `ollama.rs` | Source unique Rust |
| Messages i18n manquants (~20) | **AJOUTER** cles i18n | Coherence bilingue |
| Chemin `.zileo` non-XDG | **REPORTE** | Necessite restructuration main.rs |
| Timeouts disperses | **CONSERVER** tel quel | Fonctionnels et adaptes |

---

## Methodologie

- 5 agents d'exploration paralleles (credentials, magic values, config architecture, i18n, model IDs)
- Verification des biais via thinking_mcp (sycophantie, remplissage de liste, cadrage)
- Distinction stricte code de production vs code de test
- Classification par impact reel, pas theorique

---

## Securite : Secrets et Credentials

### Verdict : PASS - Zero risque

| Element | Trouve | Contexte |
|---------|--------|----------|
| Vraies API keys | 0 | Aucune cle reelle dans le code |
| Vraies credentials | 0 | SecureKeyStore utilise pour le stockage |
| Tokens reels | 0 | Aucun token en dur |
| Cles de test fictives | ~16 | `"sk-1234567890abcdef"`, `"test-key"` - toutes en `#[cfg(test)]` |

Les cles fictives dans les tests sont **acceptables** et **necessaires** pour valider la logique de validation.

---

## HC-1: Model IDs Hardcodes - SUPPRIMER

### Contexte

Les utilisateurs creent leurs modeles directement dans le frontend (stockes en DB). Les listes hardcodees dans le code Rust sont du **code mort** : elles ne servent plus de reference car les modeles disponibles viennent de la base de donnees.

### Inventaire a supprimer

#### Mistral (src-tauri/src/llm/mistral.rs)

| Constante | Ligne | Action |
|-----------|-------|--------|
| `MISTRAL_MODELS` | 228-237 | SUPPRIMER |
| `DEFAULT_MISTRAL_MODEL` | 241 | SUPPRIMER |
| `REASONING_MODELS` | 257 | SUPPRIMER |

#### Ollama (src-tauri/src/llm/ollama.rs)

| Constante | Ligne | Action |
|-----------|-------|--------|
| `OLLAMA_MODELS` | 32-41 | SUPPRIMER |
| `DEFAULT_OLLAMA_MODEL` | 45 | SUPPRIMER |
| `OLLAMA_THINKING_MODELS` | 48-54 | SUPPRIMER (dead code, aucun appelant externe) |
| `is_thinking_model()` | 58-63 | SUPPRIMER (dead code) |
| `is_thinking_model_name()` | 284-286 | SUPPRIMER (dead code, 0 appelant) |

#### Pricing (src-tauri/src/llm/pricing.rs)

| Constante | Lignes | Action |
|-----------|--------|--------|
| `mod mistral_pricing` | 80-96 | SUPPRIMER (`#[allow(dead_code)]`, 0 usage production) |

#### Autres

| Constante | Fichier | Action |
|-----------|---------|--------|
| `VALID_MODEL_PROVIDERS` | tools/constants.rs:258 | SUPPRIMER (0 reference) |

### Elements a CONSERVER

| Constante | Fichier | Raison |
|-----------|---------|--------|
| `MISTRAL_EMBED_MODEL` | embedding.rs:56 | Utilise pour les embeddings (pas de choix utilisateur) |
| `MISTRAL_EMBED_DIMENSION` | embedding.rs:59 | Dimension vecteur fixe |
| `OLLAMA_NOMIC_DIMENSION` | embedding.rs:65 | Dimension fixe |
| `OLLAMA_MXBAI_DIMENSION` | embedding.rs:66 | Dimension fixe |
| `DEFAULT_OLLAMA_EMBED_MODEL` | embedding.rs:~70 | Modele embedding par defaut |
| `MISTRAL_EMBEDDING_URL` | embedding.rs:52 | Endpoint API fixe |
| `MISTRAL_API_URL` | mistral.rs:254 | Endpoint API fixe |

### Adaptations du trait LLMProvider

1. `available_models()` : retourner `Vec::new()` (modeles viennent de DB)
2. `default_model()` : retourner `String::new()` (defaut en DB via provider_settings)
3. `complete()` : ajouter parametre `is_reasoning: bool` (remplace detection par nom)

**Point cle** : `is_reasoning: bool` existe deja dans le schema DB (schema.rs:187) et le struct LLMModel (llm_models.rs:132). Le champ est charge depuis la DB dans streaming.rs:589.

### Validation a supprimer

`set_default_model()` dans commands/llm.rs:152-161 valide contre `available_models()` hardcode. Cette validation doit etre supprimee car les modeles viennent de DB.

---

## HC-2: URLs Dupliquees - CENTRALISER

### Ollama URL (4 occurrences de `http://localhost:11434`)

| Fichier | Ligne | Action |
|---------|-------|--------|
| `src-tauri/src/llm/ollama.rs` | 97 | **SOURCE UNIQUE** (conserver) |
| `src-tauri/src/llm/embedding.rs` | 63 | REMPLACER par `use super::ollama::DEFAULT_OLLAMA_URL` |
| `src-tauri/src/models/llm_models.rs` | 435 | REMPLACER par import |
| `src/types/llm.ts` | 82 | CONSERVER (sync manuelle TS/Rust) + ajouter commentaire sync |

### Mistral API URLs

Conservees telles quelles : paths differents (`/chat/completions` vs `/embeddings` vs `/models`), pas de vraie duplication.

---

## HC-3: Messages i18n Manquants - AJOUTER

### ~18 nouvelles cles i18n

**Fichiers i18n** : `src/messages/en.json`, `src/messages/fr.json`

Prefixe `settings.` pour toutes les cles.

### Composants a modifier

| Fichier | Messages | Confirms |
|---------|----------|----------|
| `APIKeysSection.svelte` | 5 | 1 |
| `LLMSection.svelte` | 9 | 1 |
| `MCPSection.svelte` | 0 | 1 |
| `CustomProviderForm.svelte` | 1 | 0 |

### aria-labels (basse priorite)

5 aria-labels hardcodes dans ActivityFeed, ActivityItemDetails, ReasoningDetailsPanel, ToolDetailsPanel, MCPSection. Optionnel pour cette phase.

---

## HC-4: Timeouts Disperses - CONSERVER

Les timeouts dans llm/ (300s pour Ollama/Mistral, 30s pour OpenAI-compatible) sont fonctionnels et adaptes aux cas d'usage. Le fichier `tools/constants.rs` centralise deja les timeouts principaux. Pas d'action requise.

---

## HC-5: Chemin `.zileo` - REPORTE

Le dossier `.zileo` est hardcode dans `$HOME` au lieu de suivre les conventions XDG (`$XDG_DATA_HOME/zileo`). La correction necessite une restructuration de `main.rs` (deplacer AppState dans le setup hook Tauri) ce qui depasse le scope de cette phase d'optimisation.

---

## Ce Qui Est Bien Fait

| Domaine | Qualite | Evidence |
|---------|---------|----------|
| Constantes metier | Excellent | `tools/constants.rs` (259 lignes, 10 modules) |
| Schema DB | Excellent | `schema.rs` centralise, ASSERT validations |
| Noms de tables | Excellent | `CASCADE_DELETE_TABLES`, schemas centralises |
| Evenements Tauri | Bon | `models/streaming.rs::constants` |
| Noms d'outils | Bon | `src/lib/constants/tools.ts` |
| Embedding dimensions | Bon | Constantes nommees dans `embedding.rs` |
| Query templates | Bon | `queries.rs` avec SELECT templates |
| Status strings | Excellent | Valides par ASSERT dans le schema DB |

---

## Faux Positifs Filtres

| Element | Pourquoi c'est acceptable |
|---------|--------------------------|
| Cles de test fictives (`"test-key"`, `"sk-1234..."`) | Necessaires pour les tests, jamais en production |
| URLs `api.example.com` dans tests | Convention standard pour exemples |
| IPs privees dans tests (`192.168.*`) | Tests de validation HTTP warning |
| Constantes nommees dans `tools/constants.rs` | Deja bien centralisees |
| Enums de status dans schema DB | Validees par ASSERT, pas du hardcoding |

---

## Plan d'Implementation

| Phase | Description | Effort | Status |
|-------|-------------|--------|--------|
| P1 | Supprimer model IDs hardcodes (Rust) | 3h | FAIT |
| P2 | Centraliser DEFAULT_OLLAMA_URL | 20min | FAIT |
| P3 | i18n messages settings | 1.5h | FAIT |
| P4 | Chemin .zileo (XDG) | REPORTE | - |

P1, P2, P3 sont independantes. Execution sequentielle pour reviews incrementales.

### Verification

```bash
# Backend
cargo fmt --check && cargo clippy -- -D warnings && cargo test

# Frontend
npm run lint && npm run check && npm run test
```

---

## Implementation Status

### P1: Model IDs hardcodes - FAIT (commit e3f6d8b)
- [x] Supprimer listes de modeles (MISTRAL_MODELS, OLLAMA_MODELS, defaults)
- [x] Adapter trait LLMProvider (available_models, default_model -> vide)
- [x] Supprimer validation hardcodee dans set_default_model
- [x] Supprimer listes reasoning/thinking, adapter is_reasoning via bool param
- [x] Supprimer pricing reference (mistral_pricing module)
- [x] Supprimer VALID_MODEL_PROVIDERS dead code
- [x] Mettre a jour tests (22 fichiers, -376/+148 lignes)
- [x] Propager is_reasoning: bool dans LLMConfig (agent.rs, agent.ts, AgentForm.svelte, main.rs, spawn_agent.rs)

### P2: URLs - FAIT (commit 7c4be8c)
- [x] Centraliser DEFAULT_OLLAMA_URL (embedding.rs -> import from ollama.rs, llm_models.rs -> import from llm mod)
- [x] Re-export DEFAULT_OLLAMA_URL dans llm/mod.rs
- [x] Ajouter commentaire sync dans llm.ts

### P3: i18n - FAIT (commit c433591)
- [x] Ajouter 27 cles en.json / fr.json (prefixe settings_)
- [x] Remplacer 6 messages dans APIKeysSection
- [x] Remplacer 12 messages dans LLMSection (+ utiliser providers_all existant)
- [x] Remplacer 6 messages dans MCPSection
- [x] Remplacer 1 message dans CustomProviderForm
