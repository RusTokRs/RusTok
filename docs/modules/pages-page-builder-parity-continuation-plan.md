# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-05
Status: source-parity-current / production-relay-generation-gate-source-ready / execution-evidence-pending
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
- the module `EventDispatcher` filtering and asynchronous handler execution;
- the native storefront adapter, server host registration and public artifact HTTP route;
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
- PR #2997 — topology correction separating the synchronous test target from production asynchronous listener delivery.

The current slice implements the recommended production gate without changing Page Builder ownership.

## Corrected parity state

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

The production delivery chain now retains:

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

Source evidence is retained in:

- `apps/server/src/services/pages_cache_invalidation.rs`;
- `apps/server/src/services/tenant_generation_delivery_gate.rs`;
- `crates/rustok-pages/contracts/evidence/pages-production-relay-generation-gate-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-production-relay-generation-gate.mjs`;
- `docs/modules/pages-page-builder-production-relay-generation-gate-packet-2026-08-05.md`.

Production-gate tests, Cargo and runtime profiles remain unexecuted.

## Parity matrix

| Capability | Pages owner | Page Builder owner | Source state | Execution state |
| --- | --- | --- | --- | --- |
| Metadata schema/values | Pages | Contract/runtime validation | Complete | Conflict/isolation and browser packets pending |
| Draft registered metadata | Pages host/runtime | Canonical panel | Complete | Browser execution pending |
| Published registered metadata | Pages standalone host | Canonical panel | Complete | Browser execution pending |
| Legacy metadata editor | None | None | Removed | Not applicable |
| Reviewed publish | Pages lifecycle/artifacts/outbox | Review/sanitization/materialization contracts | Complete | Database/runtime evidence pending |
| Immutable rollback | Pages lifecycle/artifacts/outbox | No lifecycle ownership | Complete | Database/runtime evidence pending |
| Artifact HTTP miss/refill/hit | Pages route/artifact owner | Immutable artifact verification | Source-ready | SQLite/Axum and PostgreSQL HTTP pending |
| Native storefront route/cache/admission | Pages/Channel owners | Published artifact contract | Source-ready | SQLite/Axum route set pending |
| Native reviewed immutable artifact selection | Pages publish/binding/route/cache owners | Review/sanitization/materialization/integrity | Source-ready | SQLite/Axum reviewed route pending |
| Relay + handler + native refill via test target | Pages publish/outbox/handler/route/cache owners | Reviewed artifact producer contract | Source-ready, topology-corrected | SQLite test-target execution pending |
| Production relay acknowledgement after Pages invalidation | Server delivery gate / Pages invalidation owner | No delivery ownership | Source-ready | Server unit/profile and relay-route execution pending |
| Fly document mutation | Pages builder facade | Fly/Page Builder | Draft-only | Browser/runtime evidence pending |
| Published Fly authoring | Not allowed | Not mounted | Correctly blocked | Bundle/runtime proof pending |

## Changes in this slice

1. Extend the production `TenantGenerationDeliveryGate` with the real Pages handler predicate and runtime under `mod-pages`.
2. Run Pages invalidation after canonical local-listener readiness and before downstream transport acceptance.
3. Add stable-event serialization and successful-event dedupe to `ServerPagesCachePort`.
4. Commit dedupe only after generation rotation and receipt validation succeed.
5. Preserve dedupe across separately constructed gate and module-listener providers in one process.
6. Retain downstream retry without a second rotation after a post-invalidation transport failure.
7. Keep the asynchronous Pages listener registered for every delivery profile.
8. Add focused server source tests, machine evidence, verifier and dated packet.
9. Leave Page Builder, event schemas, database schemas, cache namespaces and public routes unchanged.

## Boundaries

This slice does not:

- change Page Builder review, sanitizer, materialization or artifact behavior;
- alter Pages lifecycle event order or invalidation scope policy;
- alter cache namespace names, composite key shape, TTL or capacity;
- add cache scans or wildcard deletion;
- change outbox, Pages or Page Builder migrations or DTOs;
- remove the asynchronous module listener;
- provide durable exact-once invalidation across process restarts;
- claim tests, Cargo, formatting, verifiers, databases, HTTP routes, server functions, workflows, CI or rollout execution;
- promote FFA or FBA status.

## Next cursor

1. Run the production relay generation-gate verifier and focused server tests.
2. Execute the gate under memory and OutboxLocal profiles, then retain OutboxIggy evidence where infrastructure is available.
3. Connect the production gate to the registered native route generation state in one execution packet: reviewed publish → relay gate → new-key miss/refill/hit.
4. Rerun the topology-aware continuity verifier and native SQLite/Axum route set.
5. Execute PostgreSQL publish/rollback outbox-cache and relay-restart packets against the gate.
6. Run metadata conflict/isolation and published metadata browser packets.
7. Complete compile, workflow, anonymous-bundle and observed tenant rollout evidence before promotion.

Any failure or owner-model change must update this shared cursor first, then the owning local plan, so Pages and Page Builder cannot drift into different completion claims.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-production-relay-generation-gate.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-relay-continuity.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-reviewed-artifact.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-channel-admission.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-server-fn.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-cache.mjs
node crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs
node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-surface.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-outbox-cache-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-outbox-relay-restart-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-artifact-http-cache.mjs
cargo test -p rustok-server --features mod-pages services::pages_cache_invalidation -- --nocapture
cargo test -p rustok-server --features mod-pages services::tenant_generation_delivery_gate -- --nocapture
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
