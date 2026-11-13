# RusTok Mobile Workspace

Flutter workspace scaffold based on `docs/research/flutter.md` and ADR `2026-11-12-mobile-admin-business-operator-boundary.md`.

## Architectural Scope

The RusTok mobile workspace maintains a clean separation of concerns across two dedicated host applications:

1. **`apps/rustok_frontend_mobile`**: Customer-facing **SuperApp / Storefront** host (catalog, cart, checkout, customer feeds, reviews).
2. **`apps/rustok_admin_mobile`**: Business-facing **Operator / Admin** host (CRM leads, order fulfillment, merchant catalog, courier & warehouse workflows).

### Boundary Invariant: No System SuperAdmin Settings on Mobile
Per ADR `2026-11-12-mobile-admin-business-operator-boundary.md`, `rustok_admin_mobile` is strictly focused on **business operations and CRM workflows** (the Odoo model). Low-level system DevOps settings (module toggling, server migrations, engine lifecycle failure recovery, infrastructure plumbing) are **strictly excluded** from mobile and reserved for desktop web consoles (`apps/next-admin`, `apps/admin`) and AI agent control (`rustok-ai` / MCP).

## Structure

- `apps/rustok_admin_mobile` — business admin / operator host Flutter app shell.
- `apps/rustok_frontend_mobile` — customer storefront and SuperApp host Flutter app shell.
- `packages/app_core` — shared core primitives (error mapping, results, environment).
- `packages/app_ui_kit` — design tokens and presentational widgets.
- `packages/app_graphql` — GraphQL transport wiring and authenticated client factory.
- `packages/app_route_contracts` — typed route and query contracts.
- `packages/app_module_contracts` — interfaces and data structures for module-owned mobile packages.
- `packages/rustok_catalog_mobile` — module-owned storefront mobile package for catalog and cart surfaces.
- `packages/rustok_modules_mobile` — pilot module navigation package.

## Implemented now

- Host app routing with `go_router` + `ShellRoute` for both hosts.
- Dual-surface generated manifests from `rustok-module.toml`:
  - `storefront_mobile_manifest.g.dart` generated from `provides.storefront_ui`.
  - `mobile_manifest.g.dart` generated from `provides.admin_ui`.
- Storefront catalog and cart mobile package mounted by `rustok_frontend_mobile` via canonical GraphQL operations.
- Shared route contracts with snake_case query key constraints.
- Shared GraphQL transport context/header builders (tenant/locale non-blank validation in request context).
- GraphQL client factory with HTTP/WebSocket split transport and subscription support.
- Storefront catalog/cart GraphQL verifier ties Flutter operation documents to existing server storefront/search APIs and the commerce runtime parity test flow.

## Regenerate mobile manifests

For customer storefront:
```bash
python3 rustok_mobile/tooling/scripts/generate_mobile_manifest.py --repo-root . --surface storefront
```

For business admin / operator:
```bash
python3 rustok_mobile/tooling/scripts/generate_mobile_manifest.py --repo-root . --surface admin --output rustok_mobile/apps/rustok_admin_mobile/lib/registry/mobile_manifest.g.dart --snapshot-output rustok_mobile/tooling/snapshots/mobile_manifest.snapshot.json
```

## Verify manifest freshness

```bash
python3 rustok_mobile/tooling/scripts/check_mobile_codegen.py --repo-root . --surface storefront
python3 rustok_mobile/tooling/scripts/check_mobile_codegen.py --repo-root . --surface admin --manifest rustok_mobile/apps/rustok_admin_mobile/lib/registry/mobile_manifest.g.dart --snapshot rustok_mobile/tooling/snapshots/mobile_manifest.snapshot.json
```

## Verify storefront catalog/cart GraphQL contracts

```bash
python3 rustok_mobile/tooling/scripts/verify_storefront_graphql_contract.py --repo-root .
```
