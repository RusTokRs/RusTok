# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-05
Status: source-parity-current / native-storefront-relay-topology-corrected / production-listener-ack-gap-open / execution-evidence-pending
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
- the server `EventRuntime` relay target and listener-bus composition;
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
- PR #2995 — source-ready owner/outbox/handler/native-route continuity through a synchronous test relay target.

The present correction does not remove PR #2995 evidence. It narrows the claim to the topology actually mounted by that harness.

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

The same shared test port supplies generation mutation, cache reads and cache writes. The old composite key remains physically retained after `NodePublished`, while the registered route derives only the new key. Both successful responses point to the same reviewed immutable artifact.

This proves owner, relay, handler and route contracts in one process. It does not mount the production server relay target or module dispatcher.

### Production relay-to-Pages-listener acknowledgement: open

The production outbox profile currently composes:

```text
OutboxRelay
  → configured server relay target
  → local/remote transport acceptance
  → listener_bus
  → asynchronous module EventDispatcher
  → PageCacheInvalidationEventHandler
```

`OutboxRelay` marks a row dispatched after its configured target succeeds. The module dispatcher later filters handlers with `EventHandler::handles` and runs matching handlers asynchronously.

The retained continuity harness bypasses that production listener topology by using a custom synchronous relay target that calls the Pages handler directly. Consequently:

- test-target acknowledgement after handler success is source-ready;
- production transport acceptance before `sys_events.dispatched` is source-ready;
- production Pages listener completion before `sys_events.dispatched` is not proven;
- listener failure or process crash after transport acceptance remains a durable-consistency gap for this claim.

Do not promote the continuity packet as production-topology evidence.

The topology correction is retained in:

- `docs/modules/pages-page-builder-native-storefront-relay-topology-correction-2026-08-05.md`;
- the v2 continuity evidence;
- the topology-aware continuity verifier.

## Recommended owner model

Use a synchronous idempotent Pages invalidation gate in the production relay target.

The gate should:

1. recognize Pages lifecycle events through the real handler predicate;
2. serialize work by stable event UUID;
3. skip only an already-successful invalidation for the same UUID;
4. run the real Pages invalidation runtime before downstream transport acceptance;
5. commit dedupe state only after invalidation succeeds;
6. let relay retry when invalidation fails;
7. preserve downstream event delivery for other module listeners;
8. disable or deduplicate the existing Pages module listener under outbox profiles;
9. retain listener ownership for memory-only delivery.

A durable listener-receipt protocol is an acceptable alternative, but it is broader and should not be introduced unless the synchronous gate cannot satisfy deployment requirements.

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
| Production relay acknowledgement after Pages invalidation | Server relay target / Pages invalidation owner | No delivery ownership | Open | Not executable yet |
| Fly document mutation | Pages builder facade | Fly/Page Builder | Draft-only | Browser/runtime evidence pending |
| Published Fly authoring | Not allowed | Not mounted | Correctly blocked | Bundle/runtime proof pending |

## Changes in this correction slice

1. Recheck PR #2995 against the actual server relay and module-listener topology.
2. Retain the real owner/relay/handler/native-route source evidence.
3. Record that `ContinuityTarget` is a synchronous test target.
4. Record that the production server relay target and module dispatcher are not mounted by the harness.
5. Remove the unsupported production acknowledgement claim.
6. Make the continuity verifier inspect `event_transport_factory`, `module_event_dispatcher` and the core dispatcher predicate/async path.
7. Add a dated topology correction packet.
8. Normalize the duplicate SSR guard on `published_artifact_page_body` without changing behavior.
9. Reopen the production relay-to-Pages-listener acknowledgement cursor.

## Boundaries

This correction slice does not:

- change production Pages, Page Builder, Outbox, Channel, cache or event delivery behavior;
- add the recommended synchronous production gate;
- add durable module-listener receipts;
- alter reviewed publish, sanitizer, materialization, binding, receipt or rollback contracts;
- alter event scopes, cache namespaces, generations, key shape, TTL or failure policy;
- add cache scans, wildcard deletion or another provider;
- change production migrations, entities, DTOs, GraphQL, HTTP or server-function codecs;
- claim SQLite, Axum, server-function, PostgreSQL, browser, workflow, CI or rollout execution;
- promote FFA or FBA status.

## Next cursor

1. Implement the synchronous idempotent Pages invalidation gate around the production outbox relay target.
2. Prevent duplicate Pages generation rotation by the asynchronous module listener under outbox profiles while preserving memory-profile listener behavior.
3. Add a source harness that uses the production gate and proves downstream failure/retry does not rotate twice.
4. Connect the production gate to the registered native route generation state in one retained revision.
5. Then run the topology-aware continuity verifier and native SQLite/Axum route set.
6. Run metadata conflict/isolation and published metadata browser packets.
7. Run artifact HTTP and PostgreSQL publish/rollback plus relay-restart packets.
8. Complete compile, workflow, anonymous-bundle and observed tenant rollout evidence before promotion.

Any failure or owner-model change must update this shared cursor first, then the owning local plan, so Pages and Page Builder cannot drift into different completion claims.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
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
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_relay_continuity_sqlite -- --nocapture
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_reviewed_artifact_sqlite -- --nocapture
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_channel_admission_sqlite -- --nocapture
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_server_fn_sqlite -- --nocapture
cargo test -p rustok-pages --test native_storefront_cache_contract -- --nocapture
cargo test -p rustok-pages --test artifact_http_cache_sqlite -- --nocapture
cargo check -p rustok-pages-storefront --features ssr --all-targets
cargo check -p rustok-pages --all-targets
cargo check -p rustok-page-builder --all-targets
cargo check -p rustok-outbox --all-targets
cargo check -p rustok-channel --all-targets
```
