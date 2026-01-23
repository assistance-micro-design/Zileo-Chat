# Guide Deploiement

> Build, packaging et distribution multi-OS

## Vue d'Ensemble

**Version actuelle** : 0.9.1-beta
**Strategie** : Linux + macOS + Windows (parallel via GitHub Actions)
**Format** : AppImage, .deb (Linux), .dmg (macOS), .msi (Windows)
**CI/CD** : GitHub Actions (workflows dans `.github/workflows/`)
**Auto-updates** : Non configure (prevu v1.5)

---

## Prérequis Build

### Linux (Ubuntu/Debian)
```bash
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

### macOS
```bash
xcode-select --install
```

**Requis** : macOS 10.15+ (Catalina)

### Windows
**Requis** :
- Visual Studio Build Tools 2019+
- WebView2 Runtime (installé auto si absent)

---

## Configuration Build

### tauri.conf.json (Configuration Reelle)

```json
{
  "productName": "Zileo Chat",
  "version": "0.9.0-beta",
  "identifier": "com.zileo.chat",
  "build": {
    "frontendDist": "../build",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "windows": [{ "title": "Zileo Chat", "width": 1200, "height": 800 }],
    "security": {
      "csp": "default-src 'self'; style-src 'self' 'unsafe-inline'"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["appimage", "deb"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.ico",
      "icons/icon.icns"
    ]
  }
}
```

**Note** : Les icons incluent aussi `Zileo-icon.png` (branding custom).

---

## Build Local

### Scripts Disponibles (package.json)

```bash
npm run tauri:dev      # Developpement avec HMR
npm run tauri:build    # Build production
```

### Development Build
```bash
npm run tauri:build
```

**Output** : `src-tauri/target/release/bundle/`

### Release Build
```bash
npm run tauri:build
```

**Note** : Le profil release est utilise par defaut. LTO n'est pas encore configure dans Cargo.toml.

---

## Packaging par OS

### Linux (Configure)

Les targets Linux sont configures dans tauri.conf.json : `["appimage", "deb"]`

#### AppImage (Universal)
```bash
npm run tauri:build
```

**Avantages** : Pas installation, portable, compatible toutes distros
**Output** : `zileo-chat_0.9.0-beta_amd64.AppImage`

#### .deb (Debian/Ubuntu)
Build produit automatiquement les deux formats.

**Installation** :
```bash
sudo dpkg -i zileo-chat_0.9.0-beta_amd64.deb
```

**Output** : `zileo-chat_0.9.0-beta_amd64.deb`

---

### macOS (Prevu)

> **Statut** : Non configure. Necessite ajout de "dmg" dans bundle.targets.

#### .dmg (Image Disque)
```bash
# Ajouter "dmg" dans tauri.conf.json bundle.targets
npm run tauri:build
```

**Code Signing** (requis distribution publique) :
```bash
codesign --sign "Developer ID Application: Your Name" \
  src-tauri/target/release/bundle/macos/Zileo\ Chat.app
```

**Output prevu** : `zileo-chat_0.9.0-beta_x64.dmg`

---

### Windows (Guide Complet)

#### Prerequisites Windows

**1. Visual Studio Build Tools 2022** (obligatoire)

```powershell
# Via winget (recommande)
winget install Microsoft.VisualStudio.2022.BuildTools

# OU telecharger: https://visualstudio.microsoft.com/visual-cpp-build-tools/
```

Lors de l'installation, selectionner:
- "Desktop development with C++"
- Windows 10/11 SDK

**2. Rust avec toolchain MSVC**

```powershell
# Installer Rust
winget install Rustlang.Rustup

# OU telecharger: https://rustup.rs/

# Configurer le toolchain MSVC (IMPORTANT)
rustup default stable-msvc

# Verifier
rustup show
# Doit afficher: stable-x86_64-pc-windows-msvc (default)
```

**3. Node.js 20+**

```powershell
winget install OpenJS.NodeJS.LTS

# Verifier
node --version   # >= 20.x
```

**4. WebView2 Runtime**

Normalement pre-installe sur Windows 10/11 recent. Si absent:
```powershell
winget install Microsoft.EdgeWebView2Runtime
```

#### Build Manuel Windows

```powershell
# 1. Cloner le projet
git clone https://github.com/xxx/Zileo-Chat-3.git
cd Zileo-Chat-3

# 2. Installer les dependances
npm ci

# 3. Valider le projet (optionnel mais recommande)
npm run check
npm run lint

# 4. Build release
npm run tauri build

# 5. Le MSI est genere dans:
explorer "src-tauri\target\release\bundle\msi"
```

**Output** :
```
src-tauri/target/release/bundle/
├── msi/
│   └── zileo-chat_0.9.0-beta_x64-setup.msi    # Installer MSI
└── nsis/
    └── zileo-chat_0.9.0-beta_x64-setup.exe    # Installer NSIS (alternative)
```

#### Script Automatise Windows

Un script PowerShell est fourni pour automatiser le setup complet:

```powershell
# Telecharger et executer le script
cd Zileo-Chat-3

# Setup complet (installe prerequisites si manquants + build)
powershell -ExecutionPolicy Bypass -File scripts/setup-windows.ps1

# Build uniquement (prerequisites deja installes)
powershell -ExecutionPolicy Bypass -File scripts/setup-windows.ps1 -SkipPrerequisites

# Mode developpement (lance l'app sans build release)
powershell -ExecutionPolicy Bypass -File scripts/setup-windows.ps1 -DevMode
```

**Options du script:**
| Option | Description |
|--------|-------------|
| (aucune) | Setup complet + build release |
| `-SkipPrerequisites` | Passe la verification/installation des prerequisites |
| `-BuildOnly` | Build uniquement, pas de verification |
| `-DevMode` | Lance `npm run tauri dev` au lieu de build |

#### Troubleshooting Windows

**Erreur: "MSVC not found"**
```powershell
# Verifier le toolchain
rustup show

# Si "gnu" au lieu de "msvc":
rustup default stable-msvc
```

**Erreur: "link.exe not found"**
```powershell
# Reinstaller Visual Studio Build Tools avec les composants C++
winget install Microsoft.VisualStudio.2022.BuildTools `
  --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

**Erreur: "WebView2 not found"**
```powershell
winget install Microsoft.EdgeWebView2Runtime
```

**Build tres lent (premiere fois)**

Normal! La premiere compilation Rust telecharge et compile toutes les dependances (~10-15 min).
Les builds suivants sont beaucoup plus rapides (~2-5 min).

Pour accelerer:
```powershell
# Utiliser plus de CPU cores
$env:CARGO_BUILD_JOBS = "8"
npm run tauri build
```

**Erreur: "npm ci failed"**
```powershell
# Nettoyer et reinstaller
npm cache clean --force
Remove-Item -Recurse -Force node_modules
npm ci
```

#### Workflow Complet Windows

```powershell
# === SETUP INITIAL (une seule fois) ===

# 1. Installer prerequisites
winget install Microsoft.VisualStudio.2022.BuildTools
winget install Rustlang.Rustup
winget install OpenJS.NodeJS.LTS

# 2. Configurer Rust
rustup default stable-msvc

# 3. Cloner le projet
git clone https://github.com/xxx/Zileo-Chat-3.git
cd Zileo-Chat-3

# 4. Installer dependances npm
npm ci


# === DEVELOPPEMENT (quotidien) ===

# Lancer en mode dev (hot reload)
npm run tauri dev


# === BUILD RELEASE ===

# Generer l'installeur MSI
npm run tauri build

# Installer pour tester
Start-Process "src-tauri\target\release\bundle\msi\zileo-chat_0.9.0-beta_x64-setup.msi"
```

#### Checklist Pre-Build Windows

```powershell
# Verifier tous les prerequisites
rustc --version          # >= 1.80
rustup show              # stable-x86_64-pc-windows-msvc
node --version           # >= 20.x
npm --version            # >= 10.x

# Verifier Visual Studio Build Tools
# Ouvrir "Developer PowerShell for VS 2022" - si ca marche, c'est OK
```

---

## CI/CD Pipeline

### Validation Locale

```bash
# Frontend validation
npm run lint              # ESLint
npm run check             # svelte-check + TypeScript
npm run test              # Vitest unit tests

# Backend validation
cd src-tauri
cargo fmt --check         # Format verification
cargo clippy -- -D warnings  # Linting
cargo test                # Unit tests
```

### GitHub Actions - Configuration Active

**Statut** : Configure et fonctionnel dans `.github/workflows/`

**Contrainte importante** : Tauri ne supporte PAS la cross-compilation.
Chaque OS est builde sur son propre runner en parallele.

#### Workflow Release (`.github/workflows/release.yml`)

**Declencheur** : Push de tag `v*` ou `workflow_dispatch` (manuel)

**Fonctionnement** :
- Build parallele sur 3 OS (Ubuntu, macOS, Windows)
- Cree une release draft avec tous les artifacts
- Detecte automatiquement les pre-releases (beta, alpha)
- Genere les checksums SHA256

**Artifacts generes** :
- `zileo-chat_X.Y.Z_amd64.AppImage` (Linux)
- `zileo-chat_X.Y.Z_amd64.deb` (Linux Debian/Ubuntu)
- `zileo-chat_X.Y.Z_x64.dmg` (macOS)
- `zileo-chat_X.Y.Z_x64.msi` (Windows)
- `SHA256SUMS`

#### Workflow Validate (`.github/workflows/validate.yml`)

**Declencheur** : Push sur `main`/`develop`, PR vers `main`

**Jobs paralleles** :
1. **Frontend** : `npm run lint` + `npm run check` + `npm run test`
2. **Backend** : `cargo fmt --check` + `cargo clippy` + `cargo test`

#### Resume des Workflows

| Workflow | Declencheur | Action |
|----------|-------------|--------|
| `validate.yml` | Push/PR sur main/develop | Lint + Type check + Tests (frontend + backend) |
| `release.yml` | Tag `v*` ou manuel | Build 3 OS + Release draft |

### Creer une Release

#### Via GitHub Desktop (Recommande)

1. Verifier versions synchronisees (`package.json`, `Cargo.toml`, `tauri.conf.json`)
2. Commit final des changements
3. Aller dans **History**
4. Clic droit sur le commit → **Create Tag**
5. Nom : `v0.9.2-beta` (ou version suivante)
6. **Push origin** (inclut automatiquement le tag)

#### Via CLI

```bash
# Verifier que tout est propre
git status
npm run lint && npm run check
cd src-tauri && cargo clippy && cargo test

# Creer et pousser le tag
git tag v0.9.2-beta
git push origin v0.9.2-beta
```

#### Finalisation sur GitHub

1. Aller dans **Releases** → La draft release apparait
2. Verifier les artifacts (6 fichiers attendus)
3. Editer les release notes (generees automatiquement via labels)
4. Cliquer **Publish release**

### Configuration Dependabot

Fichier : `.github/dependabot.yml`

Mises a jour automatiques hebdomadaires pour :
- **npm** : Dependencies frontend
- **cargo** : Dependencies Rust
- **github-actions** : Actions CI (mensuel)

Les PRs de dependabot ont le label `skip-changelog` pour ne pas polluer les release notes.

### Templates GitHub

| Fichier | Usage |
|---------|-------|
| `.github/ISSUE_TEMPLATE/bug_report.md` | Template pour signaler des bugs |
| `.github/ISSUE_TEMPLATE/feature_request.md` | Template pour proposer des features |
| `.github/PULL_REQUEST_TEMPLATE.md` | Checklist pour les PR |
| `.github/release.yml` | Configuration auto-generation release notes |

### Labels pour Release Notes

Les release notes sont generees automatiquement selon les labels des PRs :

| Label | Section |
|-------|---------|
| `breaking-change`, `breaking` | Breaking Changes |
| `enhancement`, `feature` | New Features |
| `bug`, `fix` | Bug Fixes |
| `performance` | Performance |
| `documentation` | Documentation |
| `dependencies` | Exclu des notes |

---

## Distribution

### GitHub Releases (Automatise)

**Processus** :
1. Creer tag `v*` (via GitHub Desktop ou CLI)
2. Push le tag → GitHub Actions se declenche
3. Build automatique sur 3 OS en parallele
4. Release draft creee avec tous les artifacts
5. Publier manuellement apres verification

**Assets generes automatiquement** :
- `zileo-chat_X.Y.Z_amd64.AppImage` (Linux Universal)
- `zileo-chat_X.Y.Z_amd64.deb` (Linux Debian/Ubuntu)
- `zileo-chat_X.Y.Z_x64.dmg` (macOS)
- `zileo-chat_X.Y.Z_x64.msi` (Windows)
- `SHA256SUMS` (Checksums)

### Checksums

**Generation** :
```bash
sha256sum zileo-chat_* > SHA256SUMS
```

**Verification utilisateur** :
```bash
sha256sum -c SHA256SUMS
```

---

## Auto-Updates (Prevu v1.5)

> **Statut** : Non configure. La configuration updater n'existe pas dans tauri.conf.json.

### Configuration Requise (Quand Active)

Ajouter dans `tauri.conf.json` :
```json
{
  "plugins": {
    "updater": {
      "active": true,
      "endpoints": ["https://releases.zileo.com/{{target}}/{{current_version}}"],
      "dialog": true,
      "pubkey": "PUBLIC_KEY_HERE"
    }
  }
}
```

### Commandes Tauri (Reference)

```bash
# Generer keypair
npx tauri signer generate

# Signer release
npx tauri signer sign zileo-chat_0.9.0-beta_amd64.AppImage
```

Voir documentation Tauri : https://v2.tauri.app/plugin/updater/

---

## Environnements

### Development
- **Build** : Debug mode via `npm run tauri:dev`
- **DB** : SurrealDB embedded (RocksDB local)
- **Logs** : Configure via tracing-subscriber (env-filter)

### Production
- **Build** : Release via `npm run tauri:build`
- **DB** : SurrealDB embedded (RocksDB)
- **Security** : API keys via keyring + AES-256, CSP configure

**Note** : Pas d'environnement staging separe (application desktop).

---

## Variables d'Environnement Critiques

### SURREAL_SYNC_DATA (CRITICAL - Production)

**IMPORTANT**: Cette variable DOIT etre configuree en production pour garantir la securite des donnees.

```bash
# Production (OBLIGATOIRE)
export SURREAL_SYNC_DATA=true

# Development (optionnel, defaut: false)
export SURREAL_SYNC_DATA=false
```

**Impact**:
- Sans cette variable, RocksDB/SurrealKV n'est PAS crash-safe
- En cas de crash ou coupure electrique, les donnees peuvent etre corrompues
- Active la synchronisation WAL (Write-Ahead Log) pour durabilite

**Reference**: [SurrealDB Performance Best Practices](https://surrealdb.com/docs/surrealdb/reference-guide/performance-best-practices)

### SURREAL_LOG (Production)

```bash
# Production (performance optimale)
export SURREAL_LOG=error

# Development (debug)
export SURREAL_LOG=debug
```

**Impact**: Le logging verbose a un impact significatif sur les performances en production.

---

## Monitoring Post-Release (Prevu)

> **Statut** : Aucun monitoring configure. Sentry n'est pas integre.

### Crash Reports (Prevu)

Pour integrer Sentry, ajouter dans Cargo.toml :
```toml
sentry = "0.32"
```

### Analytics

**Approche recommandee** : Telemetry opt-in (privacy-first)
- Version OS utilisée
- Version app
- Crashes (anonymisés)
- Pas données utilisateur

### Feedback

**GitHub Issues** : Bug reports + feature requests
**Discord/Forum** : Support communauté

---

## Rollback Strategy

### Probleme Post-Release

**1. Identifier version stable precedente**
```bash
git tag -l "v*"
```

**2. Supprimer tag problematique** (exemple)
```bash
git tag -d v0.1.1
git push origin :refs/tags/v0.1.1
```

**3. Republier version stable** (exemple)
```bash
git tag v0.9.0-beta-hotfix
git push origin v0.9.0-beta-hotfix
```

**4. Communiquer** : Release notes + notification utilisateurs

---

## Checklist Release

### Pre-Release
- [ ] Tests passent : `npm run test` + `cargo test`
- [ ] Lint OK : `npm run lint` + `cargo clippy -- -D warnings`
- [ ] Type check OK : `npm run check`
- [ ] Format OK : `cargo fmt --check`
- [ ] Version synchronisee (`package.json` + `tauri.conf.json` + `Cargo.toml`)
- [ ] Changelog mis a jour

### Release (Automatise via GitHub Actions)
- [ ] Tag cree et pushe : `git tag vX.Y.Z && git push origin vX.Y.Z`
- [ ] Workflow release termine sans erreur
- [ ] 6 artifacts presents dans la draft release
- [ ] Release notes verifiees et editees si besoin
- [ ] Release publiee

### Post-Release
- [ ] Test installation sur machine propre (au moins 1 OS)
- [ ] Verification checksums SHA256
- [ ] Feedback utilisateurs collecte
- [ ] Hotfix si critique (rollback si necessaire)

---

## Troubleshooting

### Build Failed Linux

**Error** : Missing webkit2gtk
**Solution** : Installer deps système (voir Prérequis Build)

### Build Failed macOS

**Error** : Code signing failed
**Solution** : Vérifier certificat Developer ID valide

### Build Failed Windows

**Error** : WebView2 not found
**Solution** : Installer WebView2 Runtime manuellement

### Large Binary Size

**Solutions** :
1. Strip symbols : `strip target/release/zileo-chat`
2. Enable LTO : Ajouter `[profile.release] lto = "thin"` dans Cargo.toml (non configure)
3. Optimize dependencies : Exclude unused features dans Cargo.toml

---

## Références

**Tauri Build** : https://v2.tauri.app/develop/build/
**Tauri Updater** : https://v2.tauri.app/plugin/updater/
**Code Signing** : https://v2.tauri.app/distribute/sign/
**GitHub Actions** : https://docs.github.com/actions
