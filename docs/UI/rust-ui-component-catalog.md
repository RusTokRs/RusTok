---
id: doc://docs/UI/rust-ui-component-catalog.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Rust UI Component Catalog

This document captures the current shared UI surface in RusToK and the division of
responsibility between `crates/ui/rustok-ui*`, `crates/ui/leptos-ui`, `UI/*`, and
app-local components.

## Sources of Shared UI

The repository currently has these levels of UI reuse:

- `crates/ui/rustok-ui/src` — framework-agnostic contracts (variants, sizes, state
  models) and the deterministic Tailwind class resolvers shared by every framework;
- `crates/ui/rustok-ui/leptos` (`rustok-ui-leptos`) and `crates/ui/rustok-ui/dioxus`
  (`rustok-ui-dioxus`) — the wrapper adapters that turn those resolvers into
  framework components;
- `crates/ui/leptos-ui` — RusToK-specific Leptos package boundary; re-exports
  `rustok_ui_leptos::*` and owns only the local helper components
  (`LanguageToggle`, rich-text helpers);
- `UI/tokens` — legacy design tokens and CSS variables consumed by the admin shell;
- `UI/leptos` (`iu-leptos`) and `UI/next/components` — the legacy parallel shared
  primitives kept for surfaces that have not migrated to `rustok-ui` yet.

App-local complex components remain inside specific host applications and are not considered part of the shared catalog until a reusable contract emerges.

## Shared Design Contract

- All host applications use a unified theming contract based on shared tokens and shadcn-compatible CSS variables.
- Leptos and Next.js components must maintain parity in purpose, visual result, and basic API, but are not required to have a literal one-to-one implementation.
- Shared UI packages remain a presentational layer and do not own transport, auth, routing, or domain behavior.

## Shared Primitives: `crates/ui/rustok-ui/leptos` ↔ `UI/next/components`

Current set of components with an explicit shared surface:

| Primitive | Leptos | Next.js | Status |
|-----------|--------|---------|--------|
| Alert | `crates/ui/rustok-ui/leptos/src/alert.rs` | app-local / shadcn path | Leptos canonical |
| Badge | `crates/ui/rustok-ui/leptos/src/badge.rs` | `UI/next/components/Badge.tsx` | parity |
| Button | `crates/ui/rustok-ui/leptos/src/button.rs` | `UI/next/components/Button.tsx` | parity |
| Checkbox | `crates/ui/rustok-ui/leptos/src/checkbox.rs` | `UI/next/components/Checkbox.tsx` | parity |
| Input | `crates/ui/rustok-ui/leptos/src/input.rs` | `UI/next/components/Input.tsx` | parity |
| Select | `crates/ui/rustok-ui/leptos/src/select.rs` | `UI/next/components/Select.tsx` | parity |
| Spinner | `crates/ui/rustok-ui/leptos/src/spinner.rs` | `UI/next/components/Spinner.tsx` | parity |
| Switch | `crates/ui/rustok-ui/leptos/src/switch.rs` | `UI/next/components/Switch.tsx` | parity |
| Textarea | `crates/ui/rustok-ui/leptos/src/textarea.rs` | `UI/next/components/Textarea.tsx` | parity |
| Avatar | `crates/ui/rustok-ui/leptos/src/avatar.rs` | `UI/next/components/Avatar.tsx` | parity |
| Skeleton | `crates/ui/rustok-ui/leptos/src/skeleton.rs` | `UI/next/components/Skeleton.tsx` | parity |
| Card / Tabs / Dialog / Label / Separator | `crates/ui/rustok-ui/leptos/src/{card,tabs,dialog,label,separator}.rs` | app-local / shadcn path | Leptos canonical |

`crates/ui/rustok-ui/leptos/src/lib.rs` and `UI/next/components/index.ts` are the entry
points for this shared primitive layer. The legacy `UI/leptos/src/*` implementations
remain available through the `iu-leptos` workspace crate only.

## Leptos-Specific Package Boundary: `crates/ui/leptos-ui`

`crates/ui/leptos-ui` holds the RusToK-specific Leptos surface for applications and module-owned UI packages. It re-exports every component from `rustok-ui-leptos` (`pub use rustok_ui_leptos::*;`) and adds the local helpers below. Current entry points:

- `Button`
- `Input`
- `Badge`
- `Alert`
- `Card`
- `CardHeader`
- `CardTitle`
- `CardDescription`
- `CardAction`
- `CardContent`
- `CardFooter`
- `Label`
- `Separator`
- `LanguageToggle`
- `RichTextHtml` for editor-free rendering of a typed, server-derived
  `RichTextView`
- `RichTextEditorFrame` for the isolated shared authoring runtime

This crate is needed where a simple shared primitive layer is insufficient and a stable package boundary within the Rust workspace is required.

## App-Local UI Not in the Shared Catalog

The following surfaces currently remain app-local and should not automatically be considered part of the shared catalog:

- `apps/next-admin/src/shared/ui/*`
- `apps/next-admin` data-table and related admin-only widgets
- `apps/admin` host-local layout/navigation components
- module-owned admin/storefront UI inside `crates/modules/rustok-*/admin` and `crates/modules/rustok-*/storefront`

If such a component starts being reused across multiple hosts or modules, it should either be promoted to `UI/*` or formalized through `crates/ui/leptos-ui` for the Leptos path.

## Verification When Changing Shared UI

- compare `crates/ui/rustok-ui/leptos` and `UI/next/components` for API drift;
- verify that shared components do not pull in domain-specific dependencies;
- update app-local docs if the host integration contract changes;
- update [UI index](./README.md) and related app docs if the boundary between shared and app-local UI changes.

## Related Documents

- [UI index](./README.md)
- [GraphQL architecture](./graphql-architecture.md)
- [Leptos admin docs](../../apps/admin/docs/README.md)
- [Leptos storefront docs](../../apps/storefront/docs/README.md)
- [Next.js admin docs](../../apps/next-admin/docs/README.md)
- [Next.js storefront docs](../../apps/next-frontend/docs/README.md)
