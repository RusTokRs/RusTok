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
responsibility between `crates/ui/rustok-ui*`, `UI/*`, and
app-local components.

## Sources of Shared UI

The repository currently has these levels of UI reuse:

- `crates/ui/rustok-ui/src` — framework-agnostic contracts (variants, sizes, state
  models) and the deterministic Tailwind class resolvers shared by every framework;
- `crates/ui/rustok-ui/leptos` (`rustok-ui-leptos`) and `crates/ui/rustok-ui/dioxus`
  (`rustok-ui-dioxus`) — the framework component implementations providing
  primitives (`Button`, `Input`, `Badge`, `Card`, etc.) and shared components
  (`LanguageToggle`, `RichTextHtml`, `RichTextEditorFrame`, `TableOfContents`);
- `UI/tokens` — legacy design tokens and CSS variables consumed by the admin shell;
- `UI/next/components` — the legacy parallel shared primitives kept for Next surfaces that have not migrated to `rustok-ui` yet.

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
| Alert | `crates/ui/rustok-ui/leptos/src/alert.rs` | `apps/next-admin/src/shared/ui/shadcn/alert.tsx` | full parity across Leptos and Next.js (`data-slot`, title/description sub-slots, accessible alert role) |
| Badge | `crates/ui/rustok-ui/leptos/src/badge.rs` | `apps/next-admin/src/shared/ui/shadcn/badge.tsx` | full parity across Leptos and Next.js (`data-slot="badge"`, dismissible callback, variant parity) |
| Button | `crates/ui/rustok-ui/leptos/src/button.rs` | `apps/next-admin/src/shared/ui/shadcn/button.tsx` | full parity across Leptos and Next.js (`data-slot="button"`, loading state, spinner, aria-busy) |
| Checkbox | `crates/ui/rustok-ui/leptos/src/checkbox.rs` | `apps/next-admin/src/shared/ui/shadcn/checkbox.tsx` | full parity across Leptos, Dioxus, and Next.js (`data-slot="checkbox"`, `data-state`, `aria-checked="mixed"`, indeterminate, reactive validation) |
| Input | `crates/ui/rustok-ui/leptos/src/input.rs` | `apps/next-admin/src/shared/ui/shadcn/input.tsx` | full parity across Leptos, Dioxus, and Next.js (`data-slot="input"`, prefixes/suffixes, reactive validation, aria-invalid) |
| Select | `crates/ui/rustok-ui/leptos/src/select.rs` | `apps/next-admin/src/shared/ui/shadcn/select.tsx` | full parity across Leptos, Dioxus, and Next.js (`data-slot="select"`, options rows, reactive value, placeholder, aria-invalid) |
| Spinner | `crates/ui/rustok-ui/leptos/src/spinner.rs` | `apps/next-admin/src/shared/ui/icons.tsx` (spinner icon only) | full parity across Leptos, Dioxus, and Next.js (`data-slot="spinner"`, role="status", localized accessible name) |
| Progress | `crates/ui/rustok-ui/leptos/src/progress.rs` | `apps/next-admin/src/shared/ui/shadcn/progress.tsx` | full parity across Leptos, Dioxus, and Next.js (range, orientation, ARIA attributes, SVG indicator percentage) |
| Switch | `crates/ui/rustok-ui/leptos/src/switch.rs` | `apps/next-admin/src/shared/ui/shadcn/switch.tsx` | full parity across Leptos, Dioxus, and Next.js (controlled/uncontrolled, WAI-ARIA, Radix `data-state`/`data-slot`, form submission) |
| Textarea | `crates/ui/rustok-ui/leptos/src/textarea.rs` | `apps/next-admin/src/shared/ui/shadcn/textarea.tsx` | full parity across Leptos, Dioxus, and Next.js (`data-slot="textarea"`, reactive validation, SignalSetter, described_by) |
| Avatar | `crates/ui/rustok-ui/leptos/src/avatar.rs` | `apps/next-admin/src/shared/ui/shadcn/avatar.tsx` | full parity across Leptos, Dioxus, and Next.js (`data-slot`, automatic initials, client image error fallback) |
| Skeleton | `crates/ui/rustok-ui/leptos/src/skeleton.rs` | `apps/next-admin/src/shared/ui/shadcn/skeleton.tsx` | full parity across Leptos, Dioxus, and Next.js (`data-slot="skeleton"`, pulse animation, aria-hidden) |
| Dialog | `crates/ui/rustok-ui/leptos/src/dialog.rs` | `apps/next-admin/src/shared/ui/shadcn/dialog.tsx` (`@radix-ui/react-dialog`) | full parity across Leptos, Dioxus, and Next.js (automatic unique ID pairing, focus trap, Escape/overlay dismissal, data-slot, data-state) |
| Card | `crates/ui/rustok-ui/leptos/src/card.rs` | `apps/next-admin/src/shared/ui/shadcn/card.tsx` | full parity across Leptos, Dioxus, and Next.js (`data-slot` on root and all sub-components, CSS container grid alignment) |
| Tabs | `crates/ui/rustok-ui/leptos/src/tabs.rs` | `apps/next-admin/src/shared/ui/shadcn/tabs.tsx` | full parity across Leptos, Dioxus, and Next.js (TabsContext, roving tabindex, W3C arrow navigation, controlled/uncontrolled) |
| Label | `crates/ui/rustok-ui/leptos/src/label.rs` | `apps/next-admin/src/shared/ui/shadcn/label.tsx` | full parity across Leptos, Dioxus, and Next.js (`data-slot="label"`, dynamic/static for targeting, required asterisk) |
| Separator | `crates/ui/rustok-ui/leptos/src/separator.rs` | `apps/next-admin/src/shared/ui/shadcn/separator.tsx` | full parity across Leptos, Dioxus, and Next.js (`data-slot="separator"`, orientation, decorative semantics) |
| TableOfContents | `crates/ui/rustok-ui/leptos/src/toc.rs` (`rustok-ui-leptos`), `crates/ui/rustok-ui/dioxus/src/toc.rs` | `apps/next-frontend/src/shared/ui/table-of-contents.tsx` | full parity across Leptos, Dioxus, and Next.js for Blog, Forum, and Product Descriptions |

`crates/ui/rustok-ui/leptos/src/lib.rs` and
`apps/next-admin/src/shared/ui/shadcn/index.ts` are the component entry points being
compared. `UI/next/components/*` is a separate legacy Next component set.

### Dialog feature-parity checklist

The Next reference is a shadcn wrapper around Radix Dialog, not a single styled
`<div>`. The Leptos adapter exposes `Dialog`, `DialogTrigger`, `DialogPortal`,
`DialogClose`, `DialogOverlay`, `DialogContent`, `DialogHeader`, `DialogFooter`,
`DialogTitle`, and `DialogDescription`; it supports controlled/uncontrolled open
state, modal/non-modal content, a body portal, Escape/backdrop/close-button
closure, focus on open, modal Tab trapping, and trigger focus restoration.
Accessible name/description references support automatic unique ID pairing via
`DialogContext` and atomic counters (linking `DialogTrigger aria-controls` to
`DialogContent id`, and `DialogTitle id` to `DialogContent aria-labelledby`), while
still supporting explicit caller-provided IDs. CSS styling is driven by `data-slot`
attributes (`dialog-trigger`, `dialog-overlay`, `dialog-content`, `dialog-header`,
`dialog-title`, `dialog-description`, `dialog-footer`, `dialog-close`) and
`data-state="open" | "closed"`. Dioxus exposes matching `data-slot` and `data-state`
attributes.

### Tabs & Switch feature-parity specification

- **Tabs (`crates/ui/rustok-ui/leptos/src/tabs.rs`)**:
  - Context & State: declarative subtree coordination via `TabsContext` (`value: Signal<String>`, `set_value`, `orientation`).
  - Modes: supports controlled mode (`value` + `on_value_change`) and uncontrolled mode (`default_value`). Backwards-compatible with legacy `active: bool` direct props.
  - Accessibility: WAI-ARIA `role="tablist"`, `aria-orientation`, `role="tab"`, `aria-selected`, `aria-controls`, `role="tabpanel"`, `aria-labelledby`.
  - Keyboard Navigation: roving `tabindex` (`"0"` on active tab, `"-1"` on inactive tabs) and client-side arrow key navigation on `wasm32` (`ArrowRight`/`ArrowLeft`/`ArrowDown`/`ArrowUp`/`Home`/`End`) with focus wrapping and selection trigger.

- **Switch (`crates/ui/rustok-ui/leptos/src/switch.rs`)**:
  - State: supports controlled mode (`checked` + `set_checked` / `on_checked_change`) and uncontrolled mode (`default_checked`).
  - Accessibility: WAI-ARIA `role="switch"`, `aria-checked`, `aria-label`, `aria-labelledby`, `data-disabled`.
  - CSS & Radix Parity: `data-state="checked" | "unchecked"` and `data-slot="switch"` / `data-slot="switch-thumb"` matching Radix/Shadcn Tailwind selectors.
  - Form Integration: renders a hidden accessible `<input type="checkbox">` when `name` is provided to allow native HTML form submission and `FormData` extraction.

Progress work currently covers the wrapper's value/max range, orientation, ARIA
naming and text, state/data attributes, and normalized visual percentage;
generic DOM-prop forwarding and runtime validation are still pending. Keep
shared component status evidence-based: do not mark a component `parity` solely
because its base markup or class names match.

## Leptos Package Boundary: `crates/ui/rustok-ui/leptos` (`rustok-ui-leptos`)

`crates/ui/rustok-ui/leptos` holds the canonical Leptos component library for applications and module-owned UI packages. Current entry points:

- `Button`
- `Input`
- `Badge`
- `Alert`
- `Card`, `CardHeader`, `CardTitle`, `CardDescription`, `CardAction`, `CardContent`, `CardFooter`
- `Checkbox`
- `Dialog`, `DialogTrigger`, `DialogContent`, `DialogHeader`, `DialogFooter`, `DialogTitle`, `DialogDescription`, `DialogClose`, `DialogOverlay`, `DialogPortal`
- `Label`
- `Progress`
- `Select`
- `Separator`
- `Skeleton`
- `Spinner`
- `Switch`
- `Tabs`, `TabsList`, `TabsTrigger`, `TabsContent`, `TabsContext`, `use_tabs_context`
- `Textarea`
- `LanguageToggle`
- `RichTextHtml` for editor-free rendering of a typed, server-derived
  `RichTextView`
- `RichTextEditorFrame` for the isolated shared authoring runtime
- `TableOfContents` (and `extract_headings_from_html`) for sticky, accessible heading navigation across blog posts, forum topics, long product descriptions, and documentation

## App-Local UI Not in the Shared Catalog

The following surfaces currently remain app-local and should not automatically be considered part of the shared catalog:

- `apps/next-admin/src/shared/ui/*`
- `apps/next-admin` data-table and related admin-only widgets
- `apps/admin` host-local layout/navigation components
- module-owned admin/storefront UI inside `crates/modules/rustok-*/admin` and `crates/modules/rustok-*/storefront`

If such a component starts being reused across multiple hosts or modules, it should either be promoted to `UI/*` or formalized through `crates/ui/rustok-ui/leptos` for the Leptos path.

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

## Product buyer native controls (2026-10-30)

`packages/rustok-ui-controls` now owns the portable React Input, NativeSelect and Label used by
Product buyer UI. Admin's existing Input path re-exports that control. NativeSelect is explicitly
an HTML select counterpart of the Leptos native select, **not** a claim of full Admin Radix-popup
parity. Leptos Input exposes numeric input-mode and validation/description ARIA bindings through
shared UI class contracts. Styles, host transpilation and manifests include the shared package.
Mounted keyboard/focus/controlled-state/screen-reader verification remains pending; type/source
checks do not promote the provisional component parity statuses above.
