---
id: doc://docs/UI/rust-ui-component-catalog.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Rust UI Component Catalog

This document captures the current shared UI surface in RusToK and the division of responsibility between `UI/*`, `crates/leptos-ui`, and app-local components.

## Sources of Shared UI

The repository currently has three levels of UI reuse:

- `UI/tokens` — common design tokens and basic CSS variables;
- `UI/leptos` and `UI/next/components` — parallel shared primitives for Leptos and Next.js;
- `crates/leptos-ui` — RusToK-specific Leptos package boundary with re-exports and local helper components.

App-local complex components remain inside specific host applications and are not considered part of the shared catalog until a reusable contract emerges.

## Shared Design Contract

- All host applications use a unified theming contract based on shared tokens and shadcn-compatible CSS variables.
- Leptos and Next.js components must maintain parity in purpose, visual result, and basic API, but are not required to have a literal one-to-one implementation.
- Shared UI packages remain a presentational layer and do not own transport, auth, routing, or domain behavior.

## Shared Primitives: `UI/leptos` ↔ `UI/next/components`

Current set of components with an explicit shared surface:

| Primitive | Leptos | Next.js | Status |
|-----------|--------|---------|--------|
| Alert | `UI/leptos/src/alert.rs` | app-local / shadcn path | Leptos canonical |
| Badge | `UI/leptos/src/badge.rs` | `UI/next/components/Badge.tsx` | parity |
| Button | `UI/leptos/src/button.rs` | `UI/next/components/Button.tsx` | parity |
| Checkbox | `UI/leptos/src/checkbox.rs` | `UI/next/components/Checkbox.tsx` | parity |
| Input | `UI/leptos/src/input.rs` | `UI/next/components/Input.tsx` | parity |
| Select | `UI/leptos/src/select.rs` | `UI/next/components/Select.tsx` | parity |
| Spinner | `UI/leptos/src/spinner.rs` | `UI/next/components/Spinner.tsx` | parity |
| Switch | `UI/leptos/src/switch.rs` | `UI/next/components/Switch.tsx` | parity |
| Textarea | `UI/leptos/src/textarea.rs` | `UI/next/components/Textarea.tsx` | parity |
| Avatar | missing from shared Leptos surface | `UI/next/components/Avatar.tsx` | Next-only |
| Skeleton | missing from shared Leptos surface | `UI/next/components/Skeleton.tsx` | Next-only |

`UI/leptos/src/lib.rs` and `UI/next/components/index.ts` are the entry points for this shared primitive layer.

## Leptos-Specific Package Boundary: `crates/leptos-ui`

`crates/leptos-ui` holds the RusToK-specific Leptos surface for applications and module-owned UI packages. Current entry points:

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

This crate is needed where a simple shared primitive layer is insufficient and a stable package boundary within the Rust workspace is required.

## App-Local UI Not in the Shared Catalog

The following surfaces currently remain app-local and should not automatically be considered part of the shared catalog:

- `apps/next-admin/src/shared/ui/*`
- `apps/next-admin` data-table and related admin-only widgets
- `apps/admin` host-local layout/navigation components
- module-owned admin/storefront UI inside `crates/rustok-*/admin` and `crates/rustok-*/storefront`

If such a component starts being reused across multiple hosts or modules, it should either be promoted to `UI/*` or formalized through `crates/leptos-ui` for the Leptos path.

## Verification When Changing Shared UI

- compare `UI/leptos` and `UI/next/components` for API drift;
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
