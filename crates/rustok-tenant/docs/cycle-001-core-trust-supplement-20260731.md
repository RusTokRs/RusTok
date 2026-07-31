# Cycle-001 Tenant/Core trust sweep supplement

This supplement records verified cycle-001 evidence discovered after the current
`crates/rustok-tenant/docs/implementation-plan.md` handoff was written. The main
Tenant handoff remains `in_progress`; this file does not mark the component
complete or replace its required same-SHA gates.

## Scope inspected

- Tenant owner CRUD, provisioning concurrency, locale-policy CAS/idempotency,
  locale migration/backfill, lifecycle outbox publication and cache generation.
- Authenticated/resolved tenant equality across Core and directly related native
  admin surfaces.
- Commerce store-context owner error mapping and focused all-features compile.
- Migration Compatibility blockers before the retained Tenant PostgreSQL
  backfill and incremental-upgrade fixtures can execute.
- Host-global Events/System operational authority versus tenant-scoped RBAC.

## Tenant-owner findings

The Tenant-owner count remains the existing `P0=3, P1=11, P2=1, P3=0`.
Additional findings below belong to interacting owners and are recorded in their
own plans or issue trackers rather than being silently added to the Tenant-owner
count.

## Core interaction findings fixed in `main`

- Channel Admin: all sixteen authenticated native endpoints now require
  `AuthContext.tenant_id == TenantContext.id` before
  `settings:manage`/`modules:manage` admission. Merged in PR #2671,
  commit `dabb11a321ad02935e98dacbdcd7e59eef2cb65f`.
- Index Admin: bootstrap requires authenticated/resolved tenant equality before
  `settings:read`. Merged in PR #2673,
  commit `aee9ba38e19d1baca31473d983462bffb069e0fc`.
- Search Admin: seven read and eight manage endpoints require tenant equality
  before `settings:read`/`settings:manage`; unauthenticated `track-click` remains
  outside the admin-permission contract. Merged in PR #2674,
  commit `f920841718500fec6966cd42564e94b73499566a`.
- Email Admin: tenant equality now precedes `settings:read` before reading
  tenant `platform_settings`. Merged in PR #2676,
  commit `53f1a719b571eb22ca9688ddc7000f583de14f16`.
- Outbox Admin: tenant equality now precedes `logs:read` before returning
  tenant-scoped outbox counters. Merged in PR #2677,
  commit `efbf35d8279c393e5a134ad2e9b956f1986b6373`.

Each fix returns a static denial on mismatch and keeps both tenant ids only in
structured private diagnostics. Focused source-contract regressions retain the
extractor counts and equality-before-permission ordering.

## Open cross-owner P0 blocker

Events Admin reads and mutates host-global `SharedEventDeliveryControl` through
ordinary tenant-scoped `settings:read`/`settings:manage`. System GraphQL also
exposes host-global system, cache and all-tenant event diagnostics through
ordinary tenant-scoped `logs:read`.

`rustok_api::AuthContext` has no typed host/platform/root authority claim, so a
tenant equality check cannot safely authorize these host-global resources.
Issue #2680 owns the required Auth/RBAC authority contract and the fail-closed
release option. The Events module handoff is `blocked` in
`crates/rustok-events-module/docs/implementation-plan.md`.

## Same-SHA workflow evidence

- Ecommerce Hardening for PR #2668 compiled `rustok-commerce` past the added
  `StoreContextError::TenantBoundary` mappings. The job then failed in
  `rustok-order-storefront` because it referenced the nonexistent
  `RequestContext.correlation_id`; PR #2683 replaced that dependency with a
  server-generated diagnostic UUID and merged as
  `b558465a209d3a6d8294fe340f64831a90f98e5f`.
- Migration Compatibility after the Search projector brace fix no longer stopped
  at the invalid `format!` JSON path. It then failed while exporting the base
  plan because Cargo attempted to update Athanor under `--locked`.
  PostgreSQL fresh-install, N-1 upgrade and Tenant locale-backfill jobs were
  skipped, so this is not Tenant migration evidence.
- Existing expired advisory exceptions, cache timeout assertions, browser
  fixture failures and unrelated Payment/Order verifier markers remain separate
  repository-wide blockers, not Tenant-owner defects.

## Remaining Tenant blockers

- Final-SHA focused static and Rust compile/test evidence for Tenant Admin, Auth
  Admin, RBAC Admin, Commerce context, lifecycle bypass and storefront scope.
- Both retained PostgreSQL concurrency races must rerun on the final reconciled
  revision.
- Live Redis tenant-locale generation/recovery evidence.
- PostgreSQL legacy-locale backfill fixture and real MySQL 8 duplicate-default
  rejection probe.
- Deployed/native parity with representative tenant identities.
- Resolution of the base Cargo.lock/Athanor `--locked` blocker so Migration
  Compatibility can reach the Tenant fixtures.
- Resolution of host-global authority issue #2680 before platform release.

## Resume commands

```sh
node scripts/verify/verify-auth-admin-tenant-scope.mjs
node scripts/verify/verify-rbac-admin-tenant-scope.mjs
node scripts/verify/verify-commerce-tenant-locale-boundary.mjs
node scripts/verify/verify-tenant-admin-native-error-safety.mjs
node scripts/verify/verify-tenant-locale-policy-migration.mjs
node scripts/verify/verify-email-admin-tenant-scope.mjs
npm run verify:tenant:fba
cargo test -p rustok-channel-admin --test tenant_scope_contract
cargo test -p rustok-index-admin --test tenant_scope_contract
cargo test -p rustok-search-admin --test tenant_scope_contract
cargo test -p rustok-outbox-admin --test tenant_scope_contract
cargo check -p rustok-commerce --all-features
cargo check -p rustok-order-storefront --all-features
cargo test -p rustok-tenant --test tenant_ensure_concurrency_postgres -- --nocapture
cargo test -p rustok-tenant --test locale_policy_concurrency_postgres -- --nocapture
cargo test -p rustok-server tenant_locale_generation --lib
```
