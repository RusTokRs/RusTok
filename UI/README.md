# RusToK UI (FFA Design System)

Internal UI workspace for shared design tokens, multi-framework adapters, and autonomous UI Workbenches.

## Architecture & Governance

Detailed architecture, FFA boundary guarantees, and parity rules are documented in:
- [UI Workbench Architecture](../docs/UI/ui-workbench-architecture.md)
- [UI Landscape & Framework Split](../docs/UI/README.md)
- [Cross-Framework API Contracts](./docs/api-contracts.md)

## Workspace Structure

- `tokens/`: Design tokens specification:
  - `tokens.json`: Canonical machine-readable tokens (radii, spacing, palette, shadows). This is the only
    radius scale in the system.
  - `base.css`: CSS custom properties for web runtimes. Both hosts import it (`apps/admin/input.css`,
    `apps/storefront/assets/input.css`) and both Tailwind configs derive `borderRadius` from
    `var(--iu-radius-*)`, so the radius a browser renders is the radius Flutter gets. The dark block
    targets `.dark` (what the host sets) and `[data-theme="dark"]` (attribute-based hosts).
  - `rustok_tokens.g.dart`: Generated Dart tokens for Flutter (`npm run generate:tokens:flutter`).
  - `npm run verify:ui:token-parity` pins the chain tokens.json → base.css → Tailwind → generated
    artifacts; a host that invents its own `--radius` fails it.
- `next/`: Next.js (React) component primitives.
- `docs/`: Component API contracts and design specs.

## Autonomous UI Workbenches

Each host environment features its own autonomous dev surface (zero cross-runtime dependency):

| Host / Target | Dev Surface | How to Access |
| :--- | :--- | :--- |
| **Next.js Admin** (`apps/next-admin`) | `/dashboard/design-system` | `pnpm --prefix apps/next-admin dev` -> Sidebar **Operations** -> **Design System** |
| **Leptos Admin** (`apps/admin`) | `/design-system` | `trunk serve` (in `apps/admin`) -> Navigate to `http://localhost:8080/design-system` |
| **Dioxus Adapter** (`crates/ui/rustok-ui/dioxus`) | Native component suite | Used in desktop/web shells (`apps/dioxus-admin`) |
| **Flutter Mobile** (`rustok_mobile`) | Typed Dart Tokens | `npm run generate:tokens:flutter` generates `rustok_tokens.g.dart` |

## Verification & CI Gates

```bash
# Verify tokens, variant parity, and workbench presence across frameworks:
npm run verify:ui:parity

# Regenerate Flutter tokens from tokens.json:
npm run generate:tokens:flutter
```
