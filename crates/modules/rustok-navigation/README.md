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
menu items currently store public URLs rather than owner-specific page identifiers.
The accepted [page-link decision](../../../DECISIONS/2026-10-09-page-layouts-and-menu-page-links.md)
adds identity-based page targets and host-composed Pages route resolution without
making Navigation depend on Pages. This target is not yet implemented.

## Known Limitations

Current menu links are static URLs; a published Page slug change does not update
a Navigation menu. Until the route reader, writes and all public transports are
wired together, do not store an empty URL as a page reference.

Translation consumes Navigation only through `rustok-translation-targets` and
the owner provider. It never reads or writes Navigation tables directly. Locale
fallback never contributes to translation snapshots or progress; `menus`
resource revisions and `menu_translations` aggregate revisions provide the
resource/source/target CAS guards. `navigation_translation_changes` is a
content-free cursor-repair journal. Navigation does not claim a generic menu
outbox event; provider apply commits the locale aggregate, the journal entry,
and its durable owner receipt in one transaction.

See [module documentation](docs/README.md).
