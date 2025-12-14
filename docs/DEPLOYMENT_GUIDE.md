# Guide Deploiement

> Build, packaging et distribution multi-OS

## Vue d'Ensemble

**Version actuelle** : 0.9.0-beta
**Strategie** : Linux → macOS → Windows (progressif)
**Format** : AppImage, .deb (Linux), .dmg (macOS prevu), .msi (Windows prevu)
**CI/CD** : Non configure (prevu)
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

### GitHub Actions - Build Multi-OS

**Contrainte importante** : Tauri ne supporte PAS la cross-compilation.
Chaque OS doit etre builde sur son propre runner.

#### Etape 1: Creer le workflow

Creer `.github/workflows/release.yml` :

```yaml
name: Release Build

on:
  push:
    tags:
      - 'v*'
  workflow_dispatch:  # Permet lancement manuel

permissions:
  contents: write

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          # Linux
          - platform: ubuntu-22.04
            target: ''
            bundle_targets: 'appimage,deb'
          # macOS Universal (Intel + Apple Silicon)
          - platform: macos-latest
            target: 'universal-apple-darwin'
            bundle_targets: 'dmg'
          # Windows
          - platform: windows-latest
            target: ''
            bundle_targets: 'msi'

    runs-on: ${{ matrix.platform }}

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Linux dependencies
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev \
            libappindicator3-dev \
            librsvg2-dev \
            patchelf

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'npm'

      - name: Install Rust stable
        uses: dtolnay/rust-action@stable
        with:
          targets: ${{ matrix.platform == 'macos-latest' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: './src-tauri -> target'

      - name: Install frontend dependencies
        run: npm ci

      - name: Build Tauri app
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: 'Zileo Chat ${{ github.ref_name }}'
          releaseBody: |
            ## Installation

            | OS | Fichier | Instructions |
            |----|---------|--------------|
            | Linux | `.AppImage` | `chmod +x *.AppImage && ./*.AppImage` |
            | Linux (Debian) | `.deb` | `sudo dpkg -i *.deb` |
            | macOS | `.dmg` | Double-clic, glisser dans Applications |
            | Windows | `.msi` | Double-clic, suivre assistant |

            ## Checksums
            Voir `SHA256SUMS` pour verifier l'integrite.
          releaseDraft: true
          prerelease: false
          args: ${{ matrix.target && format('--target {0}', matrix.target) || '' }}

      - name: Generate checksums (Linux)
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          cd src-tauri/target/release/bundle
          sha256sum appimage/*.AppImage deb/*.deb > SHA256SUMS-linux.txt
          cat SHA256SUMS-linux.txt

      - name: Upload checksums
        if: matrix.platform == 'ubuntu-22.04'
        uses: softprops/action-gh-release@v1
        with:
          files: src-tauri/target/release/bundle/SHA256SUMS-linux.txt
          draft: true
```

#### Etape 2: Mettre a jour tauri.conf.json

Modifier `bundle.targets` pour supporter tous les OS :

```json
{
  "bundle": {
    "active": true,
    "targets": "all",
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

#### Etape 3: Creer une release

```bash
# Mettre a jour les versions
# package.json, tauri.conf.json, Cargo.toml doivent avoir la meme version

# Creer et pousser le tag
git tag v0.9.0-beta
git push origin v0.9.0-beta
```

GitHub Actions va automatiquement :
1. Builder sur Linux, macOS et Windows en parallele
2. Creer une draft release avec tous les artifacts
3. Generer les checksums

#### Etape 4: Publier

1. Aller sur GitHub → Releases
2. Verifier la draft release
3. Editer les release notes si necessaire
4. Cliquer "Publish release"

### Workflow Validate (CI sur chaque push)

Creer `.github/workflows/validate.yml` pour validation continue :

```yaml
name: Validate

on:
  push:
    branches: [main, 'feature/**']
  pull_request:
    branches: [main]

jobs:
  lint-and-test:
    runs-on: ubuntu-22.04

    steps:
      - uses: actions/checkout@v4

      - name: Install Linux dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'npm'

      - name: Install Rust stable
        uses: dtolnay/rust-action@stable

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: './src-tauri -> target'

      - name: Install dependencies
        run: npm ci

      # Frontend checks
      - name: Lint (ESLint)
        run: npm run lint

      - name: Type check (svelte-check)
        run: npm run check

      - name: Frontend tests
        run: npm run test

      # Backend checks
      - name: Format check
        run: cargo fmt --manifest-path src-tauri/Cargo.toml --check

      - name: Clippy
        run: cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

      - name: Backend tests
        run: cargo test --manifest-path src-tauri/Cargo.toml
```

### Resume des Workflows

| Workflow | Declencheur | Action |
|----------|-------------|--------|
| `validate.yml` | Push/PR sur main, feature/* | Lint + Tests |
| `release.yml` | Tag v* | Build 3 OS + Release |

### Alternatives sans GitHub Actions

Si tu preferes builder manuellement sur chaque OS :

**Sur Windows (VM ou machine physique)** :
```powershell
# Installer prerequisites
winget install Microsoft.VisualStudio.2022.BuildTools
winget install Rustlang.Rustup

rustup default stable-msvc
npm ci
npm run tauri build
# Output: src-tauri/target/release/bundle/msi/*.msi
```

**Sur macOS (Mac physique ou MacStadium)** :
```bash
xcode-select --install
npm ci
npm run tauri build -- --target universal-apple-darwin
# Output: src-tauri/target/universal-apple-darwin/release/bundle/dmg/*.dmg
```

### Services Cloud pour macOS

| Service | Usage | Cout |
|---------|-------|------|
| GitHub Actions | 3000 min/mois (gratuit public) | Gratuit |
| MacStadium | Mac dedies | ~$99/mois |
| AWS EC2 Mac | Mac a la demande | ~$1/heure |
| Codemagic | CI/CD mobile/desktop | Gratuit tier dispo |

---

## Distribution

### GitHub Releases (Manuel)

**Creation Release** :
1. Tag version : `git tag v0.9.0-beta`
2. Push tag : `git push origin v0.9.0-beta`
3. Build local : `npm run tauri:build`
4. Creer release manuellement sur GitHub
5. Upload artifacts

**Assets attendus** :
- `zileo-chat_0.9.0-beta_amd64.AppImage` (Linux)
- `zileo-chat_0.9.0-beta_amd64.deb` (Linux)
- `zileo-chat_0.9.0-beta_x64.dmg` (macOS - prevu)
- `zileo-chat_0.9.0-beta_x64.msi` (Windows - prevu)

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
- [ ] Lint OK : `npm run lint` + `cargo clippy`
- [ ] Version synchronisee (package.json + tauri.conf.json + Cargo.toml)
- [ ] Changelog mis a jour
- [ ] Security audit : `cargo audit` (si configure)

### Release (Manuel - CI/CD non configure)
- [ ] Build local : `npm run tauri:build`
- [ ] Test manuel de l'AppImage/deb
- [ ] Tag cree : `git tag v0.x.y && git push origin v0.x.y`
- [ ] Checksums generes : `sha256sum zileo-chat_* > SHA256SUMS`
- [ ] GitHub Release creee manuellement
- [ ] Artifacts uploades

### Post-Release
- [ ] Test installation sur machine propre
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
