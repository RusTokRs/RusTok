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

## Implemented now

- Host app routing with `go_router` + `ShellRoute`.
- Generated-manifest style module registry adapter (`mobile_manifest.g.dart`).
- Manifest-driven navigation icon mapping with metadata fallbacks for generic module icons.
- Shared route contracts with snake_case query key constraints.
- Shared GraphQL transport context/header builders (tenant/locale non-blank validation in request context).
- GraphQL client factory with HTTP/WebSocket split transport and subscription support.
- Auth session scaffolding (`AuthSessionStore`, `AuthSessionManager`, in-memory store, refresh service contract).
- Manifest generator script from `crates/*/rustok-module.toml`.
- Phase 1 pilot modules package with GraphQL-backed list/detail shell navigation evidence.
- Separate Flutter storefront mobile host scaffold with host-owned tenant/locale/GraphQL context.


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
snapshot includes normalized `nav_icon` metadata so navigation parity drift is
visible in codegen checks.

## Check deterministic codegen

```bash
python3 rustok_mobile/tooling/scripts/check_mobile_codegen.py --repo-root /workspace/RusTok
```

This command runs the generator into temporary files and compares those outputs
with the committed manifest and snapshot. Use it when you need a CI-friendly
signal that exercises the generator CLI itself.

## Next steps

1. Replace in-memory auth session store with secure storage and connect refresh flow to sign-in lifecycle.
2. Expand the admin modules pilot from list/detail shell navigation to the first mutation-backed operator action.
3. Add module-owned storefront mobile packages for catalog/cart surfaces.
4. Add deterministic generated-file checks to the mobile CI pipeline.
