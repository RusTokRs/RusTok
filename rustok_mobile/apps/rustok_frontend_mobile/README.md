# RusTok Frontend Mobile

Flutter storefront mobile host for customer-facing RusTok experiences.

## Purpose

`apps/rustok_frontend_mobile` owns the mobile storefront shell. It is separate
from `rustok_admin_mobile` so admin/operator UX and customer-facing storefront
UX do not drift into one host.

## Responsibilities

- Provide a mobile-first customer storefront shell.
- Keep tenant, locale, and GraphQL transport context host-owned.
- Mirror the existing web storefront contract from `apps/storefront` and
  `apps/next-frontend` without introducing a Flutter-only backend API.
- Reserve manifest-driven module routes under `/modules/:routeSegment` for
  future module-owned storefront mobile packages.

## Interactions

- Uses `apps/server` through the shared GraphQL client package.
- Keeps route semantics aligned with the storefront contract in `docs/UI/storefront.md`.
- Shares the neutral mobile GraphQL foundation package (`app_graphql`) with other mobile hosts; route and UI-kit packages will be added when module-owned storefront surfaces need them.

## Entry points

- `lib/main.dart` — app bootstrap and provider wiring.
- `lib/app_shell/storefront_shell_page.dart` — mobile storefront shell.
- `lib/app_shell/storefront_context.dart` — host-owned runtime context and
  GraphQL client configuration.
- `lib/routes/storefront_router.dart` — route table and placeholder surfaces.

## Documentation

- [Flutter plan](../../../docs/research/flutter.md)
- [Storefront contract](../../../docs/UI/storefront.md)
- [Platform docs index](../../../docs/index.md)
