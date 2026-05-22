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
- Shared GraphQL transport context/header builders.
- Manifest generator script from `crates/*/rustok-module.toml`.

## Regenerate mobile manifest

```bash
python3 rustok_mobile/tooling/scripts/generate_mobile_manifest.py --repo-root /workspace/RusTok
```

## Verify manifest freshness

```bash
python3 rustok_mobile/tooling/scripts/verify_mobile_manifest.py --repo-root /workspace/RusTok
```

## Next steps

1. Add real GraphQL client factory (HTTP + WebSocket split links).
2. Replace generic icon mapping with module metadata mapping rules.
3. Start first module package (`rustok_auth_mobile`) with real screens.
