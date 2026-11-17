---
name: Precision Glass Studio
colors:
  surface: '#faf8fe'
  surface-dim: '#dad9df'
  surface-bright: '#faf8fe'
  surface-container-lowest: '#ffffff'
  surface-container-low: '#f4f3f8'
  surface-container: '#eeedf3'
  surface-container-high: '#e9e7ed'
  surface-container-highest: '#e3e2e7'
  on-surface: '#1a1b1f'
  on-surface-variant: '#414753'
  inverse-surface: '#2f3034'
  inverse-on-surface: '#f1f0f5'
  outline: '#717785'
  outline-variant: '#c1c6d6'
  surface-tint: '#005cbb'
  primary: '#0059b5'
  on-primary: '#ffffff'
  primary-container: '#0071e3'
  on-primary-container: '#fcfbff'
  inverse-primary: '#abc7ff'
  secondary: '#4a47d2'
  on-secondary: '#ffffff'
  secondary-container: '#6462ec'
  on-secondary-container: '#fffbff'
  tertiary: '#006a26'
  on-tertiary: '#ffffff'
  tertiary-container: '#008633'
  on-tertiary-container: '#f1ffec'
  error: '#ba1a1a'
  on-error: '#ffffff'
  error-container: '#ffdad6'
  on-error-container: '#93000a'
  primary-fixed: '#d7e2ff'
  primary-fixed-dim: '#abc7ff'
  on-primary-fixed: '#001b3f'
  on-primary-fixed-variant: '#00458f'
  secondary-fixed: '#e2dfff'
  secondary-fixed-dim: '#c2c1ff'
  on-secondary-fixed: '#0c006b'
  on-secondary-fixed-variant: '#332dbc'
  tertiary-fixed: '#72fe88'
  tertiary-fixed-dim: '#53e16f'
  on-tertiary-fixed: '#002107'
  on-tertiary-fixed-variant: '#00531c'
  background: '#faf8fe'
  on-background: '#1a1b1f'
  surface-variant: '#e3e2e7'
typography:
  display-lg:
    fontFamily: Inter
    fontSize: 32px
    fontWeight: '600'
    lineHeight: 40px
    letterSpacing: -0.025em
  headline-md:
    fontFamily: Inter
    fontSize: 22px
    fontWeight: '600'
    lineHeight: 28px
    letterSpacing: -0.015em
  headline-sm:
    fontFamily: Inter
    fontSize: 17px
    fontWeight: '600'
    lineHeight: 22px
    letterSpacing: -0.01em
  body-lg:
    fontFamily: Inter
    fontSize: 15px
    fontWeight: '400'
    lineHeight: 22px
    letterSpacing: -0.005em
  body-md:
    fontFamily: Inter
    fontSize: 13px
    fontWeight: '400'
    lineHeight: 18px
    letterSpacing: 0em
  body-sm:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: '400'
    lineHeight: 16px
    letterSpacing: 0em
  code-lg:
    fontFamily: JetBrains Mono
    fontSize: 14px
    fontWeight: '500'
    lineHeight: 22px
    letterSpacing: -0.01em
  code-md:
    fontFamily: JetBrains Mono
    fontSize: 12px
    fontWeight: '400'
    lineHeight: 18px
    letterSpacing: 0em
  code-sm:
    fontFamily: JetBrains Mono
    fontSize: 11px
    fontWeight: '400'
    lineHeight: 16px
    letterSpacing: 0.01em
  label-md:
    fontFamily: Inter
    fontSize: 11px
    fontWeight: '600'
    lineHeight: 14px
    letterSpacing: 0.04em
  label-caps:
    fontFamily: Inter
    fontSize: 10px
    fontWeight: '600'
    lineHeight: 12px
    letterSpacing: 0.06em
rounded:
  sm: 0.25rem
  DEFAULT: 0.5rem
  md: 0.75rem
  lg: 1rem
  xl: 1.5rem
  full: 9999px
spacing:
  gutter: 1rem
  gutter-dense: 0.5rem
  margin: 1rem
  margin-compact: 0.5rem
  space-xs: 0.25rem
  space-sm: 0.5rem
  space-md: 0.75rem
  space-lg: 1.25rem
  space-xl: 2rem
---

## Brand & Style

This design system embodies the calm, uncompromising clarity of modern macOS utility architecture. Built specifically for database administrators, backend architects, and software engineers, the aesthetic reconciles dense technical payloads with Apple-inspired human interface principles: pure optical balance, disciplined whitespace, crisp hairline division, and subtle materials.

The emotional signature is surgical composure. Data analysis and query construction can induce cognitive fatigue; the interface responds with a quiet environment that reduces anxiety through structured hierarchy, immediate readability, and tactile restraint. 

Key pillars:
- **Materiality over Decoration:** Subtle glassmorphism (`backdrop-filter: blur(20px)`), luminous white canvases, and ultra-fine border paths replace arbitrary visual styling.
- **Strict Light Clarity:** Light mode is treated as an intentional, high-end studio workspace—crisp contrast, warm-tinted neutral foundations, and deep charcoal glyphs.
- **Architectural Bento Alignment:** Modules snap into structured, proportional Bento containers with consistent inner radiuses and quiet background shifts.

## Colors

The palette relies on pure optical whites, layered off-white zinc tones, and Apple-grade system accents. Color is an operational signal, never passive decoration.

### Surface Tiers
- **Canvas Base (`#F5F5F7`):** The master framing canvas. Grounds window borders, utility chrome, and structural dividers.
- **Surface Translucent (`rgba(255, 255, 255, 0.72)`):** Used for pinned sidebars, navigation bars, and top-level inspector headers with hardware-accelerated blur.
- **Surface Raised (`#FFFFFF`):** Primary Bento card tier, data grid canvas, and active query editor surface.
- **Surface Subtle / Hover (`#F8F9FA`):** Inactive tabs, table header rows, and hover feedback regions.

### Borders & Dividers
- **Hairline Border (`#E5E5EA`):** Structural 1px division separating sidebars, tabs, and Bento panels.
- **Subtle Grid Border (`#E2E8F0`):** Data cell borders and horizontal rule dividers inside the table viewer.

### Functional Accents
- **Primary Accent (`#0071E3`):** Reserved strictly for primary callouts, selected navigation pills, active query runs, and focus rings.
- **Secondary Accent (`#5E5CE6`):** Schema identifiers, SQL joins, foreign keys, and indexed badges.
- **Success / Connected (`#34C759`):** Live database status, successful transaction commits, and ping latency indicators.
- **Warning / Uncommitted (`#FF9500`):** Mutating transactions, unsaved buffer changes, and index scans.
- **Destructive / Error (`#FF3B30`):** Syntax violations, failed connections, and drop/truncate confirmations.

### Ink Tokens
- **Text Primary (`#1D1D1F`):** Headings, active SQL text, data grid values, and active interface labels.
- **Text Secondary (`#6E6E73`):** Metadata, table column types, query execution timings, and sidebar badges.
- **Text Tertiary (`#86868B`):** Ghost placeholders, null values in cells, and hotkey hints.

## Typography

The typography strategy leverages two distinct roles: `Inter` tuned to mimic the neutral geometry and balance of Apple's system typeface for application chrome, and `JetBrains Mono` for developer-facing code syntax, tabular figures, and schema metadata.

- **Proportional UI Type:** Inter is calibrated with negative tracking on displays (`headline-md`, `display-lg`) for a tight, editorial presence. All interactive labels and navigational items use `13px` (`body-md`) or `12px` (`body-sm`) to support high spatial density without visual friction.
- **Monospace Code & Data Engine:** `JetBrains Mono` handles SQL queries, cell values in data grids, foreign key strings, and execution plans. It enforces fixed-width columnar alignment so large number sets align neatly along decimal points.
- **Sub-labeling & Types:** Data types (e.g., `VARCHAR(255)`, `TIMESTAMPTZ`, `UUID`) are styled exclusively in `code-sm` using uppercase treatment and `Text Secondary` styling to contrast against dynamic cell data.

## Layout & Spacing

Layouts follow a structured desktop-first application workbench: a persistent collapsible navigation sidebar (default 240px), a unified top utility bar (44px height), and a flexible Bento-arranged workspace.

### Bento Architecture
- Content panels (Data Viewer, Query Workspace, Schema Tree, Performance Inspector) inhabit individual modular Bento tiles.
- Gaps between Bento cards are pinned to `gutter-dense` (`0.5rem` / 8px) or `gutter` (`1rem` / 16px) depending on density mode.
- Cards maintain internal edge padding of `space-md` (`0.75rem`) for inspection panels, scaling to `space-lg` (`1.25rem`) for analytical dashboard views.

### Vertical Cadence & Rhythm
- Standard interactive elements (inputs, single-line buttons, breadcrumbs) use a base height of 28px (compact) or 32px (standard).
- Table rows are locked to 28px row-height in high-density modes, expanding to 36px in comfortable mode.
- Margin bounding boxes adhere strictly to the `4px` sub-grid.

## Elevation & Depth

Depth in this system avoids heavy, muddy drop shadows. Visual layering relies on translucency, surface fills, and ultra-fine border lines.

### Depth Hierarchy
1. **Layer 0 (Canvas Base):** Ground color `#F5F5F7`. Recessed beneath all active panels.
2. **Layer 1 (Bento Panels & Workspaces):** Pure white `#FFFFFF` surface with a continuous 1px outline of `#E5E5EA`. Ambient lift is provided by a feather-light shadow: `0 1px 2px rgba(0, 0, 0, 0.04), 0 4px 12px rgba(0, 0, 0, 0.02)`.
3. **Layer 2 (Floating Utility Toolbars & Filter Bars):** Dynamic glass with `background: rgba(255, 255, 255, 0.82)`, `backdrop-filter: blur(16px)`, and a crisp border of `rgba(0, 0, 0, 0.08)`. Shadow: `0 4px 20px rgba(0, 0, 0, 0.06)`.
4. **Layer 3 (Modals, Context Menus, and Autocomplete Dropdowns):** `background: rgba(255, 255, 255, 0.95)`, `backdrop-filter: blur(24px)`. Rimmed with `#E5E5EA` and sustained by Apple-grade diffusion: `0 12px 32px -4px rgba(0, 0, 0, 0.12), 0 4px 12px -2px rgba(0, 0, 0, 0.06)`.

### Border Integrity
Every surface elevation level maintains a 1px inner or perimeter boundary to retain outline definition on high-DPI displays against pale surfaces.

## Shapes

The design system uses soft, continuous squircle geometry reminiscent of macOS window management.

- **Bento Enclosure Cards:** `rounded-xl` (16px) or `rounded-2xl` (20px) outer radius creates structural modularity. Nested components within containers scale downward proportionally (8px or 10px) to preserve concentric border harmony.
- **Controls & Buttons:** Standard action buttons and text input fields use 8px (`rounded-md`) to 10px corner rounding.
- **Pills & Status Indicators:** Connection indicators, schema badges, and transaction chips use full pill rounding (`9999px`).
- **Data Grids:** Sharp inner grid intersections with rounded exterior container corners ensure cells remain rectangular for readability while framing cleanly within the window.

## Components

### Buttons
- **Primary:** Background `#0071E3`, text `#FFFFFF`, border `1px solid rgba(0, 113, 227, 0.2)`. On hover: `#0077ED`. Height: 32px, padding: 0 14px, radius: 8px. Subtle inner inset highlight (`inset 0 1px 0 rgba(255, 255, 255, 0.2)`).
- **Secondary / Glass:** Background `rgba(255, 255, 255, 0.85)`, text `#1D1D1F`, border `1px solid #E5E5EA`. On hover: background `#F8F9FA`.
- **Run Query Action:** Dual button segmented control. Left pane: Primary "Run" button with right-pointing triangle glyph. Right pane: Dropdown chevron for execution settings (limit, explain analyze).

### Data Grid (Crisp Table Engine)
- **Header:** Height 30px, background `#F8F9FA`, bottom border `1px solid #E2E8F0`. Header labels in `label-caps` (`Text Secondary`) with trailing icon for column data type.
- **Cells:** Padding `4px 10px`, vertical alignment middle, font `code-md` (`12px`). Right/bottom border `1px solid #E2E8F0`.
- **Selection State:** Active row highlighted with `rgba(0, 113, 227, 0.06)`. Active cell bordered with `2px solid #0071E3`.
- **Null Value Treatment:** Rendered in italic `code-sm`, color `#86868B`, reading `NULL`.

### Inputs & Query Editor
- **Input Fields:** Background `#FFFFFF`, border `1px solid #E5E5EA`, radius 8px, font `body-md`. Focus state introduces a 3px outer glow in `rgba(0, 113, 227, 0.25)` with `#0071E3` border.
- **SQL Editor Canvas:** Pure `#FFFFFF` background framed within a Bento card. Line numbers in `JetBrains Mono` (`code-sm`, `#86868B`). Syntax highlighting uses system blues (`#0071E3`), indigos (`#5E5CE6`), and teals, avoiding low-contrast pastels.

### Bento Cards
- Pinned structural blocks: Background `#FFFFFF`, border `1px solid #E5E5EA`, corner radius 16px.
- Internal header bar: 40px height with title in `headline-sm`, right-aligned micro-action buttons (filter, export, collapse).

### Status Chips & Badges
- **Index / Key Chip:** Background `#F5F5F7`, border `1px solid #E5E5EA`, radius 4px, font `code-sm`, color `#6E6E73`.
- **Foreign Key Indicator:** Background `rgba(94, 92, 230, 0.1)`, color `#5E5CE6`, text: `FK ↗`.
- **Health / Connection:** 6px solid dot indicator (`#34C759`) wrapped in an 8px circular blur halo alongside server latency (e.g., `12ms` in `code-sm`).