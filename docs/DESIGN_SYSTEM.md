# Zileo Chat 3 - Design System

Documentation complete du systeme de design pour l'application Zileo-Chat-3.

## Table of Contents

1. [Overview](#overview)
2. [Color Palette](#color-palette)
3. [Typography](#typography)
4. [Spacing System](#spacing-system)
5. [Border & Radius](#border--radius)
6. [Shadows](#shadows)
7. [Transitions & Animations](#transitions--animations)
8. [Z-Index Layers](#z-index-layers)
9. [Layout Components](#layout-components)
10. [UI Components](#ui-components)
11. [Form Components](#form-components)
12. [Navigation Components](#navigation-components)
13. [Status & Feedback](#status--feedback)
14. [Utility Classes](#utility-classes)
15. [Icons Library](#icons-library)
16. [Accessibility](#accessibility)
17. [SvelteKit Implementation](#sveltekit-implementation)

---

## Overview

### Design Philosophy

- **Professional & Clean**: Interface epuree, focus sur le contenu
- **Dual Theme**: Support complet light/dark mode
- **Component-Based**: Composants reutilisables et coherents
- **Accessible**: WCAG 2.1 AA compliance
- **Responsive**: Adaptation desktop-first avec sidebar collapsible

### Tech Stack Design

| Aspect | Technology |
|--------|------------|
| Icons | Lucide Icons (lucide.dev) |
| Fonts | Signika (UI) + JetBrains Mono (Code) |
| CSS | CSS Variables + Utility Classes |
| Theming | `data-theme` attribute |

---

## Color Palette

### Brand Colors

| Name | Hex | Usage |
|------|-----|-------|
| **Primary (Accent)** | `#94EFEE` | Boutons primaires, liens actifs, focus |
| **Primary Hover** | `#7de6e5` | Hover state du primary |
| **Secondary** | `#FE7254` | Boutons secondaires, CTAs importants |
| **Secondary Hover** | `#fe5a3d` | Hover state du secondary |

### Light Theme

```css
:root {
  /* Backgrounds */
  --color-bg-primary: #ffffff;      /* Main content background */
  --color-bg-secondary: #f8f9fa;    /* Sidebar, cards, footer */
  --color-bg-tertiary: #f1f3f5;     /* Nested elements, code blocks */
  --color-bg-hover: #e9ecef;        /* Hover states */
  --color-bg-active: #dee2e6;       /* Active/pressed states */

  /* Text */
  --color-text-primary: #212529;    /* Main text, headings */
  --color-text-secondary: #495057;  /* Descriptions, labels */
  --color-text-tertiary: #6c757d;   /* Hints, timestamps */
  --color-text-inverse: #ffffff;    /* Text on dark backgrounds */

  /* Borders */
  --color-border: rgba(33, 37, 41, 0.15);
  --color-border-light: rgba(33, 37, 41, 0.1);
  --color-border-dark: rgba(33, 37, 41, 0.25);

  /* Accent */
  --color-accent: #94EFEE;
  --color-accent-hover: #7de6e5;
  --color-accent-light: rgba(148, 239, 238, 0.2);
}
```

### Dark Theme

```css
[data-theme="dark"] {
  /* Backgrounds */
  --color-bg-primary: #2b2d31;      /* Gris anthracite */
  --color-bg-secondary: #1e1f22;    /* Sidebar, cards */
  --color-bg-tertiary: #161719;     /* Nested elements */
  --color-bg-hover: #35373c;        /* Hover states */
  --color-bg-active: #3f4147;       /* Active states */

  /* Text */
  --color-text-primary: #ffffff;
  --color-text-secondary: #b5bac1;
  --color-text-tertiary: #80848e;
  --color-text-inverse: #212529;

  /* Borders */
  --color-border: #3f4147;
  --color-border-light: #35373c;
  --color-border-dark: #4e5058;

  /* Accent (unchanged) */
  --color-accent: #94EFEE;
  --color-accent-hover: #7de6e5;
  --color-accent-light: rgba(148, 239, 238, 0.15);
}
```

### Semantic Colors

```css
:root {
  /* Success */
  --color-success: #10b981;
  --color-success-light: #d1fae5;

  /* Warning */
  --color-warning: #f59e0b;
  --color-warning-light: #fef3c7;

  /* Error */
  --color-error: #ef4444;
  --color-error-light: #fee2e2;
}
```

### Status Colors

```css
:root {
  --color-status-idle: #6c757d;       /* Gris */
  --color-status-running: #3b82f6;    /* Bleu */
  --color-status-completed: #10b981;  /* Vert */
  --color-status-error: #ef4444;      /* Rouge */
}
```

---

## Typography

### Font Families

```css
:root {
  --font-family: 'Signika', -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
  --font-mono: 'JetBrains Mono', 'Fira Code', monospace;
}
```

**Google Fonts Import:**
```html
<link href="https://fonts.googleapis.com/css2?family=Signika:wght@400;500;600;700&family=JetBrains+Mono&display=swap" rel="stylesheet">
```

### Font Sizes

| Token | Size | Pixels | Usage |
|-------|------|--------|-------|
| `--font-size-xs` | 0.75rem | 12px | Badges, timestamps, hints |
| `--font-size-sm` | 0.875rem | 14px | Body small, labels, descriptions |
| `--font-size-base` | 1rem | 16px | Body text, inputs |
| `--font-size-lg` | 1.125rem | 18px | Section titles, card titles |
| `--font-size-xl` | 1.25rem | 20px | Page subtitles, modal titles |
| `--font-size-2xl` | 1.5rem | 24px | Page titles, section headers |

### Font Weights

| Token | Weight | Usage |
|-------|--------|-------|
| `--font-weight-normal` | 400 | Body text |
| `--font-weight-medium` | 500 | Labels, nav items, emphasis |
| `--font-weight-semibold` | 600 | Headings, card titles, buttons |
| `--font-weight-bold` | 700 | Strong emphasis, badges |

### Line Heights

| Token | Value | Usage |
|-------|-------|-------|
| `--line-height-tight` | 1.25 | Headings, buttons |
| `--line-height-base` | 1.5 | Body text |
| `--line-height-relaxed` | 1.75 | Messages, long-form content |

---

## Spacing System

### Scale

| Token | Size | Pixels | Usage |
|-------|------|--------|-------|
| `--spacing-xs` | 0.25rem | 4px | Tight gaps, badge padding |
| `--spacing-sm` | 0.5rem | 8px | Small gaps, compact elements |
| `--spacing-md` | 1rem | 16px | Standard gaps, card padding |
| `--spacing-lg` | 1.5rem | 24px | Section spacing, large gaps |
| `--spacing-xl` | 2rem | 32px | Page padding, major sections |
| `--spacing-2xl` | 3rem | 48px | Large separations |

### Application Guidelines

- **Card padding**: `--spacing-lg` (24px)
- **Form group margin**: `--spacing-lg` (24px)
- **Button padding**: `--spacing-sm --spacing-md` (8px 16px)
- **Badge padding**: `--spacing-xs --spacing-sm` (4px 8px)
- **Icon gaps**: `--spacing-sm` (8px)

---

## Border & Radius

### Border Radius Scale

| Token | Size | Pixels | Usage |
|-------|------|--------|-------|
| `--border-radius-sm` | 0.25rem | 4px | Small elements, code blocks |
| `--border-radius-md` | 0.5rem | 8px | Buttons, inputs, nav items |
| `--border-radius-lg` | 0.75rem | 12px | Cards, modals |
| `--border-radius-xl` | 1rem | 16px | Large cards, validation modals |
| `--border-radius-full` | 9999px | Pill | Badges, status indicators, spinners |

### Border Patterns

```css
/* Standard border */
border: 1px solid var(--color-border);

/* Light border (subtle separators) */
border: 1px solid var(--color-border-light);

/* Active/focus border */
border: 2px solid var(--color-accent);

/* Error border */
border: 2px solid var(--color-error);
```

---

## Shadows

### Shadow Scale

```css
:root {
  --shadow-xs: 0 1px 2px 0 rgba(0, 0, 0, 0.05);
  --shadow-sm: 0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px -1px rgba(0, 0, 0, 0.1);
  --shadow-md: 0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -2px rgba(0, 0, 0, 0.1);
  --shadow-lg: 0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -4px rgba(0, 0, 0, 0.1);
  --shadow-xl: 0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 8px 10px -6px rgba(0, 0, 0, 0.1);
}
```

### Usage Guidelines

| Shadow | Usage |
|--------|-------|
| `xs` | Subtle elevation for inputs |
| `sm` | Cards, dropdowns |
| `md` | Floating elements, popovers |
| `lg` | Queue indicators, floating buttons |
| `xl` | Modals, validation dialogs |

### Focus Ring

```css
/* Standard focus ring */
box-shadow: 0 0 0 3px var(--color-accent-light);
```

---

## Transitions & Animations

### Transition Durations

```css
:root {
  --transition-fast: 150ms cubic-bezier(0.4, 0, 0.2, 1);
  --transition-base: 200ms cubic-bezier(0.4, 0, 0.2, 1);
  --transition-slow: 300ms cubic-bezier(0.4, 0, 0.2, 1);
}
```

### Usage

| Speed | Usage |
|-------|-------|
| `fast` | Buttons, hover states, icons |
| `base` | Sidebar collapse, tab switches |
| `slow` | Modal open/close, page transitions |

### Built-in Animations

#### Pulse (Running Status)
```css
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.status-running {
  animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}
```

#### Spin (Loading)
```css
@keyframes spin {
  to { transform: rotate(360deg); }
}

.spinner {
  animation: spin 0.8s linear infinite;
}
```

#### Fade In (Messages)
```css
@keyframes fadeIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

.message {
  animation: fadeIn 0.3s ease-in;
}
```

#### Slide Up (Notifications)
```css
@keyframes slideUp {
  from { transform: translateY(100px); opacity: 0; }
  to { transform: translateY(0); opacity: 1; }
}

.queue-indicator {
  animation: slideUp 0.3s ease-out;
}
```

---

## Z-Index Layers

```css
:root {
  --z-index-dropdown: 1000;
  --z-index-sticky: 1020;
  --z-index-fixed: 1030;
  --z-index-modal-backdrop: 1040;
  --z-index-modal: 1050;
  --z-index-popover: 1060;
  --z-index-tooltip: 1070;
}
```

### Layer Hierarchy

1. **Base content**: z-index: auto
2. **Dropdowns**: 1000
3. **Sticky headers**: 1020
4. **Fixed elements** (floating menu): 1030
5. **Modal backdrop**: 1040
6. **Modal**: 1050
7. **Popovers**: 1060
8. **Tooltips**: 1070

---

## Layout Components

### Layout Variables

```css
:root {
  /* Left Sidebar */
  --sidebar-width: 280px;
  --sidebar-collapsed-width: 60px;

  /* Right Sidebar (Activity) */
  --right-sidebar-width: 320px;
  --right-sidebar-collapsed-width: 48px;

  /* Top Menu */
  --floating-menu-height: 60px;
}
```

### App Container

```html
<div class="app-container">
  <nav class="floating-menu">...</nav>
  <div class="main-content">
    <aside class="sidebar">...</aside>
    <main class="content-area">...</main>
  </div>
</div>
```

```css
.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
}

.main-content {
  display: flex;
  margin-top: var(--floating-menu-height);
  height: calc(100vh - var(--floating-menu-height));
  overflow: hidden;
}
```

### Floating Menu

```css
.floating-menu {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  height: var(--floating-menu-height);
  background: var(--color-bg-primary);
  border-bottom: 1px solid var(--color-border);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  padding: 0 var(--spacing-xl);
  z-index: var(--z-index-fixed);
}
```

### Sidebar

```css
.sidebar {
  width: var(--sidebar-width);
  background: var(--color-bg-secondary);
  border-right: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  transition: width var(--transition-base);
  overflow: hidden;
}

.sidebar.collapsed {
  width: var(--sidebar-collapsed-width);
}

.sidebar-header {
  padding: var(--spacing-lg);
  border-bottom: 1px solid var(--color-border);
}

.sidebar-nav {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-md);
}

.sidebar-footer {
  padding: var(--spacing-md);
  border-top: 1px solid var(--color-border);
}
```

### Content Area

```css
.content-area {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-xl);
}
```

---

## UI Components

### Card

```html
<div class="card">
  <div class="card-header">
    <h3 class="card-title">Title</h3>
    <p class="card-description">Description text</p>
  </div>
  <div class="card-body">
    Content goes here
  </div>
  <div class="card-footer">
    <button class="btn btn-primary">Action</button>
  </div>
</div>
```

```css
.card {
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: var(--border-radius-lg);
  box-shadow: var(--shadow-sm);
  overflow: hidden;
}

.card-header {
  padding: var(--spacing-lg);
  border-bottom: 1px solid var(--color-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.card-title {
  font-size: var(--font-size-lg);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
}

.card-description {
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  margin-top: var(--spacing-xs);
}

.card-body {
  padding: var(--spacing-lg);
}

.card-footer {
  padding: var(--spacing-lg);
  border-top: 1px solid var(--color-border);
  background: var(--color-bg-secondary);
}
```

### Buttons

#### Variants

```html
<button class="btn btn-primary">Primary</button>
<button class="btn btn-secondary">Secondary</button>
<button class="btn btn-ghost">Ghost</button>
<button class="btn btn-danger">Danger</button>
```

#### Sizes

```html
<button class="btn btn-sm">Small</button>
<button class="btn">Default</button>
<button class="btn btn-lg">Large</button>
<button class="btn btn-icon"><i data-lucide="plus"></i></button>
```

```css
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-sm) var(--spacing-md);
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-tight);
  border-radius: var(--border-radius-md);
  border: 1px solid transparent;
  cursor: pointer;
  transition: all var(--transition-fast);
  text-decoration: none;
  white-space: nowrap;
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Primary - Turquoise */
.btn-primary {
  background: #94EFEE;
  color: #212529;
  border-color: #94EFEE;
  font-weight: var(--font-weight-semibold);
}

.btn-primary:hover:not(:disabled) {
  background: #7de6e5;
  border-color: #7de6e5;
}

/* Secondary - Coral */
.btn-secondary {
  background: #FE7254;
  color: #ffffff;
  border-color: #FE7254;
  font-weight: var(--font-weight-semibold);
}

.btn-secondary:hover:not(:disabled) {
  background: #fe5a3d;
  border-color: #fe5a3d;
}

/* Ghost */
.btn-ghost {
  background: transparent;
  color: var(--color-text-secondary);
}

.btn-ghost:hover:not(:disabled) {
  background: var(--color-bg-hover);
  color: var(--color-text-primary);
}

/* Danger */
.btn-danger {
  background: var(--color-error);
  color: var(--color-text-inverse);
}

.btn-danger:hover:not(:disabled) {
  background: #dc2626;
}

/* Sizes */
.btn-sm {
  padding: var(--spacing-xs) var(--spacing-sm);
  font-size: var(--font-size-xs);
}

.btn-lg {
  padding: var(--spacing-md) var(--spacing-lg);
  font-size: var(--font-size-base);
}

.btn-icon {
  padding: var(--spacing-sm);
  aspect-ratio: 1;
}
```

### Modal

```html
<div class="modal-backdrop">
  <div class="modal">
    <div class="modal-header">
      <h3 class="modal-title">Modal Title</h3>
      <button class="btn btn-ghost btn-icon">
        <i data-lucide="x"></i>
      </button>
    </div>
    <div class="modal-body">
      Modal content
    </div>
    <div class="modal-footer">
      <button class="btn btn-ghost">Cancel</button>
      <button class="btn btn-primary">Confirm</button>
    </div>
  </div>
</div>
```

```css
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(4px);
  z-index: var(--z-index-modal-backdrop);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-xl);
}

.modal {
  background: var(--color-bg-primary);
  border-radius: var(--border-radius-xl);
  box-shadow: var(--shadow-xl);
  max-width: 600px;
  width: 100%;
  max-height: 90vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.modal-header {
  padding: var(--spacing-lg);
  border-bottom: 1px solid var(--color-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.modal-title {
  font-size: var(--font-size-xl);
  font-weight: var(--font-weight-semibold);
}

.modal-body {
  padding: var(--spacing-lg);
  overflow-y: auto;
}

.modal-footer {
  padding: var(--spacing-lg);
  border-top: 1px solid var(--color-border);
  display: flex;
  gap: var(--spacing-md);
  justify-content: flex-end;
}
```

### Table

```html
<div class="table-container">
  <table class="table">
    <thead>
      <tr>
        <th>Column 1</th>
        <th>Column 2</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td>Data 1</td>
        <td>Data 2</td>
      </tr>
    </tbody>
  </table>
</div>
```

```css
.table-container {
  overflow-x: auto;
}

.table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--font-size-sm);
}

.table th {
  padding: var(--spacing-md);
  text-align: left;
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-secondary);
  border-bottom: 2px solid var(--color-border);
  background: var(--color-bg-secondary);
}

.table td {
  padding: var(--spacing-md);
  border-bottom: 1px solid var(--color-border-light);
}

.table tbody tr:hover {
  background: var(--color-bg-hover);
}
```

---

## Form Components

### Input

```html
<div class="form-group">
  <label class="form-label">Label</label>
  <input type="text" class="form-input" placeholder="Placeholder...">
  <span class="form-help">Help text</span>
</div>
```

### Select

```html
<select class="form-select">
  <option>Option 1</option>
  <option>Option 2</option>
</select>
```

### Textarea

```html
<textarea class="form-textarea" placeholder="Enter text..."></textarea>
```

### Checkbox & Radio

```html
<label class="flex items-center gap-sm">
  <input type="checkbox" class="form-checkbox">
  <span>Checkbox label</span>
</label>

<label class="flex items-center gap-sm">
  <input type="radio" name="group" class="form-radio">
  <span>Radio label</span>
</label>
```

### Range Slider

```html
<div class="form-group">
  <label class="form-label">Value: <output>0.7</output></label>
  <input type="range" class="form-range" min="0" max="1" step="0.1" value="0.7">
</div>
```

```css
.form-group {
  margin-bottom: var(--spacing-lg);
}

.form-label {
  display: block;
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-sm);
}

.form-help {
  display: block;
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
  margin-top: var(--spacing-xs);
}

.form-input,
.form-select,
.form-textarea {
  width: 100%;
  padding: var(--spacing-sm) var(--spacing-md);
  font-size: var(--font-size-sm);
  color: var(--color-text-primary);
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: var(--border-radius-md);
  transition: all var(--transition-fast);
}

.form-input:focus,
.form-select:focus,
.form-textarea:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-light);
}

.form-input:disabled,
.form-select:disabled,
.form-textarea:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.form-textarea {
  min-height: 100px;
  resize: vertical;
  font-family: var(--font-mono);
}

.form-checkbox,
.form-radio {
  width: 1rem;
  height: 1rem;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.form-range {
  width: 100%;
  height: 0.25rem;
  background: var(--color-bg-tertiary);
  border-radius: var(--border-radius-full);
  outline: none;
  cursor: pointer;
  accent-color: var(--color-accent);
}
```

### Filter Bar

```html
<div class="filter-bar">
  <div class="search-box">
    <i data-lucide="search" class="search-icon"></i>
    <input type="search" class="search-input" placeholder="Search...">
  </div>
  <select class="form-select" style="width: auto;">
    <option>All Types</option>
  </select>
  <button class="btn btn-primary">Action</button>
</div>
```

```css
.filter-bar {
  display: flex;
  gap: var(--spacing-md);
  padding: var(--spacing-md);
  background: var(--color-bg-secondary);
  border-radius: var(--border-radius-md);
  flex-wrap: wrap;
}

.search-box {
  position: relative;
  flex: 1;
  min-width: 200px;
}

.search-icon {
  position: absolute;
  left: var(--spacing-md);
  top: 50%;
  transform: translateY(-50%);
  color: var(--color-text-tertiary);
  width: 16px;
  height: 16px;
}

.search-input {
  width: 100%;
  padding: var(--spacing-sm) var(--spacing-md) var(--spacing-sm) 2.5rem;
  font-size: var(--font-size-sm);
  color: var(--color-text-primary);
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: var(--border-radius-md);
  transition: all var(--transition-fast);
}

.search-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-light);
}
```

---

## Navigation Components

### Nav Item

```html
<a href="#section" class="nav-item active">
  <i data-lucide="settings" class="nav-icon"></i>
  <span class="nav-text">Settings</span>
</a>
```

```css
.nav-item {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  padding: var(--spacing-md);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  border-radius: var(--border-radius-md);
  cursor: pointer;
  transition: all var(--transition-fast);
  text-decoration: none;
}

.nav-item:hover {
  background: var(--color-bg-hover);
  color: var(--color-text-primary);
}

.nav-item.active {
  background: var(--color-accent-light);
  color: var(--color-accent);
  font-weight: var(--font-weight-medium);
}

.nav-icon {
  width: 20px;
  height: 20px;
  flex-shrink: 0;
}

/* Collapsed sidebar */
.sidebar.collapsed .nav-item {
  justify-content: center;
}

.sidebar.collapsed .nav-text {
  display: none;
}
```

### Workflow Item (Agent Page)

```html
<div class="workflow-item active">
  <span class="status-indicator status-running"></span>
  <span class="workflow-name" contenteditable="true">Workflow Name</span>
  <button class="workflow-delete btn-icon">
    <i data-lucide="x"></i>
  </button>
</div>
```

```css
.workflow-item {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  padding: var(--spacing-md);
  border-radius: var(--border-radius-md);
  cursor: pointer;
  transition: all var(--transition-fast);
  border: 1px solid transparent;
  position: relative;
}

.workflow-item:hover {
  background: var(--color-bg-hover);
}

.workflow-item.active {
  background: var(--color-accent-light);
  border-color: var(--color-accent);
}

.workflow-name {
  flex: 1;
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-medium);
  outline: none;
}

.workflow-item.active .workflow-name {
  color: var(--color-accent);
}

.workflow-delete {
  opacity: 0;
  transition: opacity var(--transition-fast);
}

.workflow-item:hover .workflow-delete {
  opacity: 1;
}
```

---

## Status & Feedback

### Badges

```html
<span class="badge badge-primary">Primary</span>
<span class="badge badge-success">Success</span>
<span class="badge badge-warning">Warning</span>
<span class="badge badge-error">Error</span>
```

```css
.badge {
  display: inline-flex;
  align-items: center;
  gap: var(--spacing-xs);
  padding: var(--spacing-xs) var(--spacing-sm);
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-medium);
  border-radius: var(--border-radius-full);
  white-space: nowrap;
}

.badge-primary {
  background: var(--color-accent-light);
  color: var(--color-accent);
}

.badge-success {
  background: var(--color-success-light);
  color: var(--color-success);
}

.badge-warning {
  background: var(--color-warning-light);
  color: var(--color-warning);
}

.badge-error {
  background: var(--color-error-light);
  color: var(--color-error);
}
```

### Status Indicators

```html
<span class="status-indicator status-idle"></span>
<span class="status-indicator status-running"></span>
<span class="status-indicator status-completed"></span>
<span class="status-indicator status-error"></span>
```

```css
.status-indicator {
  width: 8px;
  height: 8px;
  border-radius: var(--border-radius-full);
}

.status-idle {
  background: var(--color-status-idle);
}

.status-running {
  background: var(--color-status-running);
  animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}

.status-completed {
  background: var(--color-status-completed);
}

.status-error {
  background: var(--color-status-error);
}
```

### Progress Bar

```html
<div class="progress-bar">
  <div class="progress-fill" style="width: 67%;"></div>
</div>
```

```css
.progress-bar {
  width: 100%;
  height: 8px;
  background: var(--color-bg-tertiary);
  border-radius: var(--border-radius-full);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--color-accent);
  border-radius: var(--border-radius-full);
  transition: width var(--transition-base);
}
```

### Spinner

```html
<div class="spinner"></div>
```

```css
.spinner {
  width: 20px;
  height: 20px;
  border: 2px solid var(--color-border);
  border-top-color: var(--color-accent);
  border-radius: var(--border-radius-full);
  animation: spin 0.8s linear infinite;
}
```

### Skeleton (Loading Placeholder)

```svelte
<script lang="ts">
  import { Skeleton } from '$lib/components/ui';
</script>

<!-- Text skeleton -->
<Skeleton variant="text" width="200px" />

<!-- Circular skeleton (avatar) -->
<Skeleton variant="circular" size="48px" />

<!-- Rectangular skeleton (image/card) -->
<Skeleton variant="rectangular" width="100%" height="200px" />
```

**Props**:
| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `variant` | `'text' \| 'circular' \| 'rectangular'` | `'text'` | Shape variant |
| `width` | `string` | `'100%'` | Width (CSS value) |
| `height` | `string` | `'1em'` | Height (CSS value) |
| `size` | `string` | - | Width & height (for circular) |
| `animate` | `boolean` | `true` | Enable shimmer animation |

```css
.skeleton {
  background: var(--color-bg-tertiary);
  border-radius: var(--border-radius-sm);
}

.skeleton.animate {
  animation: skeleton-shimmer 1.5s ease-in-out infinite;
}

.skeleton-text {
  height: 1em;
  border-radius: var(--border-radius-sm);
}

.skeleton-circular {
  border-radius: var(--border-radius-full);
}

.skeleton-rectangular {
  border-radius: var(--border-radius-md);
}

@keyframes skeleton-shimmer {
  0% { opacity: 1; }
  50% { opacity: 0.5; }
  100% { opacity: 1; }
}
```

### LanguageSelector (i18n)

```svelte
<script lang="ts">
  import { LanguageSelector } from '$lib/components/ui';
</script>

<!-- Self-contained language picker -->
<LanguageSelector />
```

The LanguageSelector is a self-contained component that:
- Displays current locale with country flag
- Shows dropdown with available languages
- Uses `localeStore` internally for state management
- Persists selection to localStorage

**Supported Locales**:
| Code | Language | Flag |
|------|----------|------|
| `en` | English | US |
| `fr` | Francais | FR |

---

## Utility Classes

### Flexbox

```html
<div class="flex items-center justify-between gap-md">
  <div class="flex-1">Content</div>
</div>

<div class="flex flex-col gap-lg">
  ...
</div>
```

```css
.flex { display: flex; }
.flex-col { flex-direction: column; }
.items-center { align-items: center; }
.items-start { align-items: flex-start; }
.justify-between { justify-content: space-between; }
.justify-center { justify-content: center; }
.flex-1 { flex: 1; }
```

### Grid

```html
<div class="grid grid-cols-2 gap-md">...</div>
<div class="grid grid-cols-3 gap-lg">...</div>
```

```css
.grid { display: grid; }
.grid-cols-2 { grid-template-columns: repeat(2, 1fr); }
.grid-cols-3 { grid-template-columns: repeat(3, 1fr); }
.gap-sm { gap: var(--spacing-sm); }
.gap-md { gap: var(--spacing-md); }
.gap-lg { gap: var(--spacing-lg); }
```

### Spacing

```css
.mt-sm { margin-top: var(--spacing-sm); }
.mt-md { margin-top: var(--spacing-md); }
.mt-lg { margin-top: var(--spacing-lg); }
.mb-sm { margin-bottom: var(--spacing-sm); }
.mb-md { margin-bottom: var(--spacing-md); }
.mb-lg { margin-bottom: var(--spacing-lg); }
```

### Typography

```css
.text-sm { font-size: var(--font-size-sm); }
.text-lg { font-size: var(--font-size-lg); }
.text-secondary { color: var(--color-text-secondary); }
.text-tertiary { color: var(--color-text-tertiary); }
.font-medium { font-weight: var(--font-weight-medium); }
.font-semibold { font-weight: var(--font-weight-semibold); }
```

### Misc

```css
.truncate {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.hidden { display: none; }
```

---

## Icons Library

### Lucide Icons

**CDN:**
```html
<script src="https://unpkg.com/lucide@latest"></script>
```

**Initialization:**
```javascript
lucide.createIcons();
```

**Usage:**
```html
<i data-lucide="icon-name" style="width: 16px; height: 16px;"></i>
```

### Icon Catalog

#### Navigation & Interface
| Icon | Name | Usage |
|------|------|-------|
| Settings gear | `settings` | Configuration |
| Robot | `bot` | Agent/Bot |
| Users | `users` | Agents list |
| Search | `search` | Search fields |
| Plus | `plus` | Add/Create |
| X | `x` | Close/Delete |
| Edit | `edit` | Edit action |
| Trash | `trash-2` | Delete |
| Copy | `copy` | Duplicate |
| Eye | `eye` | View/Preview |
| Chevron | `chevron-left` | Collapse |

#### Providers & Models
| Icon | Name | Usage |
|------|------|-------|
| Sparkles | `sparkles` | Mistral provider |
| Server | `server` | Ollama/Local |
| CPU | `cpu` | Models |
| Globe | `globe` | Providers |

#### Theme
| Icon | Name | Usage |
|------|------|-------|
| Palette | `palette` | Theme settings |
| Sun | `sun` | Light mode |
| Moon | `moon` | Dark mode |

#### Tools & MCP
| Icon | Name | Usage |
|------|------|-------|
| Plug | `plug` | MCP servers |
| Tool | `tool` | Tools |
| Database | `database` | SurrealDB |
| File text | `file-text` | Text files |
| File JSON | `file-json` | JSON files |
| Folder | `folder` | Directory |
| Folder open | `folder-open` | Open folder |

#### Workflow & Status
| Icon | Name | Usage |
|------|------|-------|
| Activity | `activity` | Metrics |
| Zap | `zap` | Performance |
| Play | `play` | Execute |
| Send | `send` | Send message |
| Paperclip | `paperclip` | Attach |

#### Memory & Data
| Icon | Name | Usage |
|------|------|-------|
| Brain | `brain` | Reasoning |
| File search | `file-search` | Memory search |
| Upload | `upload` | Upload |
| Upload cloud | `upload-cloud` | Cloud upload |
| Download | `download` | Download |

#### Validation
| Icon | Name | Usage |
|------|------|-------|
| Shield check | `shield-check` | Validation |
| Alert triangle | `alert-triangle` | Warning |
| Info | `info` | Information |
| Check | `check` | Approve |
| Hand | `hand` | Manual/Stop |

---

## Accessibility

### Keyboard Navigation

- **Tab / Shift+Tab**: Navigate between elements
- **Enter / Space**: Activate buttons
- **Escape**: Close modals
- **Ctrl+Enter**: Send message (Agent page)
- **Arrow keys**: Navigate lists/options

### ARIA Labels

```html
<!-- Icon-only buttons -->
<button aria-label="Create new workflow" class="btn btn-icon">
  <i data-lucide="plus"></i>
</button>

<!-- Live regions -->
<div role="status" aria-live="polite">
  {statusMessage}
</div>

<!-- Progress -->
<progress value="50" max="100" aria-label="Token usage: 50 of 100"></progress>

<!-- Modal -->
<div role="dialog" aria-modal="true" aria-labelledby="modal-title">
  <h3 id="modal-title">Modal Title</h3>
</div>
```

### Focus Management

```css
/* Visible focus ring */
:focus-visible {
  outline: none;
  box-shadow: 0 0 0 3px var(--color-accent-light);
}

/* Form elements focus */
.form-input:focus,
.form-select:focus {
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-light);
}
```

### Color Contrast

- Text on light backgrounds: minimum 4.5:1 ratio
- Text on dark backgrounds: minimum 4.5:1 ratio
- Large text (18px+): minimum 3:1 ratio
- UI components: minimum 3:1 ratio against background

### Reduced Motion Support

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

Users who prefer reduced motion (system setting) will see:
- No animations (spinners, pulses, shimmers)
- Instant transitions
- No scroll animations

### High Contrast Support

```css
@media (prefers-contrast: high) {
  :root {
    --color-border: rgba(0, 0, 0, 0.5);
    --color-text-secondary: var(--color-text-primary);
  }
}
```

High contrast mode enhances:
- Border visibility
- Text readability (secondary text uses primary color)

### Screen Reader Support

```css
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}
```

Use `.sr-only` for content that should be read by screen readers but not displayed visually.

---

## SvelteKit Implementation

### CSS Variables File

Create `src/styles/variables.css`:

```css
/* Import in app.css or layout */
:root {
  /* All CSS variables from design system */
}

[data-theme="dark"] {
  /* Dark theme overrides */
}
```

### Component Structure

```
src/lib/components/
  ui/                        # 13 atomic components
    Button.svelte            # 4 variants, 4 sizes
    Card.svelte              # Flexible snippet slots
    Modal.svelte             # Accessible dialog
    Badge.svelte             # 4 semantic variants
    Input.svelte             # 6 input types
    Select.svelte            # Dropdown with options
    Textarea.svelte          # Multi-line input
    ProgressBar.svelte       # Progress indicator
    Spinner.svelte           # Loading spinner
    StatusIndicator.svelte   # Status dots (4 states)
    Skeleton.svelte          # Loading placeholder (3 variants)
    LanguageSelector.svelte  # i18n language picker
  layout/
    AppContainer.svelte
    FloatingMenu.svelte
    Sidebar.svelte
    ContentArea.svelte
  navigation/
    NavItem.svelte
    FilterBar.svelte
    SearchBox.svelte
```

**Note**: Table styles are provided as CSS classes (`.table`, `.table-container`) rather than a dedicated Svelte component. Use the CSS patterns documented in the [Table section](#table).

### Button Component Example

```svelte
<!-- src/lib/components/ui/Button.svelte -->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
    size?: 'sm' | 'md' | 'lg' | 'icon';
    disabled?: boolean;
    type?: 'button' | 'submit' | 'reset';
    onclick?: () => void;
    children: Snippet;
  }

  let {
    variant = 'primary',
    size = 'md',
    disabled = false,
    type = 'button',
    onclick,
    children
  }: Props = $props();
</script>

<button
  {type}
  {disabled}
  class="btn btn-{variant} {size !== 'md' ? `btn-${size}` : ''}"
  onclick={onclick}
>
  {@render children()}
</button>
```

### Theme Store

```typescript
// src/lib/stores/theme.ts
import { writable } from 'svelte/store';

type Theme = 'light' | 'dark';

function createThemeStore() {
  const { subscribe, set, update } = writable<Theme>('light');

  return {
    subscribe,
    setTheme: (theme: Theme) => {
      document.documentElement.setAttribute('data-theme', theme);
      localStorage.setItem('theme', theme);
      set(theme);
    },
    toggle: () => {
      update(current => {
        const next = current === 'light' ? 'dark' : 'light';
        document.documentElement.setAttribute('data-theme', next);
        localStorage.setItem('theme', next);
        return next;
      });
    },
    init: () => {
      const saved = localStorage.getItem('theme') as Theme | null;
      const preferred = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
      const theme = saved || preferred;
      document.documentElement.setAttribute('data-theme', theme);
      set(theme);
    }
  };
}

export const theme = createThemeStore();
```

### All Svelte Stores

The application uses 15 Svelte stores for state management:

| Store | File | Purpose |
|-------|------|---------|
| `theme` | `theme.ts` | Light/dark theme management |
| `agentStore` | `agents.ts` | Agent CRUD and selection state |
| `localeStore` | `locale.ts` | i18n language management |
| `workflowStore` | `workflows.ts` | Workflow execution state |
| `activityStore` | `activity.ts` | Real-time activity feed |
| `tokenStore` | `tokens.ts` | LLM token usage metrics |
| `streamingStore` | `streaming.ts` | Streaming workflow execution |
| `validationStore` | `validation.ts` | Human-in-the-loop validation |
| `promptStore` | `prompts.ts` | System prompt library |
| `llmStore` | `llm.ts` | LLM provider/model configuration |
| `mcpStore` | `mcp.ts` | MCP server management |
| `onboardingStore` | `onboarding.ts` | First-launch wizard state |
| `validationSettings` | `validation-settings.ts` | Validation configuration |

**Import Pattern**:
```typescript
import { theme } from '$lib/stores/theme';
import { agentStore, agents, isLoading } from '$lib/stores/agents';
import { localeStore, locale, localeInfo } from '$lib/stores/locale';
```

### Lucide Icons in Svelte

Option 1: Use `lucide-svelte` package:

```bash
npm install lucide-svelte
```

```svelte
<script>
  import { Settings, Bot, Search } from 'lucide-svelte';
</script>

<Settings size={16} />
<Bot size={24} class="text-accent" />
```

Option 2: Create wrapper component:

```svelte
<!-- src/lib/components/ui/Icon.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import * as lucide from 'lucide';

  interface Props {
    name: string;
    size?: number;
    class?: string;
  }

  let { name, size = 16, class: className = '' }: Props = $props();
  let iconRef: HTMLElement;

  onMount(() => {
    if (iconRef) {
      lucide.createIcons({ nodes: [iconRef] });
    }
  });
</script>

<i
  bind:this={iconRef}
  data-lucide={name}
  style="width: {size}px; height: {size}px;"
  class={className}
></i>
```

### Global Styles Import

```css
/* src/app.css */
@import './styles/variables.css';
@import './styles/reset.css';
@import './styles/components.css';
@import './styles/utilities.css';
```

---

## Quick Reference

### Color Values

| Name | Light | Dark |
|------|-------|------|
| Background Primary | `#ffffff` | `#2b2d31` |
| Background Secondary | `#f8f9fa` | `#1e1f22` |
| Text Primary | `#212529` | `#ffffff` |
| Text Secondary | `#495057` | `#b5bac1` |
| Accent | `#94EFEE` | `#94EFEE` |
| Secondary Button | `#FE7254` | `#FE7254` |

### Spacing Quick Reference

| Token | Value |
|-------|-------|
| xs | 4px |
| sm | 8px |
| md | 16px |
| lg | 24px |
| xl | 32px |
| 2xl | 48px |

### Border Radius Quick Reference

| Token | Value |
|-------|-------|
| sm | 4px |
| md | 8px |
| lg | 12px |
| xl | 16px |
| full | 9999px |

### Layout Quick Reference

| Element | Expanded | Collapsed |
|---------|----------|-----------|
| Left Sidebar | 280px | 60px |
| Right Sidebar | 320px | 48px |
| Floating Menu | 60px (height) | - |

### UI Components Quick Reference

| Component | Variants | Sizes |
|-----------|----------|-------|
| Button | primary, secondary, ghost, danger | sm, md, lg, icon |
| Badge | primary, success, warning, error | - |
| StatusIndicator | idle, running, completed, error | sm, md, lg |
| Skeleton | text, circular, rectangular | custom |
| Spinner | - | sm, md, lg, custom |
| Input | text, password, email, number, search, url | - |
