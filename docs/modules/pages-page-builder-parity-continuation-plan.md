# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-05
Status: source-parity-current / production-gate-postgres-restart-source-ready / execution-evidence-pending
Scope: `rustok-pages` admin/storefront FFA and `rustok-page-builder` consumer-property, publication, artifact, event and cache boundaries

## Source-of-truth policy

This is the canonical shared continuation cursor. Historical dated packets remain evidence of the source slices that produced the present state, but they do not override this plan.

`source-ready` means that code, contracts or retained harness source exists. It does not mean that tests, Cargo, formatting, verifiers, databases, HTTP routes, server functions, production event topology, browsers, workflows, CI or tenant rollout were executed.

Pages and Page Builder continue as one vertical pipeline with explicit owners. Local module plans describe owner-local work; this shared plan decides whether the producer/consumer seam is closed and prevents either module from declaring the combined scenario complete while the other side still has a tail.

## Audit basis

The current source was rechecked against:

- `crates/rustok-pages/docs/implementation-plan.md`;
- `crates/rustok-page-builder/docs/implementation-plan.md`;
- `docs/modules/page-builder-implementation-plan.md`;
- the Pages admin draft/published metadata composition;
- reviewed publish and immutable rollback owner services;
- the Page Builder reviewed runtime, authoritative sanitizer and materialized artifact contracts;
- the Pages durable outbox/cache invalidation and read contracts;
- `OutboxRelay` claim/delivery/acknowledgement ordering;
- the server `EventRuntime` transport composition;
- `TenantCacheGenerationTransport` and `TenantGenerationDeliveryGate`;
- `ServerPagesCachePort` generation and byte-cache ownership;
- the historical PostgreSQL owner-transaction and pre-handler relay-restart harnesses;
- the module `EventDispatcher` filtering and asynchronous handler execution;
- the native storefront adapter, registered Leptos server function and public artifact HTTP route;
- the Channel owner module-binding contract;
- recent merged Pages parity PRs and their dated packets.

The audit remains source-only. Execution evidence remains pending.

## Rechecked merged cursor

Current `main` contains:

- PR #2955 — publish/rollback event-correlation and generation miss/refill contract;
- PR #2971 — source-ready PostgreSQL publish/rollback outbox-to-cache packet;
- PR #2974 — source-ready durable relay failure/restart packet;
- PR #2979 — source-ready SQLite/Axum public artifact HTTP cache packet;
- PR #2985 — source-ready native storefront composite-cache contract and plan actualization;
- PR #2988 — source-ready registered Leptos storefront route with real Pages owner data and cache behavior;
- PR #2990 — source-ready routed-channel admission before native generation/cache lookup;
- PR #2992 — source-ready reviewed Fly publication through channel-constrained immutable artifact selection and integrity-before-fill;
- PR #2995 — source-ready owner/outbox/handler/native-route continuity through a synchronous test relay target;
- PR #2997 — topology correction separating the synchronous test target from production asynchronous listener delivery;
- PR #3001 — production synchronous Pages generation gate with process-bounded same-event dedupe;
- PR #3004 — production gate to registered native route continuity source.

The current slice joins the historical PostgreSQL packets to the production gate. It changes no production behavior.

## Current parity state

### Registered metadata surfaces: source-complete

Pages owns one registered six-field `rustok.pages.metadata` contribution. Draft workspaces mount the canonical Page Builder consumer-properties panel inside Fly. Published pages mount the same registered panel in a Pages-owned standalone host without an editable Fly canvas.

The bespoke `PageMetadataEditor` and its direct workspace metadata transport write are removed. Persistence remains owned by `PagesMetadataPropertyPort`, with a metadata revision independent from the Fly document revision.

Focused stale-revision and dirty-Fly isolation regressions are source-ready. Their execution and the published browser packet remain open.

### Reviewed publication: source-complete

Pages owns the reviewed publish transaction from exact metadata/body revisions and promoted scenario review through authoritative sanitization, runtime materialization, immutable artifact persistence/binding, published state, transactional `NodeUpdated`/`NodePublished` events and the durable publish receipt plus exact artifact manifest.

Create-time/default-runtime Page Builder publication is removed. Non-builder publication rejects every Fly/GrapesJS body.

### Immutable rollback: source-complete

Pages owns a separate idempotent rollback command and receipt. Rollback selects a prior exact publish manifest, verifies immutable artifacts, replaces locale bindings, advances the published page version and writes lifecycle events plus its receipt in one transaction.

Rollback does not sanitize, materialize or compile the current Fly document. GraphQL, HTTP, OpenAPI and the typed admin prepare/confirm control are connected.

### Public artifact HTTP cache: source-ready

The public artifact route has a retained SQLite/Axum harness using real Pages migrations, a valid deterministic Page Builder artifact, `HostRuntimeContext`, the typed cache runtime and the public router.

The source packet retains generation-7 miss/refill/hit, generation-8 key rotation, old-value physical retention, conditional `304`, empty conditional bodies and the production security/header contract.

SQLite/Axum execution remains pending.

### Native storefront registered route set: source-ready

The retained native packets cover:

- composite cache miss/refill/hit, rotation and fail-open behavior;
- the real registered `/api/fn/pages/storefront-data` Leptos server function;
- routed-channel module admission before generation/cache lookup;
- reviewed Fly publication and immutable artifact selection;
- integrity-before-cache-fill behavior;
- old-key physical retention after generation rotation.

The route set remains unexecuted.

### Reviewed publish to native refill through synchronous test target: source-ready

The PR #2995 harness retains this source sequence:

```text
PageService::create
  → durable NodeCreated
PageService::publish_reviewed
  → reviewed immutable artifact + durable receipt
  → durable NodeUpdated
  → durable NodePublished
OutboxRelay(batch=1, concurrency=1)
  → custom synchronous relay target
  → real PageCacheInvalidationEventHandler
registered native web request
  → miss/refill under generations 4/6/7
OutboxRelay + synchronous target
  → NodePublished route/page/artifact rotation to 5/7/8
registered native web request
  → different-key miss/refill
registered native web request
  → hit without another put
```

This proves owner, relay, handler and route contracts in one process. It remains a test-target packet and does not replace production-gate execution evidence.

### Production relay-to-Pages generation gate: source-ready

The production delivery chain retains:

```text
publisher or OutboxRelay
  → TenantCacheGenerationTransport
  → TenantGenerationDeliveryGate
      → canonical local-listener readiness when Redis is absent
      → PageCacheInvalidationEventHandler::handles
      → PageCacheInvalidationEventHandler::handle
      → downstream EventTransport::publish
  → listener_bus
  → asynchronous module EventDispatcher
  → Pages listener duplicate no-op for the same event UUID
```

The synchronous Pages invalidation now precedes downstream transport acceptance. `OutboxRelay` therefore cannot mark a Pages lifecycle row dispatched unless the real Pages invalidation runtime has returned a valid event/correlation-bound receipt.

`ServerPagesCachePort` uses one process-bounded dedupe keyed by stable event UUID. Same-event work is serialized. A successful invalidation is recorded before downstream delivery, so downstream rejection allows delivery retry without a second generation rotation. An invalidation error returns before the UUID is recorded, allowing the relay or publisher to retry the invalidation.

The asynchronous Pages module listener remains registered for memory, OutboxLocal and OutboxIggy profiles. A separately constructed provider resolves the same process-bounded dedupe and returns a valid current receipt without another bump.

A process restart intentionally loses this bounded optimization. Replaying an already-rotated event after restart can conservatively rotate another generation; it cannot expose stale data. Exact-once invalidation across process restarts is not claimed.

### Production relay gate to registered native route: source-ready

The PR #3004 server integration source connects the production gate to the registered route:

```text
reviewed publish
  → durable NodeCreated / NodeUpdated / NodePublished
real OutboxRelay
  → production TenantGenerationDeliveryGate
  → production ServerPagesCachePort generation rotation
  → downstream acceptance
  → outbox acknowledgement
registered /api/fn/pages/storefront-data
  → production generation snapshot
  → old-key fill before NodePublished
  → new-key miss/refill/hit after NodePublished
```

One `CacheService` owns both generation state and stored bytes. The route consumes a recording wrapper that delegates every operation to the real `ServerPagesCachePort`; it does not replace the production cache provider.

The source asserts `NodeUpdated` moves generations from `0/0/0` to `1/1/0`, the first native request fills the old composite key, and `NodePublished` moves generations to `2/2/1` before its outbox row is dispatched. A later normal Pages listener call with the same envelope is a duplicate no-op. The next route request uses a different key, misses, refills the same reviewed immutable artifact, and the following request hits without another put.

The old composite key remains physically readable but unreachable through the new generation snapshot. No scan, wildcard deletion or physical eviction policy is introduced.

Source evidence is retained in:

- `apps/server/tests/pages_production_relay_native_route_sqlite.rs`;
- `apps/server/Cargo.toml` test-only `rustok-pages-storefront` SSR dependency;
- `crates/rustok-pages/contracts/evidence/pages-production-relay-native-route-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-production-relay-native-route.mjs`;
- `docs/modules/pages-page-builder-production-relay-native-route-packet-2026-08-05.md`.

The new-key miss/refill/hit sequence is source-ready; execution remains pending.

### Production gate PostgreSQL publish/rollback restart: source-ready

The new server-level PostgreSQL source closes the topology gap left between the historical packets and the production gate:

```text
PostgreSQL publish transaction
  → durable NodePublished + publish receipt
  → commit
OutboxRelay
  → production TenantGenerationDeliveryGate
  → production ServerPagesCachePort
  → generations 0/0/0 → 1/1/1
  → downstream acceptance
  → durable acknowledgement

PostgreSQL rollback transaction
  → durable NodePublished + rollback receipt
  → commit
first relay worker
  → production gate rotates 1/1/1 → 2/2/2
  → post-invalidation downstream failure
  → durable row stays Pending with retry_count=1
second relay instance
  → same stable event UUID
  → process-bounded dedupe returns the current receipt without a second rotation
  → downstream acceptance
  → durable row becomes Dispatched
ordinary Pages listener
  → same-event rotation no-op
```

The historical owner-transaction and pre-handler restart packets remain separate. PR #2971 still proves receipt/outbox transaction ordering and receipt-conflict rollback. PR #2974 still proves durable retry state when the custom target fails before the Pages handler. The new harness proves the production semantics that matter after PR #3001: generation rotation can succeed before downstream failure, and retry must deliver without another bump in the same process.

Both publish and rollback derive new storefront and artifact keys from the production generation snapshot, observe misses, refill the new keys, and retain old-generation values physically. The harness uses an isolated PostgreSQL schema with real `OutboxModule` and `PagesModule` migrations.

Source evidence is retained in:

- `apps/server/tests/pages_production_gate_postgres_restart.rs`;
- `crates/rustok-pages/contracts/evidence/pages-production-gate-postgres-restart-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-production-gate-postgres-restart.mjs`;
- `docs/modules/pages-page-builder-production-gate-postgres-restart-packet-2026-08-05.md`.

PostgreSQL execution remains pending.

## Retained source marker index

This section keeps historical static guards stable while the canonical cursor advances.

- native storefront cache source packet; execution evidence remains pending.
- Native storefront registered server function: source-ready; the real registered Leptos endpoint is retained. Routed-channel module admission remains open for execution, and durable `NodePublished` relay delivery is now connected at source level.
- `native-storefront-reviewed-artifact-source-ready`; Native reviewed immutable artifact selection: source-ready. Verification reconstructs the full Page Builder materialization envelope before a registered native storefront miss/refill.
- `native-storefront-channel-admission-source-ready`; Routed-channel admission before native lookup: source-ready. A populated composite cache cannot bypass channel module admission, and successful reads retain a verified immutable Page Builder artifact.
- Metadata revision/isolation source packet: ready, unvalidated. A stale metadata revision short-circuits before patch transport; the metadata-only transport request excludes document data; dirty Fly state is not accepted by the metadata owner port. Execution evidence remains pending. Verifier: `verify-pages-metadata-revision-isolation.mjs`.
- `production-relay-generation-gate-source-ready`; synchronous Pages invalidation now precedes downstream transport acceptance and uses process-bounded dedupe.
- `production-relay-native-route-source-ready`; Production relay gate to registered native route: source-ready.
- `production-gate-postgres-restart-source-ready`; Production gate PostgreSQL publish/rollback restart: source-ready. The packet retains a post-invalidation downstream failure and keeps the historical owner-transaction and pre-handler restart packets separate.

## Parity matrix

| Capability | Pages owner | Page Builder owner | Source state | Execution state |
| --- | --- | --- | --- | --- |
| Metadata schema/values | Pages | Contract/runtime validation | Complete | Conflict/isolation and browser packets pending |
| Draft registered metadata | Pages host/runtime | Canonical panel | Complete | Browser execution pending |
| Published registered metadata | Pages standalone host | Canonical panel | Complete | Browser execution pending |
| Legacy metadata editor | None | None | Removed | Not applicable |
| Reviewed publish | Pages lifecycle/artifacts/outbox | Review/sanitization/materialization contracts | Complete | Database/runtime evidence pending |
| Immutable rollback | Pages lifecycle/artifacts/outbox | No delivery ownership | Complete | Database/runtime evidence pending |
| Artifact HTTP miss/refill/hit | Pages route/artifact owner | Immutable artifact verification | Source-ready | SQLite/Axum and PostgreSQL HTTP pending |
| Native storefront route/cache/admission | Pages/Channel owners | Published artifact contract | Source-ready | SQLite/Axum route set pending |
| Native reviewed immutable artifact selection | Pages publish/binding/route/cache owners | Review/sanitization/materialization/integrity | Source-ready | SQLite/Axum reviewed route pending |
| Relay + handler + native refill via test target | Pages publish/outbox/handler/route/cache owners | Reviewed artifact producer contract | Source-ready, topology-corrected | SQLite test-target execution pending |
| Production relay acknowledgement after Pages invalidation | Server delivery gate / Pages invalidation owner | No delivery ownership | Source-ready | Server unit/profile execution pending |
| Production relay to registered native route | Server gate / Pages route/cache owners | Reviewed artifact producer contract | Source-ready | Server SQLite/Axum execution pending |
| Production PostgreSQL publish/rollback and relay retry | Server gate / Pages outbox/cache owners | No delivery ownership | Source-ready | PostgreSQL execution pending |
| Fly document mutation | Pages builder facade | Fly/Page Builder | Draft-only | Browser/runtime evidence pending |
| Published Fly authoring | Not allowed | Not mounted | Correctly blocked | Bundle/runtime proof pending |

## Changes in this slice

1. Add one environment-gated server PostgreSQL harness using real `OutboxRelay`, `TenantGenerationDeliveryGate` and `ServerPagesCachePort`.
2. Retain publish receipt/event commit followed by production all-scope rotation, durable acknowledgement and new-key miss/refill.
3. Retain rollback receipt/event commit followed by a post-invalidation downstream failure.
4. Create a second relay instance with a distinct worker identity and prove same-event retry does not rotate generations twice.
5. Retain durable pending/retry/dispatched state and event/correlation identity across retry.
6. Invoke the ordinary Pages listener after successful retry and retain the same-event no-op.
7. Preserve the historical owner-transaction and pre-handler restart harnesses as separate evidence.
8. Add source evidence, fail-closed verifier and a dated packet; leave production behavior unchanged.

## Boundaries

This slice does not:

- change Page Builder review, sanitizer, materialization or artifact behavior;
- alter Pages lifecycle event order or invalidation scope policy;
- alter cache namespace names, composite key shape, TTL or capacity;
- add cache scans or wildcard deletion;
- change outbox, Pages or Page Builder migrations or DTOs;
- remove the asynchronous module listener;
- provide durable exact-once invalidation across process restarts;
- mount full server bootstrap or external Iggy infrastructure;
- claim tests, Cargo, formatting, verifiers, SQLite, Axum, Leptos, PostgreSQL, browsers, workflows, CI or rollout execution;
- promote FFA or FBA status.

## Next cursor

1. Run the production-gate PostgreSQL restart verifier and focused server PostgreSQL harness.
2. Rerun the historical PostgreSQL owner-transaction and pre-handler restart verifiers/tests alongside it.
3. Run the production relay-native-route verifier and focused SQLite server integration test.
4. Run the production generation-gate verifier and focused gate/port unit tests.
5. Execute the gate under Memory and OutboxLocal profiles, then retain OutboxIggy evidence where infrastructure is available.
6. Rerun the topology-aware continuity verifier and the complete native SQLite/Axum route set.
7. Run metadata conflict/isolation and published metadata browser packets.
8. Complete compile, workflow, anonymous-bundle and observed tenant rollout evidence before promotion.

Any failure or owner-model change must update this shared cursor first, then the owning local plan, so Pages and Page Builder cannot drift into different completion claims.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-production-gate-postgres-restart.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-outbox-cache-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-outbox-relay-restart-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-production-relay-native-route.mjs
node crates/rustok-pages/scripts/verify/verify-pages-production-relay-generation-gate.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-relay-continuity.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-reviewed-artifact.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-channel-admission.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-server-fn.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-cache.mjs
node crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs
node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-surface.mjs
node crates/rustok-pages/scripts/verify/verify-pages-artifact-http-cache.mjs
cargo test -p rustok-server --features mod-pages --test pages_production_gate_postgres_restart -- --nocapture
cargo test -p rustok-server --features mod-pages --test pages_production_relay_native_route_sqlite -- --nocapture
cargo test -p rustok-server --features mod-pages services::pages_cache_invalidation -- --nocapture
cargo test -p rustok-server --features mod-pages services::tenant_generation_delivery_gate -- --nocapture
cargo test -p rustok-pages --test publish_rollback_outbox_cache_postgres -- --nocapture
cargo test -p rustok-pages --test outbox_relay_restart_postgres -- --nocapture
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_relay_continuity_sqlite -- --nocapture
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_reviewed_artifact_sqlite -- --nocapture
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_channel_admission_sqlite -- --nocapture
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_server_fn_sqlite -- --nocapture
cargo test -p rustok-pages --test native_storefront_cache_contract -- --nocapture
cargo test -p rustok-pages --test artifact_http_cache_sqlite -- --nocapture
cargo check -p rustok-server --features mod-pages --all-targets
cargo check -p rustok-pages-storefront --features ssr --all-targets
cargo check -p rustok-pages --all-targets
cargo check -p rustok-page-builder --all-targets
cargo check -p rustok-outbox --all-targets
cargo check -p rustok-channel --all-targets
```
