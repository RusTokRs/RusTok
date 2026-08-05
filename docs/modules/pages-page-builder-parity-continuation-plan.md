# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-05
Status: source-parity-current / native-storefront-server-fn-source-ready / execution-evidence-pending
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
- the native storefront adapter, server host registration and public artifact HTTP route;
- recent merged Pages parity PRs and their dated packets.

The audit remains source-only. Execution evidence remains pending.

## Rechecked merged cursor

Current `main` contains:

- PR #2955 — publish/rollback event-correlation and generation miss/refill contract;
- PR #2971 — source-ready PostgreSQL publish/rollback outbox-to-cache packet;
- PR #2974 — source-ready durable relay failure/restart packet;
- PR #2979 — source-ready SQLite/Axum public artifact HTTP cache packet;
- PR #2985 — source-ready native storefront composite-cache contract and plan actualization.

The current slice moves from the abstract native cache contract to the real registered Leptos endpoint `/api/fn/pages/storefront-data` with real Pages owner data and host/request context.

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

The public cache-contract harness retains initial miss/refill, same-generation hit, all-generation rotation, old-generation value retention, cache-read failure fallback and generation-read failure bypass using `PagesCacheReadRuntime`, `PagesCacheReadPort`, `PageCacheGenerationSnapshot` and `storefront_pages_cache_key`.

That packet does not claim a real Leptos route execution.

### Native storefront registered server function: source-ready

A new SQLite/Axum harness mounts the real registered Leptos endpoint through the same server-host shape:

```text
POST /api/fn/{*fn_name}
  -> handle_server_fns_with_context
  -> provide_context(HostRuntimeContext)
  -> pages/storefront-data
```

The harness:

1. applies the real Outbox and every real Pages migration;
2. creates and publishes a localized owner page through `PageService`;
3. attaches the typed `PagesCacheReadRuntime` to `HostRuntimeContext`;
4. attaches a trusted `TenantContextExtension` and lets the production adapter derive `RequestContext`;
5. calls `/api/fn/pages/storefront-data` with the production form codec;
6. retains initial miss/refill and same-generation hit before owner refresh;
7. advances route/page/artifact generations and retains a different-key owner refresh;
8. retains cache-read failure fallback with best-effort refill;
9. retains generation-read failure bypass with no invented lookup/fill key.

The fixture uses a published non-builder HTML body to isolate the registered route, owner and composite-cache boundary. The Page Builder immutable artifact branch remains covered at source level by the artifact HTTP packet and production source ordering, but it is not executed by this new route harness.

The real registered Leptos endpoint, SQLite/Axum scenario, Cargo and verifier remain unexecuted. Routed-channel module admission remains open because this first route fixture intentionally has no channel context.

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
| Native storefront cache contract | Pages native storefront/cache owners | Published artifact contract | Source-ready | Contract harness execution pending |
| Registered native storefront route | Pages server-function/owner/cache composition | Published rendering contract | Source-ready | SQLite/Axum server-function execution pending |
| Routed-channel admission before native lookup | Pages/channel owners | No admission ownership | Production source-connected | Real route execution pending |
| Fly document mutation | Pages builder facade | Fly/Page Builder | Draft-only | Browser/runtime evidence pending |
| Published Fly authoring | Not allowed | Not mounted | Correctly blocked | Bundle/runtime proof pending |

## Changes in this slice

1. Recheck the cursor after merged PR #2985.
2. Add a feature-gated integration harness in `rustok-pages-storefront` for the registered server-function route.
3. Apply real Outbox and Pages migrations and use real Pages create/publish owner methods.
4. Mount `handle_server_fns_with_context` with `HostRuntimeContext` and trusted tenant request extension.
5. Retain route-level miss/refill, hit-before-owner-refresh and all-generation rotation.
6. Retain route-level fail-open behavior for cache-read and generation-read failures.
7. Add machine evidence with an empty execution list and all validation flags false.
8. Add a focused verifier and dated continuation packet without changing production behavior.

## Boundaries

This slice does not:

- change Pages or Page Builder production behavior;
- alter cache namespaces, generations, key shape, TTL or failure policy;
- add Redis ownership, wildcard scans, key deletion or another cache provider;
- change migrations, entities, DTOs, GraphQL, HTTP or server-function routes/codecs;
- claim a real SQLite, Axum, server-function, browser, workflow, CI or rollout execution;
- execute channel/module admission or a native Page Builder artifact response;
- promote FFA or FBA status.

## Next cursor

1. Run and retain the registered native storefront SQLite/Axum packet.
2. Run and retain the metadata conflict/dirty-Fly isolation packet.
3. Run and retain the published metadata browser packet using the stable registered-surface DOM contract.
4. Add a routed-channel native server-function packet proving module admission precedes lookup.
5. Add a registered native-route fixture using a verified immutable Page Builder artifact.
6. Retain one exact-revision continuity packet from durable `NodePublished` relay delivery through generation rotation to native storefront miss/refill/hit.
7. Execute the existing artifact HTTP, PostgreSQL outbox/cache and relay-restart packets.
8. Complete compile, workflow, anonymous-bundle and observed tenant rollout evidence before promotion.

A durable `NodePublished` relay delivery is not yet connected to the registered native request in one process or retained revision.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-server-fn.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-cache.mjs
node crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs
node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-surface.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-outbox-cache-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-outbox-relay-restart-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-artifact-http-cache.mjs
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_server_fn_sqlite -- --nocapture
cargo test -p rustok-pages --test native_storefront_cache_contract -- --nocapture
cargo test -p rustok-pages --test artifact_http_cache_sqlite -- --nocapture
cargo check -p rustok-pages-storefront --features ssr --all-targets
cargo check -p rustok-pages --all-targets
```
