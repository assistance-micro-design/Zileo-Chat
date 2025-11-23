# Guide Démarrage - Zileo Chat 3

> Setup environnement développement et premier workflow

## Prérequis

### Versions Minimales
- **Node.js** : 20.19+ ou 22.12+ (Vite 7 requis)
- **Rust** : 1.80.1+ (SurrealDB SDK requis)
- **npm/pnpm/yarn** : Latest stable

### Outils Optionnels
- **Docker** : Pour MCP servers locaux
- **Python 3.10+** : Si MCP servers Python (uvx)

### Vérification
```bash
node --version   # >= 20.19
rustc --version  # >= 1.91.1
cargo --version  # >= 1.91.1
```

---

## Installation

### 1. Cloner Projet
```bash
git clone <repo_url>
cd zileo-chat-3
```

### 2. Installer Dépendances Frontend
```bash
npm install
# ou
pnpm install
```

### 3. Installer Tauri CLI
```bash
cargo install tauri-cli --version ^2.9
```

### 4. Configurer SurrealDB

**Option A : Embedded (Recommandé Dev)**
- Automatique, pas de setup
- RocksDB local dans `appDataDir()`

**Option B : Server Local**
```bash
surreal start --user root --pass root memory
```

---

## Configuration

### 1. Tauri Config

Vérifier `src-tauri/tauri.conf.json` :
- `identifier` : Unique bundle ID
- `allowlist` : Permissions IPC commands
- `security.csp` : Content Security Policy

---

## Développement

### 1. Mode Dev (Frontend + Backend)
```bash
npm run tauri dev
```

**Résultat** :
- Frontend : Vite HMR actif (http://localhost:5173)
- Backend : Rust compile + watch mode
- Window : Application Tauri ouverte

### 2. Frontend Seul
```bash
npm run dev
```
Utile pour design UI sans backend.

### 3. Backend Seul
```bash
cd src-tauri
cargo run
```
Test commands Rust isolé.

---

## Structure Projet

```
zileo-chat-3/
├─ src/                     # Frontend SvelteKit
│  ├─ routes/               # Pages
│  │  ├─ settings/          # Page Settings
│  │  └─ agent/             # Page Agent
│  ├─ lib/
│  │  ├─ components/        # Composants réutilisables
│  │  └─ stores/            # State management Svelte
│  └─ app.html              # Template HTML
│
├─ src-tauri/               # Backend Rust
│  ├─ src/
│  │  ├─ main.rs            # Entry point
│  │  ├─ commands/          # Tauri commands (IPC)
│  │  ├─ agents/            # Système multi-agents
│  │  ├─ llm/               # Rig.rs integration
│  │  ├─ mcp/               # MCP client/server
│  │  ├─ tools/             # Custom tools
│  │  └─ db/                # SurrealDB client
│  ├─ Cargo.toml            # Dépendances Rust
│  └─ tauri.conf.json       # Config Tauri
│
└─ docs/                    # Documentation
```

---

## Premier Workflow

### 1. Lancer Application
```bash
npm run tauri dev
```

### 2. Configuration Initiale

**Page Settings** :
1. **Providers** : Configurer Mistral (API key) et/ou Ollama (local)
2. **Models** : Sélectionner modèle
   - Mistral : mistral-large, mistral-medium
   - Ollama : llama3, mistral, codellama
3. **Theme** : Choisir Light/Dark
4. **Agents** : Vérifier agents par défaut (db_agent, api_agent)

**Note** : Toutes les API keys sont configurées via UI et stockées de manière sécurisée (Tauri secure storage + encryption)

### 3. Créer Workflow

**Page Agent** :
1. Click **+ New** (sidebar workflows)
2. Sélectionner agent : `db_agent`
3. Sélectionner provider : Mistral ou Ollama
4. Nommer workflow : "Test Query"
5. Envoyer message : "Query all users from database"

### 4. Observer Exécution

**Indicateurs Temps Réel** :
- Status workflow : ● Running
- Tokens : Update incrémental
- Tools : SurrealDBTool actif
- MCP : Si servers configurés

### 5. Validation (si Human-in-the-Loop)

Si mode validation activé :
1. Modal apparaît : "Validation Required"
2. Détails opération : Query DB avec params
3. Approve/Reject : Choisir action
4. Workflow continue après validation

---

## Configuration MCP Servers (Optionnel)

### Via UI Settings > MCP

**Exemple : serena (local Docker)**
```json
{
  "command": "docker",
  "args": ["run", "-i", "--rm", "serena-mcp"],
  "env": {}
}
```

**Exemple : context7 (SaaS)**
```json
{
  "command": "npx",
  "args": ["-y", "@context7/mcp-server"],
  "env": {
    "CONTEXT7_API_KEY": "${CONTEXT7_API_KEY}"
  }
}
```

**Test Connection** : Button "Test" → Status online/offline

---

## Debugging

### Frontend (SvelteKit)
- **DevTools** : F12 dans window Tauri
- **Console logs** : `console.log()`
- **Svelte Inspector** : Click composants (dev mode)

### Backend (Rust)
- **Logs** : `tracing` output dans terminal
- **Breakpoints** : VS Code avec `rust-analyzer`
- **Cargo test** : Tests unitaires `cargo test`

### Database (SurrealDB)
- **CLI** : `surreal sql` (mode interactif)
- **Queries manuelles** : Tester schemas/relations
- **Logs** : Activer `SURREAL_LOG=trace`

---

## Tests

### Frontend
```bash
npm run test        # Vitest unit tests
npm run test:e2e    # Playwright E2E
```

### Backend
```bash
cd src-tauri
cargo test          # Tests unitaires
cargo test --lib    # Tests library
```

---

## Build Production

### Dev Build
```bash
npm run tauri build
```

**Outputs** :
- Linux : `src-tauri/target/release/bundle/appimage/`
- macOS : `src-tauri/target/release/bundle/dmg/`
- Windows : `src-tauri/target/release/bundle/msi/`

### Release Build (CI/CD)
Voir [DEPLOYMENT_GUIDE.md](DEPLOYMENT_GUIDE.md)

---

## Troubleshooting

### Node.js Version Error
```
Error: Vite requires Node.js 20.19+ or 22.12+
```
**Solution** : Installer Node.js compatible, vérifier `node --version`

### Rust Compilation Error
```
error: package requires Rust 1.80.1+
```
**Solution** : `rustup update stable`

### SurrealDB Connection Failed
**Solution** :
1. Vérifier server running : `surreal version`
2. Mode embedded : Vérifier permissions `appDataDir()`
3. Mode server : Vérifier URL/credentials configuration

### Tauri Build Failed
**Solution** :
1. Clear cache : `cargo clean`
2. Rebuild : `npm run tauri build`
3. Check logs : `src-tauri/target/release/build/`

### MCP Server Offline
**Solution** :
1. Test command manuellement : `docker run -i --rm serena-mcp`
2. Vérifier Docker/NPX/UVX installé
3. Check logs Settings > MCP

---

## Next Steps

1. **Explorer Agents** : [MULTI_AGENT_ARCHITECTURE.md](MULTI_AGENT_ARCHITECTURE.md)
2. **Configurer Providers** : [MULTI_PROVIDER_SPECIFICATIONS.md](MULTI_PROVIDER_SPECIFICATIONS.md)
3. **Intégrer MCP** : [MCP_CONFIGURATION_GUIDE.md](MCP_CONFIGURATION_GUIDE.md)
4. **API Reference** : [API_REFERENCE.md](API_REFERENCE.md)

---

## Ressources

**Tauri** : https://v2.tauri.app/start
**SvelteKit** : https://kit.svelte.dev/docs
**SurrealDB** : https://surrealdb.com/docs
**Rig.rs** : https://rig.rs/getting-started
