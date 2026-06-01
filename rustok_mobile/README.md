# RusTok Mobile Workspace

Flutter workspace scaffold based on `docs/research/flutter.md`.

## Structure

- `apps/rustok_admin_mobile` — admin/operator host Flutter app shell.
- `apps/rustok_frontend_mobile` — customer storefront host Flutter app shell.
- `packages/app_core` — shared core primitives.
- `packages/app_ui_kit` — design tokens and presentational widgets.
- `packages/app_graphql` — GraphQL transport wiring.
- `packages/app_route_contracts` — typed route/query contracts.
- `packages/app_module_contracts` — interfaces for module-owned mobile packages.
- `packages/rustok_modules_mobile` — Phase 1 pilot package for the modules registry mobile surface.
- `packages/rustok_catalog_mobile` — first module-owned storefront mobile package for catalog/cart surfaces.

## Implemented now

- Host app routing with `go_router` + `ShellRoute`.
- Generated-manifest style module registry adapter (`mobile_manifest.g.dart`) with locale, permissions, and nested routes.
- Manifest-driven navigation icon mapping with metadata fallbacks for generic module icons.
- Shared route contracts with snake_case query key constraints.
- Shared GraphQL transport context/header builders (tenant/locale non-blank validation in request context).
- Shared Flutter app-core page-builder error mapping for the canonical `validation` / `sanitize` / `runtime` / `feature-disabled` catalog and `FEATURE_DISABLED` operator guidance.
- GraphQL client factory with HTTP/WebSocket split transport and subscription support.
- Auth session scaffolding (`AuthSessionStore`, `AuthSessionManager`, in-memory store, refresh service contract).
- Manifest generator script from `crates/*/rustok-module.toml`.
- Phase 1 pilot modules package with GraphQL-backed list/detail shell navigation evidence.
- First mutation-backed modules operator action via the canonical `toggleModule` GraphQL mutation, gated by GraphQL-hydrated `me.permissions` capability context, with post-hook recovery feedback and retry/compensation actions.
- Separate Flutter storefront mobile host scaffold with host-owned tenant/locale/GraphQL context.
- Module-owned storefront catalog/cart mobile package mounted by `rustok_frontend_mobile` without package-local transport clients; catalog reads use the existing `storefrontSearch` GraphQL surface and cart reads/writes use the canonical `storefrontCart`, `createStorefrontCart`, `addStorefrontCartLineItem`, `updateStorefrontCartLineItem`, and `removeStorefrontCartLineItem` GraphQL surfaces through the host repository boundary, with cart id state held by the host cart id store and optional durable file-backed persistence adapter.
- Generated storefront mobile manifest from `provides.storefront_ui` with a dedicated snapshot and freshness checks.
- Storefront registry adapter that maps generated `products` and `cart` routes to mounted module-owned package screens, with generic fallback for unmapped storefront modules and registry-driven home navigation for all generated storefront routes.

Host-level providers now resolve sessions via `AuthSessionManager` and `RefreshTokenService` before building the authenticated GraphQL client. The refresh flow uses an HTTP-only GraphQL client to avoid unnecessary WebSocket initialization during bootstrap. Provider wiring is isolated in `apps/rustok_admin_mobile/lib/app_shell/auth_bootstrap.dart`.

## Runtime transport configuration

Host app reads GraphQL transport defaults from `--dart-define` values:

- `RUSTOK_SERVER_BASE_URL` (default: `http://localhost:8080`)
- `RUSTOK_TENANT_SLUG` (default: `default`)
- `RUSTOK_LOCALE` (default: `en`)

Example:

```bash
flutter run \
  --dart-define=RUSTOK_SERVER_BASE_URL=https://api.example.com \
  --dart-define=RUSTOK_TENANT_SLUG=acme \
  --dart-define=RUSTOK_LOCALE=ru
```

For the storefront mobile host use the storefront-specific defines:

- `RUSTOK_STOREFRONT_SERVER_BASE_URL` (default: `http://localhost:8080`)
- `RUSTOK_STOREFRONT_TENANT_SLUG` (default: `default`)
- `RUSTOK_STOREFRONT_LOCALE` (default: `en`)
- `RUSTOK_STOREFRONT_CART_ID` (optional; when set, `/cart` reads the canonical `storefrontCart` GraphQL surface)
- `RUSTOK_STOREFRONT_CART_ID_FILE` (optional; enables host-owned durable cart id persistence for mobile/desktop storefront clients)
- `RUSTOK_STOREFRONT_CART_STORAGE_KEY` (optional; overrides the key used inside the host-owned cart id persistence adapter)

## Regenerate mobile manifest

```bash
python3 rustok_mobile/tooling/scripts/generate_mobile_manifest.py --repo-root /workspace/RusTok
```

## Verify manifest freshness

```bash
python3 rustok_mobile/tooling/scripts/verify_mobile_manifest.py --repo-root /workspace/RusTok
```

The verification command fails on stale generated files and prints a unified diff
from the committed manifest/snapshot to the expected deterministic output. The
snapshot includes normalized `nav_icon`, locale, permissions, and child-page
metadata so navigation parity drift is visible in codegen checks. The generator
uses canonical `child_pages` metadata and still accepts legacy `pages` entries
as a compatibility alias when `child_pages` is absent.

## Check deterministic codegen

```bash
python3 rustok_mobile/tooling/scripts/check_mobile_codegen.py --repo-root /workspace/RusTok
```

This command runs the generator into temporary files and compares those outputs
with the committed manifest and snapshot. Use it when you need a CI-friendly
signal that exercises the generator CLI itself.

## Regenerate storefront mobile manifest

```bash
python3 rustok_mobile/tooling/scripts/generate_mobile_manifest.py \
  --repo-root /workspace/RusTok \
  --surface storefront \
  --output rustok_mobile/apps/rustok_frontend_mobile/lib/registry/storefront_mobile_manifest.g.dart \
  --snapshot-output rustok_mobile/tooling/snapshots/storefront_mobile_manifest.snapshot.json
```

## Verify storefront manifest freshness

```bash
python3 rustok_mobile/tooling/scripts/check_mobile_codegen.py \
  --repo-root /workspace/RusTok \
  --surface storefront \
  --manifest rustok_mobile/apps/rustok_frontend_mobile/lib/registry/storefront_mobile_manifest.g.dart \
  --snapshot rustok_mobile/tooling/snapshots/storefront_mobile_manifest.snapshot.json
```

## Next steps

1. Replace in-memory auth session store with secure storage and connect refresh flow to sign-in lifecycle.
2. Promote storefront cart id persistence from the file-backed adapter to the agreed production storage backend once product requirements choose secure/non-sensitive storage boundaries.
3. Add deterministic generated-file checks to the mobile CI pipeline.
4. Extend generated storefront registry mappings as more module-owned storefront packages are added.
