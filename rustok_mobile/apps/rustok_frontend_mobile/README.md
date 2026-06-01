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
- Mount module-owned catalog/cart mobile surfaces at `/catalog` and `/cart`, including create/add/update/remove cart actions through the host-owned repository boundary.
- Resolve manifest-driven module routes under `/modules/:routeSegment`, using the generated storefront mobile registry to mount known package surfaces, show registry-driven home links, and fallback placeholders for the rest.

## Interactions

- Uses `apps/server` through the shared GraphQL client package.
- Uses `packages/rustok_catalog_mobile` for customer catalog/cart screens through a host-provided repository boundary.
- Keeps route semantics aligned with the storefront contract in `docs/UI/storefront.md`.
- Shares the neutral mobile GraphQL foundation package (`app_graphql`) with other mobile hosts; route and UI-kit packages will be added when module-owned storefront surfaces need them.

## Entry points

- `lib/main.dart` — app bootstrap and provider wiring.
- `lib/app_shell/storefront_shell_page.dart` — mobile storefront shell.
- `lib/app_shell/storefront_context.dart` — host-owned runtime context, durable cart id store adapter, and GraphQL client configuration.
- `lib/data/storefront_catalog_repository.dart` — host-owned catalog/cart repository using the shared GraphQL client and existing `storefrontSearch`, `storefrontCart`, and storefront cart mutation surfaces.
- `lib/registry/storefront_mobile_manifest.g.dart` — generated storefront registry from `provides.storefront_ui`.
- `lib/registry/storefront_surface_registry.dart` — host adapter that maps generated storefront entries to mounted mobile package surfaces.
- `lib/routes/storefront_router.dart` — route table, registry-driven module links, module-owned catalog/cart mounting, and generic manifest-backed module placeholders.

## Runtime defines

- `RUSTOK_STOREFRONT_SERVER_BASE_URL` — server base URL.
- `RUSTOK_STOREFRONT_TENANT_SLUG` — tenant slug for shared GraphQL headers.
- `RUSTOK_STOREFRONT_LOCALE` — host-selected effective locale.
- `RUSTOK_STOREFRONT_CART_ID` — optional initial cart id loaded into the host-owned cart id store for the canonical `storefrontCart` read path and cart line mutation context.
- `RUSTOK_STOREFRONT_CART_ID_FILE` — optional host-owned JSON file used by the durable cart id adapter on mobile/desktop builds; when unset, tests and previews keep using the in-memory adapter.
- `RUSTOK_STOREFRONT_CART_STORAGE_KEY` — optional key for the host-owned cart id persistence adapter; defaults to `rustok.storefront.cart_id`.

## Documentation

- [Flutter plan](../../../docs/research/flutter.md)
- [Storefront contract](../../../docs/UI/storefront.md)
- [Platform docs index](../../../docs/index.md)
