# leptos-ui

RusToK's stable Leptos package boundary for shared UI components and Leptos-only composites.

## Overview

`leptos-ui` re-exports the complete component and type surface from
`rustok-ui-leptos`. Component implementations, design tokens, and deterministic
class resolution are owned by the framework-agnostic `rustok-ui` core and its
Leptos adapter; this crate does not maintain a parallel primitive
implementation.

The shared component surface includes `Button`, `Input`, `Textarea`, `Select`,
`Checkbox`, `Switch`, `Progress`, `Alert`, `Badge`, `Spinner`, `Skeleton`,
`Avatar`, `Card*`, `Dialog*`, `Tabs*`, `Label`, and `Separator`. Components
remain presentational and leave transport, auth, routing, and domain behavior
to their owners.

## Leptos-specific composites

This crate also provides:

- `LanguageToggle` for a caller-supplied list of available locales;
- `RichTextHtml` for rendering a typed, server-derived `RichTextView` without
  accepting arbitrary HTML strings;
- `RichTextEditorFrame` for the isolated shared rich-text authoring runtime;
- `SuccessMessage` through the `ui_success_message` re-export.

## Interactions

- Used by Leptos apps and module-owned admin/storefront packages.
- Re-exports `rustok-ui-leptos` and keeps RusToK-specific composites local.
- Does not own transport, domain behavior, or locale negotiation.

## Docs

- [Rust UI component catalog](../../../docs/UI/rust-ui-component-catalog.md)
- [Platform docs index](../../../docs/index.md)
