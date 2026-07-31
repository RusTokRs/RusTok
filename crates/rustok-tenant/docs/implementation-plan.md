# Implementation plan for `rustok-tenant`

## Current state

`rustok-tenant` owns tenant identity and validation, tenant lifecycle events,
read-only tenant-module projections, the `TenantReadPort` projection and the
revisioned `TenantLocalePolicyPort` aggregate. The server owns resolver
middleware, cache infrastructure, provisioning orchestration, module lifecycle
writes and runtime composition; it must not become a second owner of tenant
business rules.

The current source contract includes:

- compare-and-set locale-policy replacement with a required idempotency key,
  canonical non-`und` locales, exactly one enabled default, enabled fallback
  targets and an acyclic fallback graph;
- one bounded owner-port retry after a concurrent revision conflict so an
  identical cross-replica request can replay its durable receipt without
  weakening stale-revision or key-reuse conflicts;
- deterministic PostgreSQL races for locale-policy replay and concurrent
  `ensure_tenant`, each using an explicit lock barrier rather than timing;
- incremental backfill for legacy tenants with no locale rows and equivalent
  one-default-locale enforcement for PostgreSQL, SQLite and MySQL;
- mandatory transactional outbox publication for every remaining tenant owner
  mutation, including installer/bootstrap creation;
- typed tenant reads for resolver, installer, storefront and commerce consumers;
- explicit `--tenant-id` before OAuth CLI credential writes;
- `ModuleControlPlane` effective policy for Tenant Admin module badges;
- physical removal of the public low-level tenant-module writer so runtime
  module changes can only use the lifecycle control plane;
- durable generation recovery for tenant and tenant-locale caches, including
  exact, wildcard, gapped, missed-publication, Redis state-loss and generation-
  regression handling.

Tenant Admin, Auth Admin user-list/detail reads and RBAC Admin bootstrap bind the
authenticated tenant to the middleware-resolved tenant before permission
admission. Mismatches return static public denials while both ids remain only in
structured diagnostics. Focused source guards and unit regressions exist, but
same-SHA execution is still required.

Commerce `StoreContextService` consumes `TenantReadPort`,
`TenantLocalePolicyPort` and canonical `TenantLocale`; it no longer queries
owner tables or normalizes locales independently. The native storefront module-
state adapter extracts `TenantContext` and never accepts a client tenant slug.

The cross-owner trust sweep also found host-global Events/System/Settings
operations that had accepted ordinary tenant `settings:*` or `logs:*`
permissions. Commit `35afdd3a5d4ae74e735a2963e7246e21a3031e5d` (PR #2720)
source-mitigated that exposure through the separate typed
`rustok_api::HostAuthorityContext`: global reads require host `Read`, global
writes require host `Manage`, and mutations are bound to a non-nil operator
actor.

The current source follow-up implements issuance without trusting tenant OAuth
or RBAC. A rejected design that allowlisted tenant OAuth client ids was removed
before PR after the audit confirmed tenant `settings:manage` can rotate OAuth
app secrets. The replacement uses a dedicated high-entropy
`X-RusTok-Host-Token`; deployments store only SHA-256 digests, explicit
read/manage levels and non-nil audit actors in host-owned
`RUSTOK_HOST_AUTHORITY_CREDENTIALS`. Middleware removes and authenticates the
raw header once before downstream dispatch. Native transports receive only the
typed request extension, while HTTP GraphQL consumes the same typed authority
from a request-task-local scope without re-reading the credential; GraphQL
WebSocket remains fail-closed.

The final surface sweep found a separate Iggy Connector native read/write path
that still used tenant `SETTINGS_READ`/`SETTINGS_MANAGE`. PR #2726 moves that
path to host `Read`/`Manage`; its mutation uses the host operator for audit and
separately requires authenticated tenant equality with the routed tenant before
accessing tenant-owned connector secrets. Issue #2680 remains open until
same-SHA compile/unit/source evidence and retained live ordinary-tenant denial,
host admission, rotation/revocation and multi-replica parity exist. These
cross-owner findings do not change the Tenant-owner count.

Detailed post-handoff evidence is retained in
[`cycle-001-core-trust-supplement-20260731.md`](./cycle-001-core-trust-supplement-20260731.md).

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `transport_verified`
- Structural shape: `core_transport_ui`
- Provider contract: `TenantReadPort` / `tenant.read_projection.v1` in
  `crates/rustok-tenant/contracts/tenant-fba-registry.json`.
- Static/runtime evidence:
  `crates/rustok-tenant/contracts/evidence/tenant-contract-test-static-matrix.json`
  and `crates/rustok-tenant/contracts/evidence/tenant-runtime-fallback-smoke.json`.
- The admin overview remains a documented native-only exception. No public
  GraphQL/REST tenant-bootstrap UI contract exists.

## Open results

1. **Execute concurrent tenant provisioning evidence.** Run
   `tenant_ensure_concurrency_postgres` with two independent owner connections
   crossing the absence read before the advisory-lock insert barrier releases.
   **Done when:** one result is `created=true`, one is `created=false`, both
   return the same tenant, and one locale seed plus one `tenant.created` event
   are durable on the same SHA.

2. **Execute concurrent locale-policy replay evidence.** Run
   `locale_policy_concurrency_postgres` with the retained tenant-row barrier.
   **Done when:** identical calls converge on one revision, receipt and event
   set, while different-payload reuse remains a typed conflict on the same SHA.

3. **Execute cache recovery evidence.** Run exact UUID, wildcard, durable-ahead,
   deterministic lag, missed-publication and Redis loss/restoration scenarios.
   **Done when:** compiled and live multi-replica recovery passes on one revision
   without stale locale data beyond the documented reconciliation bound.

4. **Execute cross-backend migration evidence.** Run the source guard, retained
   PostgreSQL legacy-locale fixture and a real MySQL 8 incremental migration with
   an attempted second default row.
   **Done when:** the backfill ordering is proven and MySQL installs
   `default_tenant_guard` and rejects duplicate defaults.

5. **Execute focused trust and compile evidence.** Run Tenant Admin, Auth Admin,
   RBAC Admin, commerce context, storefront, lifecycle-bypass and host-authority
   guards/tests on one reconciled revision.
   **Done when:** formatting, source guards, targeted compile/tests and module
   validation pass without relying on the removed temporary PR workflow.

6. **Collect deployed/native parity.** Exercise representative tenant identities,
   mismatched routes, disabled/not-found tenants, module policy, locale selection,
   static public errors and cache invalidation in a composed host.

7. **Retain host-operator execution evidence.** Keep ordinary tenant requests and
   WebSocket host operations denied; prove the host-owned credential over HTTP
   GraphQL plus Events and Iggy native transports.
   **Done when:** no-header/wrong-token denial, read/manage hierarchy, audit
   identity, Iggy authenticated/resolved tenant secret ownership, overlap
   rotation, revocation and multi-replica parity pass on one reconciled SHA and
   issue #2680 can close.

8. **Keep ownership closed.** New runtime, admin, Translation, commerce or
   installer consumers must use the typed owner ports; direct `tenant_locales`
   access, first-active-tenant authority and low-level module-state writes are
   prohibited.

## Verification

```sh
npm run verify:tenant:fba
npm run verify:tenant:admin-boundary
node scripts/verify/verify-auth-admin-tenant-scope.mjs
node scripts/verify/verify-rbac-admin-tenant-scope.mjs
node scripts/verify/verify-commerce-tenant-locale-boundary.mjs
node scripts/verify/verify-tenant-admin-native-error-safety.mjs
node scripts/verify/verify-tenant-locale-policy-migration.mjs
node scripts/verify/verify-host-global-authority-boundary.mjs
cargo test -p rustok-api host_authority -- --nocapture
cargo test -p rustok-server host_authority --lib -- --nocapture
cargo xtask module validate tenant
cargo xtask module test tenant
cargo xtask module validate commerce
cargo check -p rustok-tenant-admin --features ssr
cargo test -p rustok-tenant-admin --features ssr tenant_admin_scope_requires_matching_tenant -- --nocapture
cargo check -p rustok-auth-admin --features ssr
cargo test -p rustok-auth-admin --features ssr auth_admin_scope_requires_matching_tenant -- --nocapture
cargo check -p rustok-rbac-admin --features ssr
cargo test -p rustok-rbac-admin --features ssr rbac_admin_scope_requires_matching_tenant -- --nocapture
cargo check -p rustok-commerce --all-features
cargo test -p rustok-commerce --test context_service_test -- --nocapture
cargo check -p rustok-events-module
cargo test -p rustok-events-module
cargo check -p rustok-iggy-connector-admin --features ssr
cargo check -p rustok-server --lib
cargo check -p rustok-storefront
cargo test -p rustok-auth-cli oauth_create_app -- --nocapture
cargo test -p rustok-tenant --test tenant_ensure_concurrency_postgres -- --nocapture
cargo test -p rustok-tenant --test locale_policy_concurrency_postgres -- --nocapture
cargo test -p rustok-tenant --test integration tenant_mutations_always_publish_outbox_events -- --nocapture
cargo test -p rustok-tenant tenant_read_port --test integration
cargo test -p rustok-tenant tenant_locale_policy --test integration
cargo test -p rustok-server --test lifecycle_bypass_guard
cargo test -p rustok-server --test tenant_locale_generation_guard
cargo test -p rustok-server tenant_locale_generation --lib
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379/ \
RUSTOK_CACHE_REDIS_SERVER_BIN=/usr/bin/redis-server \
  cargo test -p rustok-server tenant_locale_generation --lib -- --ignored --nocapture --test-threads=1
```

The PostgreSQL locale fixture is `tenant-locale-policy-invariants` in
`docs/migrations/backfill-contracts.json`. A real MySQL 8 duplicate-default probe
is still required; source inspection is not a substitute.

## References

- [Cycle-001 Core trust supplement](./cycle-001-core-trust-supplement-20260731.md)
- [Commerce tenant-locale owner cutover](../../rustok-commerce/docs/tenant-locale-owner-cutover.md)
- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)
- [Events runtime adapter plan](../../rustok-events-module/docs/implementation-plan.md)
- [Host-global operator runbook](../../../apps/server/docs/host-authority.md)
- GitHub issue #2680 for retained host-operator authority evidence

## Change rules

1. Keep tenancy business logic, `TenantReadPort` and
   `TenantLocalePolicyPort` in this module.
2. Keep module lifecycle writes behind `ModuleLifecycleService` or
   `ModuleControlPlane`.
3. Treat tenant equality as mandatory before tenant permission admission, but do
   not disguise host-global resources as tenant-owned.
4. Do not infer host authority from a default tenant, first active tenant, OAuth
   application, scope, metadata, wildcard, built-in tenant role or magic UUID.
5. Do not store raw host tokens in tenant rows, OAuth rows, repository files,
   logs, URLs or browser storage.
6. Update this plan and the master verification cursor with every contract or
   status change. Never claim source, compile or live evidence that did not run.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-07-31`
- Scope inspected: `tenant owner CRUD and provisioning concurrency; locale-policy CAS/idempotency and migration invariants; lifecycle outbox; cache generation; storefront and commerce owner scope; Tenant/Auth/RBAC Admin authenticated-resolved tenant equality; host-global Events, Iggy, System and Settings authority; tenant OAuth administration; native/HTTP GraphQL/WebSocket composition`
- Findings: `P0=3, P1=11, P2=1, P3=0` (Tenant-owner count; cross-owner issue #2680 is tracked separately with two P0 surfaces and one P1 transport-secret-lifetime defect)
- Fixed in this pass: `Tenant owner and directly related P0/P1/P2 corrections are merged in main through PR #2665; subsequent Channel, Index, Search, Email and Outbox Admin tenant-scope corrections are recorded in the cycle supplement; PR #2720 merged the typed fail-closed host authority contract; PR #2726 replaces the rejected tenant-OAuth client allowlist design with a host-owned opaque token digest policy, one-shot middleware removal, typed native/HTTP GraphQL admission, WebSocket denial, overlap rotation support, separate Iggy authenticated/resolved tenant secret ownership and host-audited Iggy native control`
- Remaining risks or blockers: `same-SHA formatting, source guard, Rust compile/test and module validation are pending; both PostgreSQL races must rerun on the final revision; live Redis recovery, PostgreSQL backfill, real MySQL 8, deployed/native parity and remaining cache/RBAC inspection are required; issue #2680 needs retained live host credential denial/admission, audit actor, rotation/revocation and replica parity evidence`
- Evidence: `source files, unit regressions and the operator runbook are present on PR #2726; no execution claim is made because connector-only local execution remains unavailable while github.com DNS resolution fails; the old temporary Tenant workflow was not merged and is not evidence for the current SHA; historical PostgreSQL race passes occurred on b88e41d92815f9085467bfed4e0d62f6fc29f5c6 and must be rerun`
- Next action: `inspect every PR #2726 exact-head check and fix branch-related failures; then execute retained Tenant PostgreSQL, Redis and migration evidence before advancing the cursor`
- Resume command: `node scripts/verify/verify-host-global-authority-boundary.mjs && cargo test -p rustok-api host_authority -- --nocapture && cargo test -p rustok-server host_authority --lib -- --nocapture && cargo check -p rustok-events-module && cargo check -p rustok-iggy-connector-admin --features ssr && cargo check -p rustok-server --lib && cargo test -p rustok-tenant --test tenant_ensure_concurrency_postgres -- --nocapture && cargo test -p rustok-tenant --test locale_policy_concurrency_postgres -- --nocapture`
