---
description: Workflow de diagnostic et correction d'erreurs pour Zileo-Chat-3
allowed-tools: Task(subagent_type:*), mcp__serena__*, mcp__context7__*, mcp__sequential-thinking__*, Glob, Read, Write, Edit, Bash(git:*, npm:*, cargo:*), TodoWrite
argument-hint: <description-erreur | "auto" pour diagnostic global>
---

# Workflow de Correction d'Erreurs Zileo-Chat-3

**Stack**: SvelteKit 2.49.0 + Svelte 5.43.14 | Rust 1.91.1 + Tauri 2.9.4 | SurrealDB 2.3.10

## Objectif

Diagnostiquer et corriger `$ARGUMENTS` avec analyse root cause systematique, sans quick fixes ni patches superficiels. Si `$ARGUMENTS` = "auto", executer un diagnostic global complet.

---

## Configuration

```
PROJECT_ROOT: (working directory)
ERROR_REPORTS: docs/corrections
SRC_FRONTEND: src
SRC_BACKEND: src-tauri
TYPES_DIR: src/types (alias: $types)
```

**IMPORTANT - TypeScript Imports**:
- Always use `$types` alias: `import type { X } from '$types/module'`
- Never use `$lib/types` (does not exist)

**IMPORTANT - Tauri IPC Parameter Naming**:
- Rust commands use `snake_case` parameters
- TypeScript `invoke()` calls use `camelCase` parameters
- Tauri converts automatically between the two formats

---

## Principes

- **ROOT CAUSE**: Toujours identifier la cause racine, pas les symptomes
- **SYSTEMATIQUE**: Corriger completement, pas de patches partiels
- **COMPREHENSION**: Comprendre AVANT de modifier
- **NON-REGRESSION**: Chaque fix doit etre valide sans casser autre chose
- **DOCUMENTATION**: Documenter le probleme ET la solution

---

## Interdictions Strictes

**Approche**:
- ❌ **Quick fixes** sans comprendre la cause
- ❌ **Supprimer du code** pour faire passer les tests
- ❌ **any/as any** pour contourner erreurs TypeScript
- ❌ **#[allow(...)]** pour ignorer warnings Clippy
- ❌ **Skip tests** defaillants
- ❌ **Commentaires** de code problematique
- ❌ **try/catch silencieux** masquant erreurs

**Process**:
- ❌ Modifier sans lire le code complet concerne
- ❌ Corriger symptome sans identifier root cause
- ❌ Passer outre validation finale

---

## Workflow

### Phase 0: Collecte des Erreurs

**0.1 Mode Auto** (si `$ARGUMENTS` = "auto"):
```bash
# Executer en parallele
npm run lint 2>&1 | head -100
npm run check 2>&1 | head -100
cargo clippy -- -D warnings 2>&1 | head -100
cargo test 2>&1 | head -100
```

**0.2 Mode Specifique** (si erreur decrite):
```
1. Parser $ARGUMENTS pour extraire:
   |- Type d'erreur (lint, type, runtime, test, build)
   |- Fichier(s) concerne(s)
   |- Message d'erreur exact
   |- Contexte de reproduction
```

**0.3 Classification**:
```
Categoriser chaque erreur:
|- LINT: ESLint, Clippy (style, best practices)
|- TYPE: TypeScript, Rust compiler (typage)
|- BUILD: Compilation, bundling
|- RUNTIME: Panics, exceptions, crashes
|- TEST: Tests unitaires/integration defaillants
|- IPC: Communication Tauri frontend/backend
|- DATABASE: Queries SurrealDB, serialisation
```

### Phase 1: Diagnostic Root Cause

**1.1 Lecture Contextuelle** (Serena prioritaire):
```
Pour CHAQUE erreur:
1. Serena: get_symbols_overview(fichier_concerne)
2. Serena: find_symbol(symbole_en_erreur, include_body=true)
3. Serena: find_referencing_symbols() si erreur de signature
4. Identifier l'historique: git log -p --follow -- <fichier>
```

**1.2 Analyse Sequentielle** (erreurs complexes):
```
Si erreur non triviale:
|- mcp__sequential-thinking__sequentialthinking:
   |- Decrire le symptome observe
   |- Lister les hypotheses possibles
   |- Eliminer hypotheses une par une
   |- Identifier la cause racine
   |- Proposer la correction minimale
```

**1.3 Cartographie Dependances**:
```
Identifier l'arbre d'impact:
|- Quels fichiers importent/utilisent le code en erreur?
|- Quels tests couvrent ce code?
|- Quelles autres erreurs pourraient etre liees?
|- Y a-t-il des erreurs en cascade?
```

**1.4 TodoWrite Diagnostic**:
```
Creer todo list avec:
|- [erreur_1] Description + cause identifiee + fichiers
|- [erreur_2] ...
|- Grouper erreurs liees (meme root cause)
|- Prioriser: BUILD > TYPE > RUNTIME > TEST > LINT
```

### Phase 2: Plan de Correction

**2.1 Strategie par Type**:

**Erreurs TYPE (TypeScript/Rust)**:
```
1. Verifier synchronisation types TS <-> Rust
2. Verifier imports ($types alias obligatoire)
3. Verifier signatures Tauri commands (snake_case <-> camelCase)
4. Corriger a la SOURCE, pas aux symptomes
```

**Erreurs LINT**:
```
1. Comprendre la regle violee (pas juste la faire taire)
2. Corriger le pattern incorrect
3. Si regle inappropriee -> justifier desactivation locale avec commentaire
```

**Erreurs BUILD**:
```
1. Verifier Cargo.toml / package.json
2. Verifier imports/exports manquants
3. Verifier features flags Rust
4. Nettoyer cache si necessaire: rm -rf node_modules/.cache target/
```

**Erreurs RUNTIME**:
```
1. Reproduire avec logs minimaux
2. Tracer l'execution (ou ca crash exactement?)
3. Identifier les valeurs inattendues
4. Corriger la logique, pas juste try/catch
```

**Erreurs TEST**:
```
1. Le test est-il correct? (peut-etre test obsolete)
2. Le code est-il correct? (bug reel)
3. Corriger LE BON COTE (test ou code)
4. Ne JAMAIS supprimer un test sans comprendre
```

**Erreurs IPC Tauri**:
```
1. Verifier enregistrement command dans main.rs
2. Verifier serialisation serde (Serialize, Deserialize)
3. Verifier nommage parametres (snake_case -> camelCase)
4. Verifier types retour (Result<T, String>)
```

**2.2 Ordre de Correction**:
```
1. Erreurs bloquantes (BUILD, TYPE critiques)
2. Erreurs en cascade (corriger racine d'abord)
3. Erreurs isolees (par fichier pour batch)
4. Warnings et style (LINT)
```

### Phase 3: Correction

**3.1 Avant Modification**:
```
OBLIGATOIRE:
|- Lire le code complet concerne (Serena ou Read)
|- Comprendre l'intention originale
|- Identifier tous les usages (find_referencing_symbols)
|- Verifier s'il y a des tests existants
```

**3.2 Modification**:
```
Strategie edition:
|- Serena replace_symbol_body: Pour symboles complets
|- Edit: Pour modifications ciblees dans contexte lu
|- MultiEdit: Pour changements similaires multi-fichiers
|- JAMAIS Write pour fichier existant sans Read prealable
```

**3.3 Corrections Typiques**:

**Synchronisation Types**:
```typescript
// Frontend: src/types/feature.ts
export interface FeatureConfig {
  defaultModelId: string;  // camelCase en TS
}
```
```rust
// Backend: src-tauri/src/models/feature.rs
pub struct FeatureConfig {
    pub default_model_id: String,  // snake_case en Rust
}
// Tauri convertit automatiquement
```

**Import Correct**:
```typescript
// CORRECT
import type { Feature } from '$types/feature';

// INCORRECT - ne pas utiliser
import type { Feature } from '$lib/types/feature';
import type { Feature } from '../types/feature';
```

**Command Tauri**:
```rust
// Enregistrement obligatoire dans main.rs
.invoke_handler(tauri::generate_handler![
    commands::feature::my_command,  // <- Ajouter ici
])
```

**3.4 Validation Incrementale**:
```
Apres CHAQUE correction:
|- Verifier que l'erreur ciblee est resolue
|- Verifier qu'aucune nouvelle erreur n'apparait
|- Si regression -> rollback + analyse approfondie
|- TodoWrite: marquer erreur comme corrigee
```

### Phase 4: Validation Complete

**4.1 Frontend**:
```bash
npm run lint          # 0 erreur
npm run check         # 0 erreur (TypeScript strict)
npm run test          # Tous tests passent
```

**4.2 Backend**:
```bash
cargo fmt --check     # Format OK
cargo clippy -- -D warnings  # 0 warning
cargo test            # Tous tests passent
cargo build --release # Build OK
```

**4.3 Integration** (si erreurs IPC):
```bash
npm run tauri dev     # Demarrage sans erreur console
# Tester les flows impactes manuellement
```

**4.4 Si Nouveaux Echecs**:
```
1. STOP - Ne pas continuer
2. Analyser les nouvelles erreurs (liees au fix?)
3. Si liees: etendre le scope de correction
4. Si non liees: bug preexistant, traiter separement
5. Retour Phase 1 si necessaire
```

### Phase 5: Prevention Regression

**5.1 Ajout Test** (si bug runtime/logique):
```
Si erreur etait un BUG (pas juste lint/type):
|- Ajouter test unitaire reproduisant le bug
|- Verifier que le test ECHOUE sur code avant fix
|- Verifier que le test PASSE sur code apres fix
|- Documenter le cas dans le test
```

**5.2 Documentation Fix**:
```
Dans le code si pattern non evident:
// NOTE: Utiliser $types alias, pas $lib/types (alias inexistant)
// NOTE: Tauri convertit snake_case (Rust) <-> camelCase (TS) automatiquement
```

### Phase 6: Rapport & Commit

**6.1 Git Commit**:
```bash
git add <fichiers-corriges>
git commit -m "$(cat <<'EOF'
fix(<scope>): <description courte du fix>

Root cause: <explication de la cause racine>
Solution: <explication de la correction>

Erreurs corrigees:
- [TYPE] <description>
- [LINT] <description>

Fichiers modifies:
- path/to/file.ts - <action>
- path/to/file.rs - <action>

Tests ajoutes: [oui/non]

Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

**6.2 Rapport** (`docs/corrections/YYYY-MM-DD_fix-<scope>.md`):
```markdown
# Rapport de Correction - [Scope]

## Metadonnees
- **Date**: YYYY-MM-DD HH:MM
- **Erreurs initiales**: X erreurs
- **Erreurs corrigees**: Y
- **Nouvelles regressions**: 0 (obligatoire)

## Diagnostic Initial

### Erreurs Collectees
| Type | Fichier | Message | Cause Racine |
|------|---------|---------|--------------|
| TYPE | path/file.ts:42 | ... | ... |
| LINT | path/file.rs:15 | ... | ... |

### Analyse Root Cause
[Explication de pourquoi les erreurs sont apparues]

## Corrections Appliquees

### [Erreur 1]
- **Probleme**: [Description]
- **Cause**: [Root cause identifiee]
- **Solution**: [Ce qui a ete fait]
- **Fichiers**: [Liste des modifications]

### [Erreur 2]
...

## Validation Finale

### Frontend
- **Lint**: PASS (0 erreurs)
- **TypeCheck**: PASS (0 erreurs)
- **Tests**: X/X PASS

### Backend
- **Clippy**: PASS (0 warnings)
- **Tests**: X/X PASS
- **Build**: SUCCESS

## Prevention

### Tests Ajoutes
- [Test 1]: Couvre [scenario]
- [Test 2]: ...

### Patterns a Eviter
- [Pattern problematique] -> [Pattern correct]

## Lecons Apprises
- [Point 1]
- [Point 2]
```

**6.3 Serena Memory** (si correction significative):
```
write_memory("fix_<scope>_<date>", {
  root_causes: [...],
  patterns_corrected: [...],
  files_impacted: [...],
  prevention_notes: [...]
})
```

---

## Gestion Complexite Erreurs

| Niveau | Criteres | Approche |
|--------|----------|----------|
| **Trivial** | 1-2 erreurs lint/format | Fix direct, validation rapide |
| **Simple** | <5 erreurs type/lint isolees | Fix sequentiel, validation complete |
| **Medium** | 5-15 erreurs ou erreurs liees | TodoWrite + Serena + groupage |
| **Complex** | >15 erreurs ou erreurs cascade | Sequential + Task agents + checkpoints |
| **Critical** | Build casse, runtime crash | Rollback possible + analyse profonde |

---

## Patterns d'Erreurs Frequents

### TypeScript
```typescript
// Erreur: Cannot find module '$lib/types/...'
// Fix: Utiliser $types alias
import type { X } from '$types/module';  // CORRECT
import type { X } from '$lib/types/module';  // INCORRECT
```

### Tauri IPC
```typescript
// Erreur: Invalid args for command
// Fix: camelCase en TypeScript, Tauri convertit vers snake_case
await invoke('my_command', { defaultModelId: 'x' });  // CORRECT
await invoke('my_command', { default_model_id: 'x' });  // INCORRECT
```

### Rust Serde
```rust
// Erreur: Failed to deserialize
// Fix: Verifier que tous les champs ont Serialize/Deserialize
#[derive(Debug, Clone, Serialize, Deserialize)]  // Toujours les deux
pub struct MyStruct { ... }
```

### SurrealDB
```rust
// Erreur: Meta::id not found
// Fix: Utiliser meta::id(id) pour extraire UUID propre
"SELECT meta::id(id) AS id, name FROM table"  // CORRECT
"SELECT * FROM table"  // Retourne ID avec angle brackets
```

---

## Outils MCP

### Serena (prioritaire)
- **Diagnostic**: `get_symbols_overview()` + `find_symbol(include_body=true)`
- **Impact**: `find_referencing_symbols()`
- **Edition**: `replace_symbol_body()` pour corrections symboliques
- **Memoire**: `write_memory()` pour patterns identifies

### Sequential (erreurs complexes)
- Raisonnement structure pour root cause analysis
- Elimination methodique d'hypotheses
- Documentation du processus de diagnostic

### Context7 (si erreur framework)
- Patterns officiels Svelte/Tauri/SurrealDB
- Verification best practices
- API correcte des dependances

---

## Checklist Final

Avant de marquer termine:

- [ ] TOUTES les erreurs initiales corrigees
- [ ] Root cause identifiee pour chaque erreur
- [ ] Aucune nouvelle regression introduite
- [ ] Validation frontend: lint + check
- [ ] Validation backend: clippy + test + build
- [ ] Tests de regression ajoutes (si bug)
- [ ] Commit avec message descriptif
- [ ] Rapport sauvegarde (`docs/corrections/`)
- [ ] Serena memory (si correction significative)

---

**RAPPEL**: Jamais de quick fix. Comprendre -> Diagnostiquer -> Corriger -> Valider -> Documenter.
