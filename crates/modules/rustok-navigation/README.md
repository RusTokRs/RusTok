# rustok-navigation

## Purpose

Own localized navigation menus and deterministic tenant/channel/slot bindings.

## Responsibilities

- Menu and nested item persistence.
- Exact-locale public reads.
- Current-channel bindings for header, footer, sidebar and mobile locations.
- Exact `navigation/menu` Translation target aggregates: a menu name and every
  nested item title apply as one locale transaction.
- Navigation-owned GraphQL, HTTP and storefront UI surfaces.

## Entry points

- `NavigationModule`
- `NavigationQuery` / `NavigationMutation`
- `http::axum_router`
- `MenuService` / `MenuBindingService`
- `NavigationMenuTranslationTargetProvider`

## Interactions

Navigation depends on Channel for current-channel scope and on Outbox only for
the shared durable owner-operation receipt ledger. It does not depend on Pages;
menu items store public URLs rather than owner-specific page identifiers.

Translation consumes Navigation only through `rustok-translation-targets` and
the owner provider. It never reads or writes Navigation tables directly. Locale
fallback never contributes to translation snapshots or progress; `menus`
resource revisions and `menu_translations` aggregate revisions provide the
resource/source/target CAS guards. `navigation_translation_changes` is a
content-free cursor-repair journal. Navigation does not claim a generic menu
outbox event; provider apply commits the locale aggregate, the journal entry,
and its durable owner receipt in one transaction.

See [module documentation](docs/README.md).
