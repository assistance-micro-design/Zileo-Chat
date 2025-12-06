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
npm run tauri:dev
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
│  ├─ routes/               # Pages (file-based routing)
│  │  ├─ +layout.svelte     # Layout global (theme, locale, onboarding)
│  │  ├─ +page.svelte       # Accueil (redirect vers /agent)
│  │  ├─ settings/          # Page Settings (9 sections)
│  │  └─ agent/             # Page Agent (chat principal)
│  ├─ lib/
│  │  ├─ components/        # 79 composants Svelte
│  │  │  ├─ ui/             # Composants atomiques (14)
│  │  │  ├─ layout/         # Layout (4)
│  │  │  ├─ chat/           # Chat UI (8)
│  │  │  ├─ workflow/       # Gestion workflows (14)
│  │  │  ├─ settings/       # Sections settings (13)
│  │  │  └─ onboarding/     # Assistant premier lancement (9)
│  │  ├─ stores/            # 14 stores Svelte
│  │  ├─ services/          # Couche business logic
│  │  └─ i18n.ts            # Internationalisation
│  ├─ types/                # Définitions TypeScript (22 fichiers)
│  ├─ messages/             # Traductions (en.json, fr.json)
│  └─ app.html              # Template HTML
│
├─ src-tauri/               # Backend Rust
│  ├─ src/
│  │  ├─ main.rs            # Entry point
│  │  ├─ commands/          # 18 modules (112 commandes Tauri)
│  │  ├─ agents/            # Système multi-agents
│  │  ├─ llm/               # Rig.rs integration (Mistral, Ollama)
│  │  ├─ mcp/               # MCP client/server
│  │  ├─ tools/             # 6 outils (Memory, Todo, Calculator, SubAgent)
│  │  ├─ models/            # Structs Rust (sync avec TS)
│  │  ├─ security/          # Keystore + validation
│  │  └─ db/                # SurrealDB client (17 tables)
│  ├─ Cargo.toml            # Dépendances Rust
│  └─ tauri.conf.json       # Config Tauri
│
└─ docs/                    # Documentation (16 fichiers)
```

---

## Premier Workflow

### 1. Lancer Application
```bash
npm run tauri:dev
```

### 2. Assistant Premier Lancement (Onboarding)

Au premier lancement, un assistant guide la configuration :
1. **Langue** : Sélection français/anglais
2. **Thème** : Light/Dark
3. **Provider** : Configuration API key (Mistral recommandé)
4. **Import** : Option d'importer une configuration existante

### 3. Configuration Avancée

**Page Settings** (9 sections) :
1. **Providers** : Configurer Mistral (API key) et/ou Ollama (local)
2. **Models** : Gérer modèles LLM (builtin + custom)
3. **Agents** : Créer votre premier agent (aucun agent par défaut)
4. **MCP Servers** : Configurer serveurs MCP (Docker/NPX/UVX)
5. **Memory** : Configuration embeddings + gestion mémoires
6. **Validation** : Paramètres human-in-the-loop
7. **Prompts** : Bibliothèque de prompts
8. **Import/Export** : Sauvegarde/restauration configuration
9. **Theme** : Choisir Light/Dark

**Note** : Toutes les API keys sont configurées via UI et stockées de manière sécurisée (Tauri secure storage + encryption)

### 4. Créer Workflow

**Page Agent** :
1. Cliquer **+ New** (sidebar workflows)
2. Sélectionner votre agent créé précédemment
3. Nommer workflow : "Mon premier workflow"
4. Envoyer message dans la zone de chat

### 5. Observer Exécution

**Indicateurs Temps Réel** :
- Status workflow : ● Running
- Tokens : Mise à jour incrémentale (input/output/coût)
- Tools : MemoryTool, TodoTool, CalculatorTool (selon config agent)
- MCP : Si serveurs MCP configurés

### 6. Validation (si Human-in-the-Loop)

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
npm run tauri:build
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
2. Rebuild : `npm run tauri:build`
3. Check logs : `src-tauri/target/release/build/`

### MCP Server Offline
**Solution** :
1. Test command manuellement : `docker run -i --rm serena-mcp`
2. Vérifier Docker/NPX/UVX installé
3. Check logs Settings > MCP

---

## Next Steps

1. **Explorer Agents** : [MULTI_AGENT_ARCHITECTURE.md](MULTI_AGENT_ARCHITECTURE.md)
2. **Configurer MCP** : [MCP_CONFIGURATION_GUIDE.md](MCP_CONFIGURATION_GUIDE.md)
3. **Outils Agents** : [AGENT_TOOLS_DOCUMENTATION.md](AGENT_TOOLS_DOCUMENTATION.md)
4. **API Reference** : [API_REFERENCE.md](API_REFERENCE.md)
5. **Sub-Agents** : [SUB_AGENT_GUIDE.md](SUB_AGENT_GUIDE.md)

---

## Ressources

**Tauri** : https://v2.tauri.app/start
**SvelteKit** : https://kit.svelte.dev/docs
**SurrealDB** : https://surrealdb.com/docs
**Rig.rs** : https://rig.rs/getting-started
