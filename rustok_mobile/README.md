# RusTok Mobile Workspace

Flutter workspace scaffold based on `docs/research/flutter.md` and ADR `2026-11-12-decommission-mobile-admin-host.md`.

## Architectural Scope

The RusTok mobile workspace is dedicated exclusively to the customer-facing **SuperApp / Storefront** host (`apps/rustok_frontend_mobile`) and modular client-side packages.

Administrative back-office management is maintained via canonical web hosts (`apps/next-admin`, `apps/admin`) and headless/agentic interfaces (`rustok-ai`, MCP tools, GraphQL). The experimental mobile admin host (`rustok_admin_mobile`) and its pilot registry package (`rustok_modules_mobile`) were decommissioned under ADR `2026-11-12-decommission-mobile-admin-host.md`.

## Structure

- `apps/rustok_frontend_mobile` — customer storefront and SuperApp host Flutter app shell.
- `packages/app_core` — shared core primitives (error mapping, results, environment).
- `packages/app_ui_kit` — design tokens and presentational widgets.
- `packages/app_graphql` — GraphQL transport wiring and authenticated client factory.
- `packages/app_route_contracts` — typed route and query contracts.
- `packages/app_module_contracts` — interfaces and data structures for module-owned mobile packages.
- `packages/rustok_catalog_mobile` — module-owned storefront mobile package for catalog and cart surfaces.

## Implemented now

- Host app routing with `go_router` + `ShellRoute`.
- Separate Flutter storefront mobile host scaffold with host-owned tenant/locale/GraphQL context.
- Generated storefront mobile manifest from `provides.storefront_ui` with a dedicated snapshot and freshness checks (`storefront_mobile_manifest.g.dart`).
- Storefront registry adapter that maps generated `products` and `cart` routes to mounted module-owned package screens, with generic fallback for unmapped storefront modules and registry-driven home navigation for all generated storefront routes.
- Module-owned storefront catalog/cart mobile package mounted by `rustok_frontend_mobile` without package-local transport clients; catalog reads use the existing `storefrontSearch` GraphQL surface and cart reads/writes use the canonical `storefrontCart`, `createStorefrontCart`, `addStorefrontCartLineItem`, `updateStorefrontCartLineItem`, and `removeStorefrontCartLineItem` GraphQL surfaces through the host repository boundary.
- Shared route contracts with snake_case query key constraints.
- Shared GraphQL transport context/header builders (tenant/locale non-blank validation in request context).
- GraphQL client factory with HTTP/WebSocket split transport and subscription support.
- Storefront catalog/cart GraphQL verifier ties Flutter operation documents to existing server storefront/search APIs and the commerce runtime parity test flow, so catalog/cart drift is caught even when the Flutter SDK is unavailable in CI.
- Storefront checkout intent policy lives in the host-owned runtime context view-model; catalog/cart packages emit checkout intents without owning checkout copy, cart-id fallback policy, or backend endpoints.

## Runtime transport configuration

Storefront mobile host reads GraphQL transport defaults from `--dart-define` values:

- `RUSTOK_STOREFRONT_SERVER_BASE_URL` (default: `http://localhost:8080`)
- `RUSTOK_STOREFRONT_TENANT_SLUG` (default: `default`)
- `RUSTOK_STOREFRONT_LOCALE` (default: `en`)
- `RUSTOK_STOREFRONT_CART_ID` (optional; when set, `/cart` reads the canonical `storefrontCart` GraphQL surface)
- `RUSTOK_STOREFRONT_CART_ID_FILE` (optional; enables host-owned durable cart id persistence for mobile/desktop storefront clients)
- `RUSTOK_STOREFRONT_CART_STORAGE_KEY` (optional; overrides the key used inside the host-owned cart id persistence adapter)

Example:

```bash
flutter run \
  --dart-define=RUSTOK_STOREFRONT_SERVER_BASE_URL=https://api.example.com \
  --dart-define=RUSTOK_STOREFRONT_TENANT_SLUG=acme \
  --dart-define=RUSTOK_STOREFRONT_LOCALE=ru
```

## Regenerate mobile manifest

```bash
python3 rustok_mobile/tooling/scripts/generate_mobile_manifest.py --repo-root .
```

## Verify manifest freshness

```bash
python3 rustok_mobile/tooling/scripts/verify_mobile_manifest.py --repo-root .
```

The verification command fails on stale generated files and prints a unified diff from the committed manifest/snapshot to the expected deterministic output.

## Check deterministic codegen

```bash
python3 rustok_mobile/tooling/scripts/check_mobile_codegen.py --repo-root .
```

This command runs the generator into temporary files and compares those outputs with the committed manifest and snapshot.

## Verify storefront catalog/cart GraphQL contracts

```bash
python3 rustok_mobile/tooling/scripts/verify_storefront_graphql_contract.py --repo-root .
```

## Next steps

1. Replace in-memory auth session store with secure storage and connect refresh flow to sign-in lifecycle.
2. Promote storefront cart id persistence from the file-backed adapter to the agreed production storage backend once product requirements choose secure/non-sensitive storage boundaries.
3. Extend generated storefront registry mappings as more module-owned storefront packages are added (e.g., Blog, Forum, Profiles).
4. Integrate Server-Driven UI (Page Builder) and lightweight mini-app webviews for long-tail modular extensions.
