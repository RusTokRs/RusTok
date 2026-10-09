---
id: doc://docs/UI/ui-workbench-architecture.md
kind: architecture_guide
language: markdown
status: active
---

# UI Workbench Architecture (FFA Design System)

This document describes the architecture of the **RusToK UI Workbench** and cross-framework Design System.

---

## 1. Motivation: Why Not Classic Storybook?

1. **No Wasm / Rust Support:** Classic Storybook only supports JavaScript/TypeScript (React, Vue, Svelte). It cannot natively compile or render Leptos/Dioxus components.
2. **Zero Dependency Bloat:** Avoids 400+ MB of `node_modules` and brittle bundler plugins (Webpack/Vite conflicts with Next.js App Router and React 19).
3. **Runtime Autonomy:** Next.js and Leptos run their own dev galleries locally without requiring a mock environment or synthetic providers.

---

## 2. FFA (Fluid Frontend Architecture) Compliance

The UI Workbench follows RusToK FFA principles ([`docs/UI/module-package-architecture.md`](./module-package-architecture.md)):
- **Zero Runtime Coupling:** The Next.js host, the Leptos host, and future Flutter clients are completely autonomous. Neither requires the other to be running in development or build pipelines.
- **Contract Parity:** Component variants (`ButtonVariant`), sizes (`Size`), and states are defined as headless contracts and mirrored across platforms.
- **Single Source of Truth for Tokens:** All design tokens originate in [`UI/tokens/tokens.json`](../../UI/tokens/tokens.json) and [`UI/tokens/base.css`](../../UI/tokens/base.css).

```
                      ┌──────────────────────────────────────┐
                      │    CANONICAL DESIGN SYSTEM TOKENS    │
                      │  • UI/tokens/tokens.json             │
                      │  • UI/tokens/base.css                │
                      │  • crates/ui/rustok-ui (tokens/types)│
                      └──────────────────┬───────────────────┘
                                         │
         ┌───────────────────┬───────────┴───────────┬───────────────────┐
         ▼                   ▼                       ▼                   ▼
┌─────────────────┐ ┌─────────────────┐     ┌─────────────────┐ ┌─────────────────┐
│  Leptos / WASM  │ │  Dioxus / RSX   │     │   Next.js / TS  │ │  Flutter / Dart │
│  (Leptos Host)  │ │ (Desktop / Web) │     │  (Next-Admin)   │ │  (Mobile Host)  │
├─────────────────┤ ├─────────────────┤     ├─────────────────┤ ├─────────────────┤
│ • rustok-ui-    │ │ • rustok-ui-    │     │ • shared/ui/    │ │ • Dart tokens:  │
│   leptos        │ │   dioxus        │     │   shadcn        │ │   rustok_tokens │
│ • /design-system│ │ • apps/         │     │ • /dashboard/   │ │ • Code generator│
│   in apps/admin │ │   dioxus-admin  │     │   design-system │ │   script        │
└─────────────────┘ └─────────────────┘     └─────────────────┘ └─────────────────┘
```

---

## 3. Multi-Framework Implementations (FFA)

### Next.js Host (`apps/next-admin`)
- **Route:** `/dashboard/design-system`
- **Features:**
  - Interactive Button Inspector with dynamic props (variant, size, disabled, loading).
  - Pure TSX code snippet generator with clipboard copy.
  - Component baseline matrix (Buttons, Badges, Inputs, Switches, Checkboxes, Alerts).
  - 4 Composite recipes:
    1. Confirm Delete Dialog (Destructive action guard)
    2. Resource Filter Toolbar (Search & Tag filter bar)
    3. Save Action Toolbar (Dirty state and loading spinner simulation)
    4. Entity Summary Card (Product/resource summary with status, metrics, and actions)

### Leptos Host (`apps/admin`)
- **Route:** `/design-system`
- **Features:**
  - Native WASM rendering of `leptos_ui` / `rustok_ui_leptos` primitives.
  - Interactive reactive signal playground for Buttons and Form Controls (Input, Textarea, Switch, Checkbox).
  - Parity-verified matrix matching the Next.js visual states.
  - 4 Composite recipes matching the Next.js host with zero JavaScript bridges.

### Dioxus Adapter (`crates/ui/rustok-ui/dioxus` & `apps/dioxus-admin`)
- Native Dioxus 0.6 component suite wrapping shared `rustok-ui` tokens and contracts.
- Includes `Button`, `Badge`, `Card`, `Input`, `Checkbox`, `Switch`, `Alert`, `Separator`, etc.
- Provides identical styling and headless behavior for desktop (Tao/Wry) and web shells.

### Flutter Support (`rustok_mobile/packages/app_ui_kit`)
- **Tokens Generator:** `npm run generate:tokens:flutter` (via [`scripts/generate/generate-flutter-tokens.mjs`](../../scripts/generate/generate-flutter-tokens.mjs)).
- Generates `rustok_tokens.g.dart` providing `RusTokRadius`, `RusTokSpacing`, and `RusTokColors`.
- **Component Kit:**
  - `RusTokButton`: 6 variants, 3 sizes, loading and disabled states.
  - `RusTokBadge`: 6 variants matching web design tokens.
  - `RusTokCard`: Compound card layout (`Header`, `Title`, `Description`, `Content`, `Footer`).
  - `RusTokInput`: Text field with token styling, error states, and prefix/suffix support.
  - `RusTokCheckbox`: Token-styled checkbox with checkmark.
  - `RusTokSwitch`: Token-styled animated toggle switch.
  - `RusTokSeparator`: Horizontal and vertical divider lines.
  - `RusTokAvatar`: Profile/entity avatar with image and text fallback.

---

## 4. Verification and CI Gates

Cross-framework parity is enforced automatically via:
```bash
npm run verify:ui:parity
```
This script checks:
1. `UI/tokens/tokens.json` structure and CSS variable parity in `base.css`.
2. Generated Dart tokens freshness and correctness.
3. Enum parity between Rust `ButtonVariant` and React `buttonVariants`.
4. Rust adapter completeness across Dioxus and Leptos (`Button`, `Badge`, `Card`, `Input`, `Checkbox`, `Switch`).
5. Flutter component kit completeness in `app_ui_kit`.
6. Existence and mounting of Workbench routes and full composite recipe sets in both admin hosts.
