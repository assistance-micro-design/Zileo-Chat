# Zileo Chat 3 - Design System

Reference documentation for the Zileo-Chat-3 design system. For CSS implementation details, see `src/lib/styles/`.

## Table of Contents

1. [Design Principles](#design-principles)
2. [Color Palette](#color-palette)
3. [Typography](#typography)
4. [Spacing System](#spacing-system)
5. [Border and Radius](#border-and-radius)
6. [Shadows](#shadows)
7. [Transitions and Animations](#transitions-and-animations)
8. [Z-Index Layers](#z-index-layers)
9. [Layout](#layout)
10. [UI Components](#ui-components)
11. [Form Components](#form-components)
12. [Navigation Components](#navigation-components)
13. [Status and Feedback](#status-and-feedback)
14. [Utility Classes](#utility-classes)
15. [Icons](#icons)
16. [Accessibility](#accessibility)
17. [Dark Mode](#dark-mode)
18. [Quick Reference](#quick-reference)

---

## Design Principles

- **Professional and Clean**: Minimal interface, focus on content
- **Dual Theme**: Full light/dark mode support, synced with OS preference
- **Component-Based**: Reusable, consistent Svelte 5 components
- **Accessible**: WCAG 2.1 AA compliance
- **Responsive**: Desktop-first with collapsible sidebars

### Tech Stack

| Aspect | Technology |
|--------|------------|
| Icons | Lucide Icons via `@lucide/svelte` |
| Fonts | Signika (UI) + JetBrains Mono (Code) |
| CSS | CSS custom properties + utility classes |
| Theming | `data-theme` attribute on root element |

---

## Color Palette

### Brand Colors

| Name | Hex | Usage |
|------|-----|-------|
| Primary (Accent) | `#94EFEE` | Primary buttons, active links, focus rings |
| Primary Hover | `#7de6e5` | Hover state for primary |
| Secondary | `#FE7254` | Secondary buttons, important CTAs |
| Secondary Hover | `#fe5a3d` | Hover state for secondary |

### Theme Colors

| Token | Light | Dark | Usage |
|-------|-------|------|-------|
| bg-primary | `#ffffff` | `#2b2d31` | Main content background |
| bg-secondary | `#f8f9fa` | `#1e1f22` | Sidebar, cards, footer |
| bg-tertiary | `#f1f3f5` | `#161719` | Nested elements, code blocks |
| bg-hover | `#e9ecef` | `#35373c` | Hover states |
| bg-active | `#dee2e6` | `#3f4147` | Active/pressed states |
| text-primary | `#212529` | `#ffffff` | Main text, headings |
| text-secondary | `#495057` | `#b5bac1` | Descriptions, labels |
| text-tertiary | `#6c757d` | `#80848e` | Hints, timestamps |
| text-inverse | `#ffffff` | `#212529` | Text on contrasting backgrounds |
| border | `rgba(33,37,41,0.15)` | `#3f4147` | Standard borders |
| border-light | `rgba(33,37,41,0.1)` | `#35373c` | Subtle separators |
| border-dark | `rgba(33,37,41,0.25)` | `#4e5058` | Emphasized borders |

### Semantic Colors

| Name | Hex | Light Background | Usage |
|------|-----|------------------|-------|
| Success | `#10b981` | `#d1fae5` | Successful operations |
| Warning | `#f59e0b` | `#fef3c7` | Warnings, caution states |
| Error | `#ef4444` | `#fee2e2` | Errors, destructive actions |

### Status Colors

| Status | Hex | Visual |
|--------|-----|--------|
| Idle | `#6c757d` | Grey dot |
| Running | `#3b82f6` | Blue dot with pulse animation |
| Completed | `#10b981` | Green dot |
| Error | `#ef4444` | Red dot |

See `src/lib/styles/` for the full CSS variable definitions.

---

## Typography

### Font Families

| Purpose | Font | Fallbacks |
|---------|------|-----------|
| UI text | Signika | system-ui, sans-serif |
| Code | JetBrains Mono | Fira Code, monospace |

Fonts are loaded from Google Fonts. See `src/app.html` for the import.

### Font Sizes

| Token | Size | Pixels | Usage |
|-------|------|--------|-------|
| xs | 0.75rem | 12px | Badges, timestamps, hints |
| sm | 0.875rem | 14px | Body small, labels, descriptions |
| base | 1rem | 16px | Body text, inputs |
| lg | 1.125rem | 18px | Section titles, card titles |
| xl | 1.25rem | 20px | Page subtitles, modal titles |
| 2xl | 1.5rem | 24px | Page titles, section headers |

### Font Weights

| Token | Weight | Usage |
|-------|--------|-------|
| normal | 400 | Body text |
| medium | 500 | Labels, nav items, emphasis |
| semibold | 600 | Headings, card titles, buttons |
| bold | 700 | Strong emphasis, badges |

### Line Heights

| Token | Value | Usage |
|-------|-------|-------|
| tight | 1.25 | Headings, buttons |
| base | 1.5 | Body text |
| relaxed | 1.75 | Messages, long-form content |

---

## Spacing System

| Token | Size | Pixels | Usage |
|-------|------|--------|-------|
| xs | 0.25rem | 4px | Tight gaps, badge padding |
| sm | 0.5rem | 8px | Small gaps, compact elements |
| md | 1rem | 16px | Standard gaps, card padding |
| lg | 1.5rem | 24px | Section spacing, large gaps |
| xl | 2rem | 32px | Page padding, major sections |
| 2xl | 3rem | 48px | Large separations |

### Application Guidelines

- **Card padding**: lg (24px)
- **Form group margin**: lg (24px)
- **Button padding**: sm vertical, md horizontal (8px 16px)
- **Badge padding**: xs vertical, sm horizontal (4px 8px)
- **Icon gaps**: sm (8px)

---

## Border and Radius

| Token | Size | Pixels | Usage |
|-------|------|--------|-------|
| sm | 0.25rem | 4px | Small elements, code blocks |
| md | 0.5rem | 8px | Buttons, inputs, nav items |
| lg | 0.75rem | 12px | Cards, modals |
| xl | 1rem | 16px | Large cards, validation modals |
| full | 9999px | Pill | Badges, status indicators, spinners |

Border patterns use three levels: standard (1px, border color), light (1px, border-light for subtle separators), and active/focus (2px, accent color). Error states use 2px with the error color.

---

## Shadows

| Token | Usage |
|-------|-------|
| xs | Subtle elevation for inputs |
| sm | Cards, dropdowns |
| md | Floating elements, popovers |
| lg | Queue indicators, floating buttons |
| xl | Modals, validation dialogs |

Focus rings use a 3px accent-light box-shadow. See `src/lib/styles/` for exact shadow values.

---

## Transitions and Animations

### Durations

| Speed | Duration | Easing | Usage |
|-------|----------|--------|-------|
| fast | 150ms | ease-out | Buttons, hover states, icons |
| base | 200ms | ease-out | Sidebar collapse, tab switches |
| slow | 300ms | ease-out | Modal open/close, page transitions |

All transitions use `cubic-bezier(0.4, 0, 0.2, 1)`.

### Built-in Animations

| Animation | Usage | Duration |
|-----------|-------|----------|
| Pulse | Running status indicator | 2s infinite |
| Spin | Loading spinner | 0.8s linear infinite |
| Fade In | New messages | 0.3s ease-in |
| Slide Up | Queue notifications | 0.3s ease-out |
| Skeleton Shimmer | Loading placeholders | 1.5s ease-in-out infinite |

---

## Z-Index Layers

| Layer | Value | Usage |
|-------|-------|-------|
| Base content | auto | Normal document flow |
| Dropdowns | 1000 | Dropdown menus |
| Sticky | 1020 | Sticky headers |
| Fixed | 1030 | Floating menu |
| Modal backdrop | 1040 | Dark overlay behind modals |
| Modal | 1050 | Modal dialogs |
| Popover | 1060 | Popovers, context menus |
| Tooltip | 1070 | Tooltips (highest layer) |

---

## Layout

### Dimensions

| Element | Expanded | Collapsed |
|---------|----------|-----------|
| Left Sidebar | 280px | 60px |
| Right Sidebar (Activity) | 320px | 48px |
| Floating Menu (height) | 60px | - |

### Structure

The app uses a vertical flex layout: a fixed floating menu at the top, with a horizontal flex body containing the left sidebar, main content area, and optional right sidebar. The sidebar collapses smoothly with a CSS transition.

See `src/lib/components/layout/` for `AppContainer.svelte`, `FloatingMenu.svelte`, and `Sidebar.svelte`.

---

## UI Components

All UI components live in `src/lib/components/ui/`. They follow Svelte 5 patterns (runes, snippets, `$props()`).

### Button

Inline flex element with icon support. Four variants: **primary** (turquoise, dark text), **secondary** (coral, white text), **ghost** (transparent, subtle text), **danger** (red, white text). Four sizes: sm, md (default), lg, icon (square). Disabled state reduces opacity to 0.5.

### Card

Container with optional header (title + description), body, and footer sections. Uses snippet slots for flexible content. Light shadow, rounded corners (lg), border. Footer has a secondary background.

### Modal

Centered dialog over a blurred backdrop. Contains header (title + close button), scrollable body, and footer with action buttons. Maximum width 600px, maximum height 90vh. Closes on Escape key.

### DeleteConfirmModal

Reusable confirmation dialog for destructive or significant actions. Two visual variants: **danger** (red confirm button, for delete operations) and **primary** (accent confirm button, for non-destructive confirmations like save or regenerate). Shows item name in bold, optional warning message. All labels use i18n keys.

### Table

Styled via CSS classes (`.table`, `.table-container`) rather than a dedicated component. Full-width with collapsed borders. Header row has secondary background and semibold text. Body rows highlight on hover. Wrapped in a scrollable container for overflow.

### Skeleton

Loading placeholder with three variants: **text** (single line), **circular** (avatar), **rectangular** (image/card). Supports custom width, height, and size. Animates with a shimmer effect by default.

### Spinner

Circular loading indicator using a border animation. Accent-colored top border rotates over a neutral border ring.

### ProgressBar

Horizontal bar showing completion percentage. Accent-colored fill with smooth width transitions. Full border-radius for pill shape.

### LanguageSelector

Self-contained language picker. Displays current locale with country flag, shows a dropdown with available languages. Persists selection to localStorage via `localeStore`.

| Code | Language |
|------|----------|
| en | English |
| fr | Francais |

### MarkdownRenderer

Renders markdown content with proper styling. Used for chat messages and documentation display.

### ToastContainer / ToastItem

Notification system. Toasts appear at the bottom of the screen, support multiple severity levels, and auto-dismiss.

### ErrorBanner

Displays error messages with appropriate styling. Used for form validation errors and operation failures.

### HelpButton

Small trigger button that shows a help tooltip on interaction.

### ContextMenu

Right-click context menu with positioned dropdown. Appears at cursor position, dismisses on outside click or Escape.

---

## Form Components

Form components follow a consistent structure: label, input element, and optional help text, wrapped in a form group with bottom margin.

### Input

Supports types: text, password, email, number, search, url. Full-width, padded, with border that transitions to accent color on focus. Focus state adds a 3px accent-light ring. Disabled state reduces opacity.

### Select

Dropdown with the same styling as inputs. Supports auto-width for use in filter bars.

### Textarea

Multi-line input with monospace font. Minimum height 100px, vertically resizable. Same focus behavior as inputs.

### Checkbox and Radio

Native elements styled with accent color. 1rem square with pointer cursor.

### Range Slider

Full-width slider with accent color track. 0.25rem height, pill-shaped.

### Filter Bar

Composite pattern combining a search box (with search icon), select filters, and action buttons in a horizontal flex layout. Wraps on narrow screens. Secondary background with medium border-radius.

---

## Navigation Components

### NavItem

Sidebar navigation link with icon and label. Three states: default (secondary text), hover (highlighted background), active (accent-light background with accent text). In collapsed sidebar mode, text hides and icon centers.

### WorkflowItem

Agent page workflow entry with status indicator dot, editable name, and delete button. Active state shows accent border and background. Delete button appears on hover only.

---

## Status and Feedback

### Badge

Pill-shaped inline label. Four semantic variants: **primary** (accent), **success** (green), **warning** (amber), **error** (red). Each uses a light background tint with matching text color. Extra-small font, medium weight.

### StatusIndicator

8px colored dot representing workflow or agent status. Four states: idle (grey), running (blue, pulsing), completed (green), error (red).

---

## Utility Classes

The application provides utility classes for common layout patterns. See `src/lib/styles/` for the full set.

### Available Utilities

| Category | Classes |
|----------|---------|
| Flexbox | `flex`, `flex-col`, `items-center`, `items-start`, `justify-between`, `justify-center`, `flex-1` |
| Grid | `grid`, `grid-cols-2`, `grid-cols-3` |
| Gap | `gap-sm`, `gap-md`, `gap-lg` |
| Margin | `mt-sm`, `mt-md`, `mt-lg`, `mb-sm`, `mb-md`, `mb-lg` |
| Typography | `text-sm`, `text-lg`, `text-secondary`, `text-tertiary`, `font-medium`, `font-semibold` |
| Misc | `truncate` (ellipsis overflow), `hidden`, `sr-only` (screen reader only) |

---

## Icons

The application uses [Lucide Icons](https://lucide.dev) via the `@lucide/svelte` package. Icons are imported individually as Svelte components.

### Icon Catalog

| Category | Icons |
|----------|-------|
| Navigation | `settings`, `bot`, `users`, `search`, `plus`, `x`, `edit`, `trash-2`, `copy`, `eye`, `chevron-left` |
| Providers | `sparkles` (Mistral), `server` (Ollama), `cpu` (Models), `globe` (Providers) |
| Theme | `palette`, `sun` (light), `moon` (dark) |
| Tools/MCP | `plug`, `tool`, `database`, `file-text`, `file-json`, `folder`, `folder-open` |
| Workflow | `activity`, `zap`, `play`, `send`, `paperclip` |
| Memory | `brain`, `file-search`, `upload`, `upload-cloud`, `download` |
| Validation | `shield-check`, `alert-triangle`, `info`, `check`, `hand` |

---

## Accessibility

### Keyboard Navigation

- **Tab / Shift+Tab**: Navigate between focusable elements
- **Enter / Space**: Activate buttons
- **Escape**: Close modals and dropdowns
- **Ctrl+Enter**: Send message (Agent page)
- **Arrow keys**: Navigate lists and options

### ARIA Guidelines

- Icon-only buttons must have `aria-label`
- Live regions use `role="status"` with `aria-live="polite"` for dynamic content
- Progress elements include descriptive `aria-label`
- Modals use `role="dialog"`, `aria-modal="true"`, and `aria-labelledby`

### Focus Management

All interactive elements display a visible focus ring (3px accent-light box-shadow) on `:focus-visible`. Form inputs additionally change their border to the accent color on focus.

### Color Contrast

- Text on light/dark backgrounds: minimum 4.5:1 ratio
- Large text (18px+): minimum 3:1 ratio
- UI components: minimum 3:1 ratio against background

### Reduced Motion

Users with `prefers-reduced-motion: reduce` enabled see no animations, instant transitions, and no scroll animations. All durations are reduced to near-zero.

### High Contrast

Users with `prefers-contrast: high` see stronger borders and secondary text rendered at primary text color for improved readability.

### Screen Reader Support

The `.sr-only` utility class hides content visually while keeping it accessible to screen readers. Use for descriptive text that would be redundant visually but is necessary for assistive technology.

---

## Dark Mode

The application supports light and dark themes, controlled by the `data-theme` attribute on the root HTML element.

### Behavior

- On first launch, the theme matches the OS preference (`prefers-color-scheme`)
- Users can toggle manually via the theme button in the floating menu (sun/moon icons)
- Selection is persisted to localStorage
- The theme store (`src/lib/stores/theme.ts`) manages state and DOM updates
- The accent color (`#94EFEE`) remains consistent across both themes
- All semantic colors (success, warning, error, status) remain consistent across themes
- Background and text colors invert: light uses white backgrounds with dark text, dark uses anthracite backgrounds with light text

---

## Quick Reference

### Component Variants

| Component | Variants | Sizes |
|-----------|----------|-------|
| Button | primary, secondary, ghost, danger | sm, md, lg, icon |
| Badge | primary, success, warning, error | - |
| StatusIndicator | idle, running, completed, error | sm, md, lg |
| Skeleton | text, circular, rectangular | custom |
| Spinner | - | sm, md, lg, custom |
| Input | text, password, email, number, search, url | - |
| DeleteConfirmModal | danger, primary | - |

### Component File Structure

Components are organized in `src/lib/components/`:

| Directory | Contents |
|-----------|----------|
| `ui/` | 19 atomic components (Button, Card, Modal, Badge, Input, Select, etc.) |
| `layout/` | AppContainer, FloatingMenu, Sidebar |
| `navigation/` | NavItem |

### Stores

The application uses Svelte stores for state management, located in `src/lib/stores/`:

| Store | Purpose |
|-------|---------|
| `theme` | Light/dark theme management |
| `agentStore` | Agent CRUD and selection state |
| `localeStore` | i18n language management |
| `workflowStore` | Workflow execution state |
| `foldersStore` | Workflow folder organization |
| `tokenStore` | LLM token usage metrics |
| `streamingStore` | Streaming workflow execution |
| `validationStore` | Human-in-the-loop validation |
| `promptStore` | System prompt library |
| `llmStore` | LLM provider/model configuration |
| `mcpStore` | MCP server management |
| `onboardingStore` | First-launch wizard state |
| `validationSettings` | Validation configuration |
