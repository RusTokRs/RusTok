# leptos-ui

## Purpose

`leptos-ui` owns shared Leptos UI primitives and re-exports for RusToK applications and module-owned UI packages.

## Responsibilities

- Provide shared button, input, badge, alert, card, and other UI primitives.
- Re-export selected `iu_leptos` components behind a consistent RusToK package boundary.
- Keep common Leptos UI building blocks out of app-local duplication.

## Entry points

- `Button`
- `Input`
- `Badge`
- `Alert`
- `Card`
- `Label`
- `Separator`
- `LanguageToggle`

## Interactions

- Used by Leptos apps and module-owned admin/storefront packages across the workspace.
- Wraps and re-exports `iu_leptos` primitives while keeping RusToK-specific helpers local.
- Stays presentational and does not own transport or domain behavior.

## Docs

- [Platform docs index](../../docs/index.md)
