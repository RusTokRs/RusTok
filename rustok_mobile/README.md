# RusTok Mobile Workspace

Flutter workspace scaffold based on `docs/research/flutter.md`.

## Structure

- `apps/rustok_admin_mobile` — host Flutter app shell.
- `packages/app_core` — shared core primitives.
- `packages/app_ui_kit` — design tokens and presentational widgets.
- `packages/app_graphql` — GraphQL transport wiring.
- `packages/app_route_contracts` — typed route/query contracts.
- `packages/app_module_contracts` — interfaces for module-owned mobile packages.

## Implemented now

- Host app routing with `go_router` + `ShellRoute`.
- Generated-manifest style module registry adapter (`mobile_manifest.g.dart`).
- Shared route contracts with snake_case query key constraints.
- Shared GraphQL transport context/header builders (tenant/locale non-blank validation in request context).
- GraphQL client factory with HTTP/WebSocket split transport and subscription support.
- Auth session scaffolding (`AuthSessionStore`, `AuthSessionManager`, in-memory store, refresh service contract).
- Manifest generator script from `crates/*/rustok-module.toml`.


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

## Next steps

1. Replace generic icon mapping with module metadata mapping rules.
2. Start first module package (`rustok_auth_mobile`) with real screens.
3. Replace in-memory auth session store with secure storage and connect refresh flow to sign-in lifecycle.
