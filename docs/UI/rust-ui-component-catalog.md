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
- Rust adapters must maintain functional parity with the corresponding components used by `apps/next-admin`, including their meaningful props, controlled/uncontrolled state, accessibility semantics, and user interactions. Matching purpose, visuals, or only the basic props is not sufficient.
- The Rust API may use framework-idiomatic types and composition rather than copying React/Radix signatures verbatim, but every user-facing capability in the Next wrapper must have an equivalent, documented Rust path or be recorded as an explicit parity gap.
- Shared UI packages remain a presentational layer and do not own transport, auth, routing, or domain behavior.

## Functional Reference: Leptos ↔ `apps/next-admin`

Components with an explicit shared surface. Historical `parity` labels are not
acceptance evidence: functional parity means the actual exposed props, state
transitions, accessibility behavior, and interactions have been compared and
covered. Components not yet given that feature audit remain provisional.

| Primitive | Leptos | Next.js | Status |
|-----------|--------|---------|--------|
| Alert | `crates/ui/rustok-ui/leptos/src/alert.rs` | `apps/next-admin/src/shared/ui/shadcn/alert.tsx` | functional audit pending |
| Badge | `crates/ui/rustok-ui/leptos/src/badge.rs` | `apps/next-admin/src/shared/ui/shadcn/badge.tsx` | functional audit pending |
| Button | `crates/ui/rustok-ui/leptos/src/button.rs` | `apps/next-admin/src/shared/ui/shadcn/button.tsx` | functional audit pending |
| Checkbox | `crates/ui/rustok-ui/leptos/src/checkbox.rs` | `apps/next-admin/src/shared/ui/shadcn/checkbox.tsx` | functional audit pending |
| Input | `crates/ui/rustok-ui/leptos/src/input.rs` | `apps/next-admin/src/shared/ui/shadcn/input.tsx` | functional audit pending |
| Select | `crates/ui/rustok-ui/leptos/src/select.rs` | `apps/next-admin/src/shared/ui/shadcn/select.tsx` | functional audit pending |
| Spinner | `crates/ui/rustok-ui/leptos/src/spinner.rs` | `apps/next-admin/src/shared/ui/icons.tsx` (spinner icon only) | no shared component counterpart identified |
| Progress | `crates/ui/rustok-ui/leptos/src/progress.rs` | `apps/next-admin/src/shared/ui/shadcn/progress.tsx` | range/orientation/ARIA behavior implemented; generic DOM-prop forwarding and validation pending |
| Switch | `crates/ui/rustok-ui/leptos/src/switch.rs` | `apps/next-admin/src/shared/ui/shadcn/switch.tsx` | functional audit pending |
| Textarea | `crates/ui/rustok-ui/leptos/src/textarea.rs` | `apps/next-admin/src/shared/ui/shadcn/textarea.tsx` | functional audit pending |
| Avatar | `crates/ui/rustok-ui/leptos/src/avatar.rs` | `apps/next-admin/src/shared/ui/shadcn/avatar.tsx` | functional audit pending |
| Skeleton | `crates/ui/rustok-ui/leptos/src/skeleton.rs` | `apps/next-admin/src/shared/ui/shadcn/skeleton.tsx` | functional audit pending |
| Dialog | `crates/ui/rustok-ui/leptos/src/dialog.rs` | `apps/next-admin/src/shared/ui/shadcn/dialog.tsx` (`@radix-ui/react-dialog`) | feature parity in progress; see current gaps below |
| Card | `crates/ui/rustok-ui/leptos/src/card.rs` | `apps/next-admin/src/shared/ui/shadcn/card.tsx` | functional audit pending |
| Tabs | `crates/ui/rustok-ui/leptos/src/tabs.rs` | `apps/next-admin/src/shared/ui/shadcn/tabs.tsx` | functional audit pending |
| Label | `crates/ui/rustok-ui/leptos/src/label.rs` | `apps/next-admin/src/shared/ui/shadcn/label.tsx` | functional audit pending |
| Separator | `crates/ui/rustok-ui/leptos/src/separator.rs` | `apps/next-admin/src/shared/ui/shadcn/separator.tsx` | functional audit pending |

`crates/ui/rustok-ui/leptos/src/lib.rs` and
`apps/next-admin/src/shared/ui/shadcn/index.ts` are the component entry points being
compared. `UI/next/components/*` is a separate legacy Next component set;
`UI/leptos/src/*` remains available through the `iu-leptos` workspace crate.

### Dialog feature-parity checklist

The Next reference is a shadcn wrapper around Radix Dialog, not a single styled
`<div>`. The Leptos adapter now exposes `Dialog`, `DialogTrigger`, `DialogPortal`,
`DialogClose`, `DialogOverlay`, `DialogContent`, `DialogHeader`, `DialogFooter`,
`DialogTitle`, and `DialogDescription`; it supports controlled/uncontrolled open
state, modal/non-modal content, a body portal, Escape/backdrop/close-button
closure, focus on open, modal Tab trapping, and trigger focus restoration.
Accessible name/description references are explicit through `aria_label`,
`aria_labelledby`, `aria_describedby`, and IDs on title/description. Set
`DialogTrigger aria_controls` manually to match `DialogContent id`.

This is **not yet declared full parity**: Radix's `asChild` slot behavior,
background isolation/inertness, full outside-interaction semantics
(especially non-modal dialogs), Radix-managed IDs, and mount/exit
animation lifecycle still need equivalent implementations and browser-level
verification. Keep the status as in progress until those gaps are implemented or
explicitly resolved and tested against the Next wrapper. The Dioxus `Dialog`
adapter remains a single host-managed wrapper and is also a known parity gap; the
Leptos adapter is the web path being aligned with `apps/next-admin` here.

Progress work currently covers the wrapper's value/max range, orientation, ARIA
naming and text, state/data attributes, and normalized visual percentage;
generic DOM-prop forwarding and runtime validation are still pending. Keep
shared component status evidence-based: do not mark a component `parity` solely
because its base markup or class names match.

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
