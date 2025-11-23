# Zileo Chat 3 - Exemples de Design

Exemples HTML/CSS professionnels pour l'application Zileo Chat 3, basés sur la documentation complète.

## Fichiers

### `style.css`
Design system complet avec toutes les variables CSS et classes réutilisables.

### `settings.html`
Page de configuration avec toutes les sections :
- **Providers** : Configuration Mistral, Ollama avec test de connexion
- **Models** : Sélection modèle, capacités, paramètres (temperature, top_p, etc.)
- **Theme** : Cards Light/Dark avec aperçu couleurs (blanc / gris anthracite)
- **Agents** : Liste agents avec filtres, CRUD, cards
- **Modèle de Prompt** : Bibliothèque de prompts avec variables
- **MCP** : Configuration serveurs MCP, tools disponibles, status
- **Memory** : Configuration embedding, liste mémoires, recherche sémantique
- **Directories** : Gestion fichiers, upload, arbre de fichiers
- **Validation** : Modes validation (auto/manual/selective), thresholds

### `agent.html`
Interface agent multi-workflow avec :
- **Sidebar Workflows** : Liste workflows avec status (running, idle, completed, error)
- **Zone Input** : Textarea avec sélection prompt template
- **Output Stream** : Messages user/agent avec cards de résultats
- **Message Queue** : Indicateur flottant pour messages en file
- **Validation Modal** : Demandes de validation avec risk level
- **Token Display** : Compteur tokens temps réel avec barre de progression
- **Tools Panel** : Outils actifs avec status et durée
- **MCP Servers** : Status et latence des serveurs
- **Sub-Agents** : Cards agents en cours avec progress bar
- **Reasoning** : Steps de raisonnement collapsibles

## Bibliothèque d'Icônes

**Lucide Icons** (https://lucide.dev)
- CDN: `https://unpkg.com/lucide@latest`
- Initialisation: `lucide.createIcons()`
- Usage: `<i data-lucide="icon-name"></i>`

### Icônes Utilisées

**Navigation & Interface**
- `settings` : Configuration
- `bot` : Agent/Bot
- `users` : Agents/Utilisateurs
- `search` : Recherche
- `plus` : Ajouter
- `x` : Fermer/Supprimer
- `edit` : Éditer
- `trash-2` : Supprimer
- `copy` : Dupliquer
- `eye` : Voir/Preview
- `chevron-left` : Collapse sidebar

**Providers & Models**
- `sparkles` : Mistral
- `server` : Ollama/Server
- `cpu` : Models/CPU
- `globe` : Providers/Global

**Theme & UI**
- `palette` : Theme/Couleurs
- `sun` : Light mode
- `moon` : Dark mode
- `monitor` : Auto mode

**Tools & MCP**
- `plug` : MCP Servers
- `tool` : Tools/Outils
- `database` : Database/SurrealDB
- `file-text` : Fichier texte/Prompt
- `file-json` : Fichier JSON
- `folder` : Dossier
- `folder-open` : Ouvrir dossier

**Workflow & Status**
- `activity` : Activity/Metrics
- `zap` : Performance/Speed
- `play` : Exécuter
- `send` : Envoyer
- `paperclip` : Attacher/Prompt

**Memory & Data**
- `brain` : Reasoning/Pensée
- `file-search` : Recherche/Memory
- `upload` : Upload
- `upload-cloud` : Upload cloud
- `download` : Télécharger

**Validation & Security**
- `shield-check` : Validation/Sécurité
- `alert-triangle` : Alerte/Warning
- `info` : Information
- `check` : Valider/Approuver
- `hand` : Manual/Stop

**Filtering & Actions**
- `filter` : Filtre
- `calculator` : Calcul/Analysis

## Design System

### Couleurs

**Light Theme**
```css
--color-bg-primary: #ffffff  /* Blanc */
--color-bg-secondary: #f8f9fa
--color-text-primary: #212529
--color-text-secondary: #495057
--color-accent: #94EFEE
--color-success: #10b981
--color-warning: #f59e0b
--color-error: #ef4444
```

**Dark Theme** : Activé via `data-theme="dark"`
```css
--color-bg-primary: #2b2d31  /* Gris anthracite */
--color-bg-secondary: #1e1f22
--color-text-primary: #ffffff
--color-text-secondary: #b5bac1
--color-accent: #94EFEE
```

**Boutons**
```css
Primary: #94EFEE (Turquoise/Cyan)
Secondary: #FE7254 (Coral/Orange)
```

### Spacing

```css
--spacing-xs: 4px
--spacing-sm: 8px
--spacing-md: 16px
--spacing-lg: 24px
--spacing-xl: 32px
--spacing-2xl: 48px
```

### Typography

**Font Family**
- Primary: Signika (Google Fonts)
- Monospace: JetBrains Mono

**Font Sizes**
```css
--font-size-xs: 12px
--font-size-sm: 14px
--font-size-base: 16px
--font-size-lg: 18px
--font-size-xl: 20px
--font-size-2xl: 24px
```

### Border Radius

```css
--border-radius-sm: 4px
--border-radius-md: 8px
--border-radius-lg: 12px
--border-radius-xl: 16px
--border-radius-full: 9999px
```

## Classes Réutilisables

### Layout

```html
<!-- App Container -->
<div class="app-container">
  <nav class="floating-menu">...</nav>
  <div class="main-content">
    <aside class="sidebar">...</aside>
    <main class="content-area">...</main>
  </div>
</div>
```

### Cards

```html
<div class="card">
  <div class="card-header">
    <h3 class="card-title">Title</h3>
    <p class="card-description">Description</p>
  </div>
  <div class="card-body">Content</div>
  <div class="card-footer">Actions</div>
</div>
```

### Buttons

```html
<button class="btn btn-primary">Primary</button>
<button class="btn btn-secondary">Secondary</button>
<button class="btn btn-ghost">Ghost</button>
<button class="btn btn-danger">Danger</button>

<!-- Sizes -->
<button class="btn btn-sm">Small</button>
<button class="btn btn-lg">Large</button>

<!-- Icon button -->
<button class="btn btn-icon">
  <i data-lucide="plus"></i>
</button>
```

### Form Elements

```html
<!-- Input -->
<div class="form-group">
  <label class="form-label">Label</label>
  <input type="text" class="form-input">
  <span class="form-help">Help text</span>
</div>

<!-- Select -->
<select class="form-select">
  <option>Option 1</option>
</select>

<!-- Textarea -->
<textarea class="form-textarea"></textarea>

<!-- Checkbox -->
<input type="checkbox" class="form-checkbox">

<!-- Radio -->
<input type="radio" class="form-radio">

<!-- Range -->
<input type="range" class="form-range">
```

### Navigation

```html
<a href="#" class="nav-item active">
  <i data-lucide="icon" class="nav-icon"></i>
  <span class="nav-text">Label</span>
</a>
```

### Badges & Status

```html
<!-- Badges -->
<span class="badge badge-primary">Primary</span>
<span class="badge badge-success">Success</span>
<span class="badge badge-warning">Warning</span>
<span class="badge badge-error">Error</span>

<!-- Status Indicators -->
<span class="status-indicator status-idle"></span>
<span class="status-indicator status-running"></span>
<span class="status-indicator status-completed"></span>
<span class="status-indicator status-error"></span>
```

### Filter Bar

```html
<div class="filter-bar">
  <div class="search-box">
    <i data-lucide="search" class="search-icon"></i>
    <input type="search" class="search-input" placeholder="Search...">
  </div>
  <select class="form-select">...</select>
  <button class="btn btn-primary">Action</button>
</div>
```

### Table

```html
<div class="table-container">
  <table class="table">
    <thead>
      <tr>
        <th>Column</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td>Data</td>
      </tr>
    </tbody>
  </table>
</div>
```

### Modal

```html
<div class="modal-backdrop">
  <div class="modal">
    <div class="modal-header">
      <h3 class="modal-title">Title</h3>
    </div>
    <div class="modal-body">Content</div>
    <div class="modal-footer">
      <button class="btn btn-secondary">Cancel</button>
      <button class="btn btn-primary">Confirm</button>
    </div>
  </div>
</div>
```

### Progress

```html
<!-- Progress Bar -->
<div class="progress-bar">
  <div class="progress-fill" style="width: 67%;"></div>
</div>

<!-- Spinner -->
<div class="spinner"></div>

<!-- Native Progress -->
<progress value="50" max="100"></progress>
```

### Utility Classes

**Flexbox**
```html
<div class="flex items-center justify-between gap-md">
  <div class="flex-1">Content</div>
</div>

<div class="flex flex-col gap-lg">
  ...
</div>
```

**Grid**
```html
<div class="grid grid-cols-2 gap-md">
  <div>Column 1</div>
  <div>Column 2</div>
</div>

<div class="grid grid-cols-3 gap-lg">
  ...
</div>
```

**Spacing**
```html
<div class="mt-md mb-lg">Margin top & bottom</div>
```

**Text**
```html
<span class="text-sm text-secondary">Small secondary text</span>
<span class="text-lg font-semibold">Large semibold</span>
<span class="truncate">Long text that will be truncated...</span>
```

## Animations

### Pulse (Running Status)
```css
.status-running {
  animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}
```

### Fade In (Messages)
```css
.message {
  animation: fadeIn 0.3s ease-in;
}
```

### Slide Up (Queue Indicator)
```css
.queue-indicator {
  animation: slideUp 0.3s ease-out;
}
```

### Spin (Loading)
```css
.spinner {
  animation: spin 0.8s linear infinite;
}
```

## Responsive Behavior

### Sidebar Collapse
```javascript
document.querySelector('.sidebar-footer button').addEventListener('click', function() {
  document.querySelector('.sidebar').classList.toggle('collapsed');
  lucide.createIcons(); // Re-render icons
});
```

### Auto-resize Textarea
```javascript
textarea.addEventListener('input', function() {
  this.style.height = 'auto';
  this.style.height = Math.min(this.scrollHeight, 300) + 'px';
});
```

### Smooth Scroll Navigation
```javascript
document.querySelectorAll('.nav-item').forEach(item => {
  item.addEventListener('click', function(e) {
    e.preventDefault();
    const target = document.querySelector(this.getAttribute('href'));
    target.scrollIntoView({ behavior: 'smooth', block: 'start' });
  });
});
```

## Features Interactives

### Contenteditable Workflow Names
```html
<span class="workflow-name" contenteditable="true">Workflow Name</span>
```

### Range avec Output
```html
<label class="form-label">Temperature: <output>0.7</output></label>
<input type="range" min="0" max="2" step="0.1" value="0.7">

<script>
range.addEventListener('input', function() {
  output.textContent = this.value;
});
</script>
```

### Details/Summary (Collapsible)
```html
<details>
  <summary>Available Tools (8)</summary>
  <div>Content</div>
</details>
```

## Accessibilité

### Keyboard Navigation
- Tab/Shift+Tab : Navigation
- Enter/Space : Activer boutons
- Esc : Fermer modals
- Ctrl+Enter : Envoyer message

### ARIA Labels
```html
<button aria-label="Create new workflow">
  <i data-lucide="plus"></i>
</button>

<div role="status" aria-live="polite">
  {statusMessage}
</div>

<progress value="50" max="100" aria-label="Token usage: 50 of 100"></progress>
```

### Focus Management
- Focus visible sur tous les éléments interactifs
- Outline personnalisé avec box-shadow accent
- Focus trap dans les modals

## Notes d'Implémentation

1. **Lucide Icons** doit être initialisé après chaque modification du DOM
2. Les **variables CSS** permettent le theme switching facile
3. Les **classes utilitaires** réduisent le CSS custom
4. **Transitions** : 150ms fast, 200ms base, 300ms slow
5. **Z-index** : Système de layers défini dans les variables
6. **Shadows** : 5 niveaux (xs, sm, md, lg, xl)
7. **Grid** : Responsive avec 2-3 colonnes selon l'espace

## Prochaines Étapes

Pour intégrer dans SvelteKit :
1. Convertir les classes en composants Svelte
2. Extraire les variables CSS vers un fichier global
3. Utiliser Svelte stores pour l'état (workflows, messages, validation)
4. Implémenter les Tauri commands pour la communication backend
5. Ajouter les animations/transitions Svelte natives
6. Utiliser Svelte 5 runes ($state, $derived, $effect)
