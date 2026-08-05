# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-05
Status: source-parity-current / native-storefront-cache-source-ready / execution-evidence-pending
Scope: `rustok-pages` admin/storefront FFA and `rustok-page-builder` consumer-property, publication and cache boundaries

## Source-of-truth policy

This is the current continuation cursor. Historical dated packets remain evidence of the source slices that produced the present state, but they do not override this plan.

`source-ready` means that code, contracts or retained harness source exists. It does not mean that tests, Cargo, formatting, verifiers, databases, HTTP routes, server functions, browsers, workflows, CI or tenant rollout were executed.

## Audit basis

The current source was rechecked against:

- `crates/rustok-pages/docs/implementation-plan.md`;
- `crates/rustok-page-builder/docs/implementation-plan.md`;
- `docs/modules/page-builder-implementation-plan.md`;
- `crates/rustok-page-builder/contracts/page-builder-consumer-properties.json`;
- the Pages admin draft/published metadata composition;
- reviewed publish and immutable rollback owner services;
- the Pages outbox/cache invalidation and read contracts;
- the native storefront adapter and public artifact HTTP route;
- recent merged Pages parity PRs and their dated packets.

The audit remains source-only. Execution evidence remains pending.

## Rechecked merged cursor

The earlier 2026-08-03 plan stopped before later source packets. Current `main` also contains:

- PR #2955 — publish/rollback event-correlation and generation miss/refill contract;
- PR #2971 — source-ready PostgreSQL publish/rollback outbox-to-cache packet;
- PR #2974 — source-ready durable relay failure/restart packet;
- PR #2979 — source-ready SQLite/Axum public artifact HTTP cache packet.

The current slice adds the native storefront cache source packet over the same public Pages cache runtime and composite route/page/artifact key.

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

### Outbox and cache correlation: source-ready

Publish and rollback write durable lifecycle events in the owner transaction and never call cache infrastructure inline. The Pages handler maps the committed `NodePublished` envelope to an event/correlation-bound invalidation request and validates positive route/page/artifact generations before acknowledging delivery.

The retained PostgreSQL harness covers publish/rollback receipt and outbox atomicity at source level. The relay restart harness covers a failed first delivery, durable retry state, a distinct worker identity and later acknowledgement only after the real Pages cache handler succeeds.

PostgreSQL and relay execution remain pending.

### Public artifact HTTP cache: source-ready

The public artifact route has a retained SQLite/Axum harness using real Pages migrations, a valid deterministic Page Builder artifact, `HostRuntimeContext`, the typed cache runtime and the public router.

The source packet retains generation-7 miss/refill/hit, generation-8 key rotation, old-value physical retention, conditional `304`, empty conditional bodies and the production security/header contract.

SQLite/Axum execution remains pending.

### Native storefront composite cache: source-ready

The production native adapter already retains this order:

1. trusted tenant/locale/channel resolution;
2. channel module admission;
3. bounded route/locale/fallback/channel variant construction;
4. route/page/artifact generation snapshot;
5. Pages-owned composite key;
6. typed cache hit before owner reads;
7. published page and verified immutable artifact owner reads on miss;
8. public page-list owner read;
9. best-effort cache fill after the complete owner response.

Generation-read, cache-read and cache-fill failures fail open to authoritative source reads.

The current source packet adds a bounded harness using `PagesCacheReadRuntime`, `PagesCacheReadPort`, `PageCacheGenerationSnapshot` and `storefront_pages_cache_key`. It retains initial miss/refill, same-generation hit, all-generation rotation, old-generation value retention, cache-read failure fallback and generation-read failure bypass.

The real native server function was not executed. Database, Leptos/Axum and browser observations remain open.

## Parity matrix

| Capability | Pages owner | Page Builder owner | Source state | Execution state |
| --- | --- | --- | --- | --- |
| Metadata schema/values | Pages | Contract/runtime validation | Complete | Conflict/isolation and browser packets pending |
| Draft registered metadata | Pages host/runtime | Canonical panel | Complete | Browser execution pending |
| Published registered metadata | Pages standalone host | Canonical panel | Complete | Browser execution pending |
| Legacy metadata editor | None | None | Removed | Not applicable |
| Reviewed publish | Pages lifecycle/artifacts/outbox | Review/sanitization/materialization contracts | Complete | Database/runtime evidence pending |
| Immutable rollback | Pages lifecycle/artifacts/outbox | No lifecycle ownership | Complete | Database/runtime evidence pending |
| Outbox → generation rotation | Pages event/cache owners | No cache ownership | Source-ready | PostgreSQL/relay execution pending |
| Artifact HTTP miss/refill/hit | Pages route/artifact owner | Immutable artifact verification | Source-ready | SQLite/Axum and PostgreSQL HTTP pending |
| Native storefront miss/refill/hit | Pages native storefront/cache owners | Published artifact contract | Source-ready | Real server-function execution pending |
| Fly document mutation | Pages builder facade | Fly/Page Builder | Draft-only | Browser/runtime evidence pending |
| Published Fly authoring | Not allowed | Not mounted | Correctly blocked | Bundle/runtime proof pending |

## Changes in this slice

1. Recheck merged Pages/Page Builder parity through PR #2979.
2. Correct stale broad-plan wording for typed metadata contributions and immutable rollback through a dated Page Builder actualization overlay.
3. Add `crates/rustok-pages/tests/native_storefront_cache_contract.rs` over the public cache runtime and key contract.
4. Retain initial miss/refill, hit short-circuit, all-generation rotation and old-key retention.
5. Retain cache-read and generation-read fail-open source behavior.
6. Add machine evidence with an empty execution list and all validation flags false.
7. Add a focused static verifier over production native ordering, the public cache contract, the harness and the current plans.
8. Add the dated native storefront cache packet without changing production behavior.

## Boundaries

This slice does not:

- change Pages or Page Builder production behavior;
- alter cache namespaces, generations, key shape, TTL or failure policy;
- add Redis ownership, wildcard scans, key deletion or another cache provider;
- change migrations, entities, DTOs, GraphQL, HTTP or server-function routes;
- claim a real database, native server-function, HTTP, browser, workflow, CI or rollout execution;
- promote FFA or FBA status.

## Next cursor

1. Run and retain the metadata conflict/dirty-Fly isolation packet.
2. Run and retain the published metadata browser packet using the stable registered-surface DOM contract.
3. Mount and execute the real Pages native storefront server-function route with trusted `HostRuntimeContext`, tenant/request context, real Pages database fixtures and `PagesCacheReadRuntime`.
4. Retain one exact-revision continuity packet from durable `NodePublished` relay delivery through generation rotation to native storefront miss/refill/hit.
5. Execute the existing artifact HTTP, PostgreSQL outbox/cache and relay-restart packets.
6. Complete compile, workflow, anonymous-bundle and observed tenant rollout evidence before promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs
node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-surface.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-outbox-cache-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-outbox-relay-restart-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-artifact-http-cache.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-cache.mjs
cargo test -p rustok-pages --test native_storefront_cache_contract -- --nocapture
cargo test -p rustok-pages --test artifact_http_cache_sqlite -- --nocapture
cargo check -p rustok-pages --all-targets
cargo check -p rustok-pages-storefront --features ssr --all-targets
```
