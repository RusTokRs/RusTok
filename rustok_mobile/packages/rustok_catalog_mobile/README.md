# RusTok Catalog Mobile

Module-owned Flutter package for customer-facing catalog and cart storefront
surfaces.

## Purpose

`rustok_catalog_mobile` provides the first storefront mobile package in the
Flutter track. The package owns catalog/cart presentation and a small repository
boundary, while the host remains responsible for tenant, locale, auth/session,
GraphQL transport, and route wiring.

## Responsibilities

- Render customer catalog and cart screens for the mobile storefront host.
- Consume a host-provided `StorefrontCatalogRepository` instead of creating a
  package-local GraphQL client or locale resolver.
- Keep loading, empty, and error states inside the package so host routing stays
  declarative.
- Avoid admin/operator affordances in customer-facing surfaces.

## Interactions

- `apps/rustok_frontend_mobile` mounts `StorefrontCatalogScreen` at `/catalog`
  and `StorefrontCartScreen` at `/cart`.
- The host overrides `storefrontCatalogRepositoryProvider` with a host-owned
  repository implementation.
- The current storefront host implementation reads catalog data through shared
  GraphQL transport and the existing storefront search surface; future cart
  transport must continue using host-owned wiring instead of package-local clients.

## Entry points

- `lib/rustok_catalog_mobile.dart` — public package exports.
- `lib/src/catalog_repository.dart` — repository boundary and Riverpod providers.
- `lib/src/catalog_screens.dart` — catalog and cart screens.
- `lib/src/product_summary.dart` — UI-facing product/cart DTOs.

## Documentation

- [Flutter plan](../../../docs/research/flutter.md)
- [Storefront contract](../../../docs/UI/storefront.md)
- [Platform docs index](../../../docs/index.md)
