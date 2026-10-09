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
                      └──────────────────┬───────────────────┘
                                         │
         ┌───────────────────────────────┼───────────────────────────────┐
         ▼                               ▼                               ▼
┌─────────────────┐             ┌─────────────────┐             ┌─────────────────┐
│   Rust / WASM   │             │   Next.js / TS  │             │  Flutter / Dart │
│  (Leptos Host)  │             │  (Next-Admin)   │             │  (Mobile Host)  │
├─────────────────┤             ├─────────────────┤             ├─────────────────┤
│ • UI/leptos/    │             │ • shared/ui/    │             │ • Dart tokens:  │
│ • /design-system│             │ • /dashboard/   │             │   rustok_tokens │
│   route in      │             │   design-system │             │ • Code generator│
│   apps/admin    │             │   in next-admin │             │   script        │
└─────────────────┘             └─────────────────┘             └─────────────────┘
```

---

## 3. Host Implementations

### Next.js Host (`apps/next-admin`)
- **Route:** `/dashboard/design-system`
- **Features:**
  - Interactive Button Inspector with dynamic props (variant, size, disabled, loading).
  - Code generator snippet preview (Next.js TSX and Leptos Rust).
  - Component baseline matrix (Buttons, Badges, Inputs, Switches, Checkboxes, Alerts).
  - Composite recipes (Confirm Delete Dialog, Filter & Search Toolbar, Save Action Bar).

### Leptos Host (`apps/admin`)
- **Route:** `/design-system`
- **Features:**
  - Native WASM rendering of `leptos_ui` / `rustok_ui_leptos` primitives.
  - Interactive reactive signal playground.
  - Parity-verified matrix matching the Next.js visual states.

### Flutter Support (`rustok_mobile`)
- **Tokens Generator:** `npm run generate:tokens:flutter` (via [`scripts/generate/generate-flutter-tokens.mjs`](../../scripts/generate/generate-flutter-tokens.mjs)).
- Generates `UI/tokens/rustok_tokens.g.dart` providing `RusTokRadius`, `RusTokSpacing`, and `RusTokColors` for Flutter theme extensions without external runtime overhead.

---

## 4. Verification and CI Gates

Cross-framework parity is enforced automatically via:
```bash
npm run verify:ui:parity
```
This script checks:
1. `UI/tokens/tokens.json` structure and CSS variable parity in `base.css`.
2. Generated Dart tokens freshness.
3. Enum parity between Rust `ButtonVariant` and React `buttonVariants`.
4. Existence and mounting of Workbench routes in both admin hosts.
