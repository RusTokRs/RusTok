# Implementation plan for `rustok-tenant`

## Current state

`rustok-tenant` owns the tenant domain, tenant-module read contract, validation,
tenant lifecycle events, the `TenantReadPort` read projection, and the
revisioned `TenantLocalePolicyPort` aggregate. The server owns resolver
middleware, cache infrastructure, provisioning orchestration, module lifecycle
writes, and runtime composition; it must not take over tenant business rules.

Locale-policy replacement is compare-and-set guarded and requires an
idempotency key. The owner validates canonical non-`und` tenant locales,
exactly one enabled default, enabled fallback targets, and acyclic fallback
graphs. It synchronizes `tenants.default_locale`, records a durable replay
receipt, and emits tenant/locale events in the owner transaction. The public
owner port performs one bounded retry when a concurrent commit first surfaces
as a revision conflict, allowing the durable receipt to replay an identical
cross-replica request while preserving real stale-revision and key-reuse
conflicts. Server locale resolution loads this projection through the port and
keeps only host-owned cache/invalidation behavior.

The retained PostgreSQL locale race makes this boundary deterministic rather
than timing-dependent. A separate transaction holds the tenant row lock while
two independent owner connections pass the initial receipt lookup and wait
before revision CAS. Releasing the barrier proves both identical calls return
the same revision-2 projection, only one receipt and one locale event set are
durable, and different-payload reuse of the same key remains a typed conflict.

`TenantService::ensure_tenant` preserves its promised idempotent provisioning
semantics under concurrency. After the initial absence read, the ordinary
`create_tenant` path still owns validation, locale seeding and transactional
`tenant.created` publication. If another replica wins the unique slug insert,
the loser re-reads by slug and returns the committed tenant with `created=false`;
if no tenant exists after the failed create, the original error is preserved.
The retained PostgreSQL test holds an advisory transaction lock through a
`BEFORE INSERT` trigger so both independent calls pass the absence read before
one unique winner is released.

The owner migration closes the incremental-upgrade gap created when
`tenant_locales` was introduced after `tenants`: any legacy tenant with no locale
rows receives one enabled default row derived from `tenants.default_locale`
before policy constraints are installed. PostgreSQL and SQLite enforce at most
one default row with filtered unique indexes. MySQL uses a nullable generated
`BINARY(16)` tenant guard plus a unique index, so its schema rejects the same
multiple-default corruption instead of relying only on owner validation. The
retained PostgreSQL fixture proves one backfilled row with revision `0`, and a
focused source guard preserves both backfill ordering and the MySQL guard.

Every remaining `TenantService` mutation writes its validated lifecycle event to
the canonical outbox before committing the owner transaction. `TenantService::new`
is the only constructor; event publication cannot be omitted by host wiring.
Installer/bootstrap creation therefore uses the same `tenant.created`
transaction as ordinary tenant creation.

The host cache-miss resolver and installer provisioning/verification use
`TenantReadPort` for typed id, slug, and domain reads. The module keeps inactive
tenants hidden unless explicitly requested and requires read deadlines. Cache
invalidation after lifecycle changes remains a server-owned integration
responsibility.

Operational write adapters must not convert the global
`read_default_active_tenant` convenience into tenant authority. The auth CLI
`oauth create-app` command requires an explicit `--tenant-id` before it can
persist OAuth credentials. Unit regressions and the tenant FBA guard prohibit
reintroducing first-active-tenant inference into that credential path.

The Leptos storefront native module-state adapter no longer accepts a configured
or client-provided tenant slug. Native server functions execute after tenant
middleware and now extract `rustok_api::TenantContext`, then read module state
only for `tenant.id`. The configured slug remains isolated to the GraphQL
transport as a host-routing hint. The tenant FBA verifier forbids slug arguments
and `get_tenant_by_slug` in the native adapter.

The Tenant Admin bootstrap no longer reconstructs effective module availability
from raw `tenant_modules` rows. It resolves the active composition snapshot and
uses `ModuleControlPlane::effective_policy(...).resolve_enabled(tenant.id)`, while
raw overrides remain explanatory metadata only. Manifest defaults and dependency
closure therefore match the server and storefront control-plane contract.

The public low-level tenant-module writer has been removed. `ToggleModuleInput`,
`TenantService::toggle_module`, its crate-root export, and legacy-only tests no
longer exist. Runtime module enable/disable must use `ModuleLifecycleService` or
`ModuleControlPlane`, preserving policy and dependency checks, lifecycle hooks,
and operation journaling. `TenantService` retains only read-only tenant-module
projections for admin/runtime consumers.

Tenant resolution and the separate byte-weighted tenant-locale cache share the
durable tenant generation channel. In-order records invalidate the exact tenant
locale entry, namespace-wide manual rotations carry `*`, and unverified, gapped,
lagged or reconciled advancement clears every process-local locale entry before
acknowledgement. Every event is checked against durable generation before cache
mutation or tracker acknowledgement. When durable state is already ahead of an
otherwise in-order exact event, the listener treats the difference as a missed
invalidation, full-clears the locale namespace and records the durable offset
instead of applying only the event key. Generation regression clears local
locale values but remains fail-closed; it never lowers the trusted process epoch.
The listener exposes recovery health, not only task liveness, to the critical
runtime guardrail.

Source evidence now covers:

- mandatory same-transaction outbox insertion for ordinary and installer tenant
  creation, tenant updates, and locale-policy changes;
- concurrent-idempotent `ensure_tenant` loser replay through the committed slug,
  with a PostgreSQL advisory-lock insert barrier proving two independent owner
  connections return one tenant, one locale seed, one `tenant.created` event and
  exactly one `created=true` result;
- bounded public-port recovery after a locale-policy revision conflict, with a
  permanent guard requiring exactly one retry through the same durable
  idempotency key;
- a retained PostgreSQL two-connection locale-policy race with an explicit row-lock
  barrier, identical-result assertion, one-revision/one-receipt/one-event-set
  durability checks, and a different-payload key-reuse conflict assertion;
- migration-time creation of one enabled default locale row for every legacy
  tenant with no `tenant_locales`, a retained PostgreSQL fixture contract, and a
  source guard proving the backfill runs before constraints;
- equivalent at-most-one-default schema enforcement across PostgreSQL, SQLite and
  MySQL, with the MySQL generated nullable tenant guard permanently required by
  the focused migration verifier;
- explicit tenant UUID selection before auth CLI OAuth credential writes, with
  missing/invalid/valid UUID cases and a source guard forbidding
  `read_default_active_tenant` in that write path;
- trusted storefront native module-state reads through resolved `TenantContext`,
  with configured slug use restricted to the GraphQL transport branch;
- effective Tenant Admin module badges from the active composition and
  `ModuleControlPlane`, never raw tenant override rows as policy authority;
- physical removal of the public low-level module writer, DTO/export and legacy
  integration evidence, with the FBA verifier forbidding every removed symbol;
- exact UUID, wildcard, deterministic lag, durable-ahead, missed-publication,
  Redis state-loss and generation-regression recovery guards for tenant locale
  caches.

The lifecycle outbox, tenant provisioning replay, locale-policy
idempotency/concurrency, legacy-locale backfill, cross-backend default uniqueness,
explicit OAuth tenant-selection, trusted storefront module-state scope, and
lifecycle-bypass removal regressions are staged for same-SHA execution.
Multi-replica cache evidence remains source-complete but is not compiled or live
verified on the current revision. The retained PostgreSQL races and a real MySQL
migration run must pass on the same revision before this component can be
completed.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `transport_verified`
- Structural shape: `core_transport_ui`
- FBA provider contract: `TenantReadPort` / `tenant.read_projection.v1` in
  `crates/rustok-tenant/contracts/tenant-fba-registry.json`.
- Static and runtime evidence:
  `crates/rustok-tenant/contracts/evidence/tenant-contract-test-static-matrix.json`
  and `crates/rustok-tenant/contracts/evidence/tenant-runtime-fallback-smoke.json`.
- The admin overview is a documented native-only exception: no public
  GraphQL/REST tenant-bootstrap UI contract exists yet. Its native adapter uses
  `HostRuntimeContext` and is host-neutral.

## Open results

1. **Execute concurrent tenant provisioning evidence.** Run
   `tenant_ensure_concurrency_postgres` so two independent owner connections pass
   the absence read, wait at the advisory-lock insert trigger, and converge on one
   committed tenant after release.
   **Depends on:** `RUSTOK_TENANT_TEST_DATABASE_URL` or `DATABASE_URL`.
   **Done when:** one call reports `created=true`, the other `created=false`, both
   return the same tenant id, only one locale seed and `tenant.created` event are
   durable, and targeted installer seed compilation passes on the same SHA.

2. **Execute multi-replica tenant-locale recovery evidence.** Run exact UUID,
   wildcard, durable-ahead gap recovery, deterministic lag, missed-publication
   reconciliation and Redis state-loss/restoration scenarios on the same
   reconciled revision.
   **Depends on:** the permanent cache workflow or another Rust 1.96 environment
   with isolated Redis 7 and `redis-server`.
   **Done when:** compiled and live Redis jobs pass on one revision and every
   failure is fixed.

3. **Execute the retained PostgreSQL locale-policy concurrency evidence.** Run
   `locale_policy_concurrency_postgres` against PostgreSQL so two independent
   owner connections pass receipt lookup, wait on the explicit tenant-row barrier,
   and converge through the durable receipt after release.
   **Depends on:** `RUSTOK_TENANT_TEST_DATABASE_URL` or `DATABASE_URL`.
   **Done when:** one revision, one receipt/event set, identical projections and
   typed different-payload key-reuse conflict are proven on the same SHA.

4. **Execute cross-backend locale-policy migration evidence.** Run the focused
   source guard, retained PostgreSQL backfill fixture, and a real MySQL 8
   incremental migration with an attempted second default locale row.
   **Depends on:** a MySQL 8 test database; the repository migration workflow
   currently exercises PostgreSQL only.
   **Done when:** MySQL installs `default_tenant_guard` and rejects a second
   default for the same tenant.

5. **Collect deployed/native transport parity evidence.** Confirm host locale,
   tenant-scoped RBAC, disabled/not-found behavior, native storefront module-state
   scope, and typed error mapping in a composed runtime.
   **Depends on:** a deployed host with representative tenant identities.

6. **Keep lifecycle and cache behavior synchronized.** Any create, update,
   deactivate, domain, locale, or module-state change must preserve transactional
   outbox publication, typed ports, and durable generation invalidation.

7. **Maintain FBA read-projection compatibility.** Evolve selector, deadline,
   inactive-tenant or error semantics atomically across provider, resolver,
   installer, metadata and evidence.

8. **Keep locale-policy ownership closed.** New runtime, admin, Translation or
   installer consumers must use `TenantLocalePolicyPort`; direct
   `tenant_locales` access outside the owner is prohibited.

## Verification

- `npm run verify:tenant:fba`
- `npm run verify:tenant:admin-boundary`
- `node scripts/verify/verify-tenant-locale-policy-migration.mjs`
- PostgreSQL fixture `tenant-locale-policy-invariants` from
  `docs/migrations/backfill-contracts.json`
- MySQL 8 incremental migration plus duplicate-default rejection probe
- `cargo xtask module validate tenant`
- `cargo xtask module test tenant`
- `cargo test -p rustok-auth-cli oauth_create_app -- --nocapture`
- `cargo test -p rustok-tenant --test tenant_ensure_concurrency_postgres -- --nocapture`
- `cargo test -p rustok-tenant --test integration tenant_mutations_always_publish_outbox_events -- --nocapture`
- `cargo test -p rustok-tenant tenant_read_port --test integration`
- `cargo test -p rustok-tenant tenant_locale_policy --test integration`
- `cargo test -p rustok-tenant --test locale_policy_concurrency_postgres -- --nocapture`
- `cargo test -p rustok-server --test lifecycle_bypass_guard`
- `cargo check -p rustok-storefront`
- `cargo test -p rustok-server --test tenant_locale_generation_guard`
- `cargo test -p rustok-server tenant_locale_generation --lib`
- `RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379/ RUSTOK_CACHE_REDIS_SERVER_BIN=/usr/bin/redis-server cargo test -p rustok-server tenant_locale_generation --lib -- --ignored --nocapture --test-threads=1`

## References

- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)

## Change rules

1. Keep tenancy business logic, `TenantReadPort`, and
   `TenantLocalePolicyPort` in this module.
2. Update the local README, `rustok-module.toml`, and server documentation with a
   public/runtime contract change.
3. Update this status block and `docs/modules/registry.md` with a UI or transport
   boundary change.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-07-30`
- Scope inspected: `tenant owner CRUD and provisioning concurrency, locale-policy CAS/idempotency concurrency, incremental and cross-backend locale-policy migration invariants, lifecycle outbox publication, installer orchestration, module control-plane exclusivity, native storefront module-state trust, read ports, FBA guard, native admin RBAC and operational OAuth tenant selection; resolver/cache generation and remaining RBAC parity stay under audit`
- Findings: `P0=0, P1=8, P2=1, P3=0`
- Fixed in this pass: `made every remaining TenantService mutation publish through TransactionalEventBus::publish_root_in_tx; made ensure_tenant replay a concurrent unique-slug winner and retained an advisory-lock PostgreSQL race; added one bounded locale-policy conflict retry and deterministic PostgreSQL receipt race; backfilled legacy tenants missing locale policy; required explicit tenant UUID before OAuth CLI credential writes; made storefront native enabled-module reads use resolved TenantContext instead of a client slug; made Tenant Admin render ModuleControlPlane effective policy instead of raw tenant overrides; added MySQL generated nullable tenant UUID uniqueness; removed TenantService::toggle_module, ToggleModuleInput, the crate export and legacy tests so runtime module writes can only use the lifecycle control plane`
- Remaining risks or blockers: `same-SHA targeted Rust, both PostgreSQL races, PostgreSQL backfill fixture, lifecycle/tenant-admin/storefront compilation and real MySQL migration evidence is pending; multi-replica Redis evidence is source-complete but unexecuted; resolver invalidation and remaining RBAC parity require continued inspection`
- Evidence: `storefront native adapter has no tenant argument and extracts rustok_api::TenantContext before list_tenant_modules(tenant.id); Tenant Admin resolves the active composition through ModuleControlPlane and uses resolve_enabled(tenant.id) for badges; enabled_modules.rs passes configured slug only in the GraphQL branch; Tenant FBA forbids native tenant_slug/get_tenant_by_slug and binds the transport split; service/integration diffs contain only ensure replay plus physical removal of the low-level writer; provisioning and locale PostgreSQL races retain independent connections and explicit lock barriers; completed broad workflow failures inspected so far remain outside tenant scope`
- Next action: `run same-SHA Tenant FBA, lifecycle bypass guard, rustok-storefront check, auth CLI tests, tenant_ensure_concurrency_postgres, locale_policy_concurrency_postgres, focused migration guard, PostgreSQL fixture and real MySQL duplicate-default probe; then continue cache/RBAC inspection`
- Resume command: `node scripts/verify/verify-tenant-locale-policy-migration.mjs && npm run verify:tenant:fba && cargo test -p rustok-auth-cli oauth_create_app -- --nocapture && cargo test -p rustok-tenant --test tenant_ensure_concurrency_postgres -- --nocapture && cargo test -p rustok-tenant --test locale_policy_concurrency_postgres -- --nocapture && cargo test -p rustok-tenant tenant_locale_policy --test integration -- --nocapture && cargo test -p rustok-tenant --test integration tenant_mutations_always_publish_outbox_events -- --nocapture && cargo test -p rustok-server --test lifecycle_bypass_guard && cargo check -p rustok-storefront && cargo xtask module validate tenant`
