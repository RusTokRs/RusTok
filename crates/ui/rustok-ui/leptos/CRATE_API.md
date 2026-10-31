# rustok-ui-leptos / CRATE_API

## Purpose

`rustok-ui-leptos` is the canonical Leptos 0.8 design system adapter for `rustok-ui`, providing SSR-first, accessible, reactive component implementations for web storefronts and administration dashboards.

## Architecture & Responsibilities

- **Fluid Frontend Architecture (FFA) Adapter**: Integrates framework-agnostic Tailwind class resolvers and tokens from `rustok-ui` with Leptos 0.8 reactive primitives.
- **SSR-First Hydration Parity**: Generates deterministic HTML during server-side rendering (SSR) that matches client-side hydration, preventing hydration mismatch artifacts.
- **Accessible ARIA Integration**: Implements WAI-ARIA and Radix-grade interaction semantics (focus trapping, Escape key closing, roving tabindex, `aria-expanded`, `aria-checked`, `aria-selected`).
- **Full Parity with Dioxus UI**: Ensures identical visual presentation, variant names, size modifiers, and component APIs with `rustok-ui-dioxus`.

## Public Surface

### Components
- **Actions & Controls**: `Button`, `Checkbox`, `Switch`, `Input`, `Textarea`, `Select`.
- **Feedback & Status**: `Alert`, `Badge`, `Progress`, `Spinner`, `Skeleton`.
- **Layout & Structure**: `Card` (`CardHeader`, `CardTitle`, `CardDescription`, `CardAction`, `CardContent`, `CardFooter`), `Label`, `Separator`, `Tabs` (`TabsList`, `TabsTrigger`, `TabsContent`).
- **Overlays**: `Dialog` (`DialogTrigger`, `DialogContent`, `DialogHeader`, `DialogTitle`, `DialogDescription`, `DialogFooter`, `DialogClose`, `DialogOverlay`, `DialogPortal`).
- **Data Display & Navigation**: `Avatar`, `TableOfContents`.

### Re-exported Core Types & Contracts
- All core types and tokens from `rustok-ui`:
  `ButtonVariant`, `AlertVariant`, `BadgeVariant`, `CardVariant`, `Size`, `DialogState`, `TabsState`, `TocItem`, `extract_initials`, `slugify_heading`, `merge_classes`.
- Prefixed `ui_*` aliases (`ui_button`, `ui_alert`, `ui_badge`, `ui_card`, `ui_dialog`, etc.) for consistent cross-module naming.

## Invariants & Guarantees

1. **Deterministic SSR Markup**: Initial server markup must strictly match client expectations without hydration drift.
2. **Accessible Interaction**: Interactive elements include appropriate roles (`button`, `dialog`, `tab`, `tabpanel`), state indicators (`aria-checked`, `aria-selected`, `aria-busy`), and keyboard focus rings.
3. **Native Attribute State**: State flags (`disabled`, `aria-invalid`, `aria-checked`) are expressed via native HTML/WAI-ARIA attributes rather than class mutations.

## Dependencies & Boundaries

- **Upstream Dependencies**:
  - `leptos`: Reactive UI framework for Leptos 0.8 (SSR, hydrate, csr features).
  - `rustok-ui`: Core design tokens, headless states, and CSS resolvers.
  - `serde`: Serialization for component prop models and options.
  - `web-sys`: Browser DOM interfaces on `wasm32`.
- **Downstream Consumers**:
  - `apps/admin`, `apps/storefront`.
  - All Leptos module admin and storefront packages (`crates/modules/*/admin`, `crates/modules/*/storefront`).
- **Boundary Rules**:
  - Contains no business logic, domain entities, or direct network transport calls.
  - Reactive signal bindings and route connections are managed by consuming views and modules.
