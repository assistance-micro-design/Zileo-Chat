# Guide Déploiement

> Build, packaging et distribution multi-OS

## Vue d'Ensemble

**Stratégie** : Linux → macOS → Windows (progressif)
**Format** : AppImage, .deb (Linux), .dmg (macOS), .msi (Windows)
**Auto-updates** : Non v1, prévu v1.5

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

### tauri.conf.json

**Identifier** : Unique bundle ID
```json
{
  "identifier": "com.zileo.chat3"
}
```

**Version** : Synchroniser avec package.json
```json
{
  "version": "1.0.0"
}
```

**Bundle Targets**
```json
{
  "bundle": {
    "active": true,
    "targets": ["appimage", "deb"],  // Linux
    "identifier": "com.zileo.chat3",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",  // macOS
      "icons/icon.ico"    // Windows
    ]
  }
}
```

---

## Build Local

### Development Build
```bash
npm run tauri build
```

**Output** : `src-tauri/target/release/bundle/`

### Release Build (Optimisé)
```bash
npm run tauri build -- --release
```

**Optimisations** :
- Strip symbols (taille réduite)
- LTO (Link-Time Optimization)
- Codegen optimized

---

## Packaging par OS

### Linux

#### AppImage (Universal)
```bash
npm run tauri build -- --target appimage
```

**Avantages** : Pas installation, portable, compatible toutes distros
**Output** : `zileo-chat-3_1.0.0_amd64.AppImage`

#### .deb (Debian/Ubuntu)
```bash
npm run tauri build -- --target deb
```

**Installation** :
```bash
sudo dpkg -i zileo-chat-3_1.0.0_amd64.deb
```

**Output** : `zileo-chat-3_1.0.0_amd64.deb`

---

### macOS

#### .dmg (Image Disque)
```bash
npm run tauri build -- --target dmg
```

**Code Signing** (requis distribution publique) :
```bash
codesign --sign "Developer ID Application: Your Name" \
  src-tauri/target/release/bundle/macos/Zileo\ Chat\ 3.app
```

**Notarization** (Apple requirement) :
```bash
xcrun notarytool submit zileo-chat-3.dmg \
  --apple-id your@email.com \
  --password app-specific-password \
  --team-id TEAMID
```

**Output** : `zileo-chat-3_1.0.0_x64.dmg`

---

### Windows

#### .msi (Installer)
```bash
npm run tauri build -- --target msi
```

**Code Signing** (optionnel mais recommandé) :
```powershell
signtool sign /f certificate.pfx /p password zileo-chat-3_1.0.0_x64.msi
```

**Output** : `zileo-chat-3_1.0.0_x64.msi`

---

## CI/CD Pipeline

### GitHub Actions

**Workflow** : `.github/workflows/build.yml`

#### Branches
- **feature/** : Linting + Tests
- **main** : Build multi-OS + Tests E2E
- **tags (v*)** : Release publique

#### Jobs

**1. Lint & Test**
```yaml
- runs: npm run lint
- runs: npm run test
- runs: cargo clippy
- runs: cargo test
```

**2. Build Linux**
```yaml
- os: ubuntu-latest
- runs: npm run tauri build -- --target appimage deb
- upload: artifacts
```

**3. Build macOS**
```yaml
- os: macos-latest
- runs: npm run tauri build -- --target dmg
- code-sign: if secrets.MACOS_CERTIFICATE
- upload: artifacts
```

**4. Build Windows** (Phase 2)
```yaml
- os: windows-latest
- runs: npm run tauri build -- --target msi
- code-sign: if secrets.WINDOWS_CERTIFICATE
- upload: artifacts
```

**5. Release**
```yaml
- if: startsWith(github.ref, 'refs/tags/v')
- create: GitHub Release
- upload: All artifacts
```

---

### GitLab CI

**Workflow** : `.gitlab-ci.yml`

#### Stages
1. `test` : Linting, tests unitaires
2. `build` : Compilation multi-OS (runners spécifiques)
3. `deploy` : Publication artifacts

#### Runners
- **Linux** : Docker avec deps Tauri
- **macOS** : Shell runner macOS 12+
- **Windows** : Shell runner Windows Server

---

## Distribution

### GitHub Releases

**Création Release**
1. Tag version : `git tag v1.0.0`
2. Push tag : `git push origin v1.0.0`
3. CI/CD auto-crée release avec artifacts

**Assets** :
- `zileo-chat-3_1.0.0_amd64.AppImage`
- `zileo-chat-3_1.0.0_amd64.deb`
- `zileo-chat-3_1.0.0_x64.dmg`
- `zileo-chat-3_1.0.0_x64.msi` (Phase 2)

### Checksums

**Génération** :
```bash
sha256sum zileo-chat-3* > SHA256SUMS
```

**Vérification utilisateur** :
```bash
sha256sum -c SHA256SUMS
```

---

## Auto-Updates (v1.5)

### Tauri Updater

**Config** : `tauri.conf.json`
```json
{
  "updater": {
    "active": true,
    "endpoints": [
      "https://releases.zileo.com/{{target}}/{{current_version}}"
    ],
    "dialog": true,
    "pubkey": "PUBLIC_KEY_HERE"
  }
}
```

### Update Manifest

**Format JSON** : `latest.json`
```json
{
  "version": "1.0.1",
  "notes": "Bug fixes and performance improvements",
  "pub_date": "2025-11-23T10:00:00Z",
  "platforms": {
    "linux-x86_64": {
      "signature": "SIGNATURE",
      "url": "https://releases.zileo.com/zileo-chat-3_1.0.1_amd64.AppImage"
    }
  }
}
```

### Signing

**Generate keypair** :
```bash
tauri signer generate
```

**Sign release** :
```bash
tauri signer sign zileo-chat-3.AppImage
```

---

## Environnements

### Development
- **Build** : Debug mode, symbols inclus
- **DB** : SurrealDB embedded local
- **Logs** : Verbose (DEBUG level)

### Staging
- **Build** : Release optimized
- **DB** : SurrealDB server (staging)
- **Logs** : INFO level
- **Testing** : E2E complets

### Production
- **Build** : Release + strip + LTO
- **DB** : SurrealDB embedded (desktop app)
- **Logs** : WARN/ERROR uniquement
- **Security** : API keys encrypted, CSP strict

---

## Monitoring Post-Release

### Crash Reports

**Sentry Integration** (optionnel)
```rust
// src-tauri/src/main.rs
sentry::init(("DSN", sentry::ClientOptions {
    release: Some(env!("CARGO_PKG_VERSION").into()),
    ..Default::default()
}));
```

### Analytics

**Telemetry Opt-in** (privacy-first)
- Version OS utilisée
- Version app
- Crashes (anonymisés)
- Pas données utilisateur

### Feedback

**GitHub Issues** : Bug reports + feature requests
**Discord/Forum** : Support communauté

---

## Rollback Strategy

### Problème Post-Release

**1. Identifier version stable précédente**
```bash
git tag -l "v*"
```

**2. Revert tag**
```bash
git tag -d v1.0.1
git push origin :refs/tags/v1.0.1
```

**3. Republier version stable**
```bash
git tag v1.0.0-hotfix
git push origin v1.0.0-hotfix
```

**4. Communiquer** : Release notes + notification utilisateurs

---

## Checklist Release

### Pre-Release
- [ ] Tests passent (unitaires + E2E)
- [ ] Version bumped (package.json + tauri.conf.json)
- [ ] Changelog updated
- [ ] Security audit (cargo audit)
- [ ] Dependencies updated

### Release
- [ ] Tag créé et pushed
- [ ] CI/CD builds réussis
- [ ] Artifacts téléchargés et testés manuellement
- [ ] Checksums générés
- [ ] GitHub Release créée

### Post-Release
- [ ] Monitoring crashes 24h
- [ ] Feedback utilisateurs collecté
- [ ] Hotfix si critique (rollback si nécessaire)

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
1. Strip symbols : `strip target/release/zileo-chat-3`
2. Enable LTO : `lto = "thin"` dans Cargo.toml
3. Optimize dependencies : Exclude unused features

---

## Références

**Tauri Build** : https://v2.tauri.app/develop/build/
**Tauri Updater** : https://v2.tauri.app/plugin/updater/
**Code Signing** : https://v2.tauri.app/distribute/sign/
**GitHub Actions** : https://docs.github.com/actions
