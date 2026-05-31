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
- Mount module-owned catalog/cart mobile surfaces at `/catalog` and `/cart`.
- Resolve manifest-driven module routes under `/modules/:routeSegment`, using the generated storefront mobile registry to mount known package surfaces and fallback placeholders for the rest.

## Interactions

- Uses `apps/server` through the shared GraphQL client package.
- Uses `packages/rustok_catalog_mobile` for customer catalog/cart screens through a host-provided repository boundary.
- Keeps route semantics aligned with the storefront contract in `docs/UI/storefront.md`.
- Shares the neutral mobile GraphQL foundation package (`app_graphql`) with other mobile hosts; route and UI-kit packages will be added when module-owned storefront surfaces need them.

## Entry points

- `lib/main.dart` — app bootstrap and provider wiring.
- `lib/app_shell/storefront_shell_page.dart` — mobile storefront shell.
- `lib/app_shell/storefront_context.dart` — host-owned runtime context and GraphQL client configuration.
- `lib/data/storefront_catalog_repository.dart` — host-owned catalog repository using the shared GraphQL client and existing storefront search surface.
- `lib/registry/storefront_mobile_manifest.g.dart` — generated storefront registry from `provides.storefront_ui`.
- `lib/registry/storefront_surface_registry.dart` — host adapter that maps generated storefront entries to mounted mobile package surfaces.
- `lib/routes/storefront_router.dart` — route table, module-owned catalog/cart mounting, and generic manifest-backed module placeholders.

## Documentation

- [Flutter plan](../../../docs/research/flutter.md)
- [Storefront contract](../../../docs/UI/storefront.md)
- [Platform docs index](../../../docs/index.md)
