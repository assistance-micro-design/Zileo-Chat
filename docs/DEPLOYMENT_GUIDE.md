# Guide Deploiement

> Build, packaging et distribution multi-OS

## Vue d'Ensemble

**Version actuelle** : 0.1.0
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
  "version": "0.1.0",
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
**Output** : `zileo-chat_0.1.0_amd64.AppImage`

#### .deb (Debian/Ubuntu)
Build produit automatiquement les deux formats.

**Installation** :
```bash
sudo dpkg -i zileo-chat_0.1.0_amd64.deb
```

**Output** : `zileo-chat_0.1.0_amd64.deb`

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

**Output prevu** : `zileo-chat_0.1.0_x64.dmg`

---

### Windows (Prevu Phase 2)

> **Statut** : Non configure. Necessite ajout de "msi" dans bundle.targets.

#### .msi (Installer)
```bash
# Ajouter "msi" dans tauri.conf.json bundle.targets
npm run tauri:build
```

**Output prevu** : `zileo-chat_0.1.0_x64.msi`

---

## CI/CD Pipeline (Prevu)

> **Statut** : Aucun workflow CI/CD configure actuellement.
> Les fichiers `.github/workflows/` et `.gitlab-ci.yml` n'existent pas.

### Validation Locale (Disponible)

En attendant CI/CD, utiliser les commandes locales :

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

### Structure CI/CD Recommandee

Quand les workflows seront crees :

**Branches** :
- `feature/*` : Lint + Tests
- `main` : Build multi-OS
- `tags (v*)` : Release publique

**Jobs prevus** :
1. Lint & Test (ubuntu-latest)
2. Build Linux (appimage + deb)
3. Build macOS (dmg) - Phase 1.5
4. Build Windows (msi) - Phase 2

---

## Distribution

### GitHub Releases (Manuel)

**Creation Release** :
1. Tag version : `git tag v0.1.0`
2. Push tag : `git push origin v0.1.0`
3. Build local : `npm run tauri:build`
4. Creer release manuellement sur GitHub
5. Upload artifacts

**Assets attendus** :
- `zileo-chat_0.1.0_amd64.AppImage` (Linux)
- `zileo-chat_0.1.0_amd64.deb` (Linux)
- `zileo-chat_0.1.0_x64.dmg` (macOS - prevu)
- `zileo-chat_0.1.0_x64.msi` (Windows - prevu)

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
npx tauri signer sign zileo-chat_0.1.0_amd64.AppImage
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
git tag v0.1.0-hotfix
git push origin v0.1.0-hotfix
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
