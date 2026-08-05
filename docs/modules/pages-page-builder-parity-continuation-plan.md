# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-05
Status: source-parity-current / native-storefront-channel-admission-source-ready / execution-evidence-pending
Scope: `rustok-pages` admin/storefront FFA and `rustok-page-builder` consumer-property, publication, artifact and cache boundaries

## Source-of-truth policy

This is the current continuation cursor. Historical dated packets remain evidence of the source slices that produced the present state, but they do not override this plan.

`source-ready` means that code, contracts or retained harness source exists. It does not mean that tests, Cargo, formatting, verifiers, databases, HTTP routes, server functions, browsers, workflows, CI or tenant rollout were executed.

## Audit basis

The current source was rechecked against:

- `crates/rustok-pages/docs/implementation-plan.md`;
- `crates/rustok-page-builder/docs/implementation-plan.md`;
- `docs/modules/page-builder-implementation-plan.md`;
- the Pages admin draft/published metadata composition;
- reviewed publish and immutable rollback owner services;
- the Pages outbox/cache invalidation and read contracts;
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
- PR #2988 — source-ready registered Leptos storefront route with real Pages owner data and cache behavior.

The current slice adds the routed-channel module-admission packet on that same registered route.

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

The retained PostgreSQL harness covers publish/rollback receipt and outbox atomicity at source level. The relay restart harness covers failed first delivery, durable retry state, a distinct worker identity and later acknowledgement only after the real Pages cache handler succeeds.

PostgreSQL and relay execution remain pending.

### Public artifact HTTP cache: source-ready

The public artifact route has a retained SQLite/Axum harness using real Pages migrations, a valid deterministic Page Builder artifact, `HostRuntimeContext`, the typed cache runtime and the public router.

The source packet retains generation-7 miss/refill/hit, generation-8 key rotation, old-value physical retention, conditional `304`, empty conditional bodies and the production security/header contract.

SQLite/Axum execution remains pending.

### Native storefront composite cache: source-ready

The public cache-contract harness retains initial miss/refill, same-generation hit, all-generation rotation, old-generation value retention, cache-read failure fallback and generation-read failure bypass using `PagesCacheReadRuntime`, `PagesCacheReadPort`, `PageCacheGenerationSnapshot` and `storefront_pages_cache_key`.

That packet does not claim a real Leptos route execution.

### Native storefront registered server function: source-ready

The retained SQLite/Axum harness mounts the real registered endpoint through the same server-host shape:

```text
POST /api/fn/{*fn_name}
  -> handle_server_fns_with_context
  -> provide_context(HostRuntimeContext)
  -> pages/storefront-data
```

It applies the real Outbox and Pages migrations, creates and publishes a localized owner page through `PageService`, attaches the typed cache runtime and trusted tenant context, and retains route-level miss/refill, same-generation hit, generation rotation and both fail-open paths.

The fixture uses a published non-builder HTML body to isolate route, owner and composite-cache behavior. The registered route packet remains unexecuted.

### Routed-channel admission before native lookup: source-ready

A separate SQLite/Axum harness now adds trusted `ChannelContextExtension`, applies real Channel migrations and manages the routed channel plus Pages binding through `ChannelService`.

The retained sequence is:

1. explicit `pages=false` binding rejects `/api/fn/pages/storefront-data`;
2. rejection occurs before generation snapshot, cache get or cache put;
3. `pages=true` through the same Channel owner allows the route and produces a normal miss/refill;
4. `pages=false` is restored while the cached value remains present;
5. the next request is rejected with no additional generation read, cache get or cache put.

Therefore a populated composite cache cannot bypass channel module admission. The production absent-binding compatibility policy remains unchanged: no binding defaults to enabled.

The channel-admission harness and verifier remain unexecuted. The native Page Builder immutable-artifact branch is still the next source cursor.

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
| Routed-channel admission before native lookup | Pages/Channel owners | No admission ownership | Source-ready | SQLite/Axum route execution pending |
| Native immutable artifact selection | Pages artifact binding owner | Verified immutable artifact contract | Production source-connected | Registered route fixture pending |
| Fly document mutation | Pages builder facade | Fly/Page Builder | Draft-only | Browser/runtime evidence pending |
| Published Fly authoring | Not allowed | Not mounted | Correctly blocked | Bundle/runtime proof pending |

## Changes in this slice

1. Recheck the cursor after merged PR #2988.
2. Add a feature-gated registered-route harness with trusted tenant and channel extensions.
3. Apply real Outbox, Channel and Pages migrations.
4. Create the channel and explicit Pages module binding through `ChannelService`.
5. Retain disabled-before-generation/cache ordering.
6. Retain enabled miss/refill with the production storefront TTL.
7. Retain a second denial while the prior cached value remains present and unreachable.
8. Add machine evidence with an empty execution list and all validation flags false.
9. Add a focused verifier and dated packet without changing production behavior.

## Boundaries

This slice does not:

- change Pages, Page Builder, Channel or cache production behavior;
- alter the Channel absent-binding compatibility policy;
- alter cache namespaces, generations, key shape, TTL or failure policy;
- add Redis ownership, wildcard scans, key deletion or another cache provider;
- change production migrations, entities, DTOs, GraphQL, HTTP or server-function routes/codecs;
- claim real SQLite, Axum, server-function, browser, workflow, CI or rollout execution;
- execute a verified immutable Page Builder artifact through the registered native route;
- promote FFA or FBA status.

## Next cursor

1. Add a registered native-route fixture using a verified immutable Page Builder artifact and channel-constrained owner selection.
2. Retain one exact-revision continuity packet from durable `NodePublished` relay delivery through generation rotation to an admitted native storefront miss/refill.
3. Run and retain the registered native storefront SQLite/Axum packets, including channel admission.
4. Run and retain the metadata conflict/dirty-Fly isolation packet.
5. Run and retain the published metadata browser packet using the stable registered-surface DOM contract.
6. Execute the existing artifact HTTP, PostgreSQL outbox/cache and relay-restart packets.
7. Complete compile, workflow, anonymous-bundle and observed tenant rollout evidence before promotion.

A durable `NodePublished` relay delivery is not yet connected to the registered native request in one process or retained revision.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-channel-admission.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-server-fn.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-cache.mjs
node crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs
node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-surface.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-outbox-cache-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-outbox-relay-restart-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-artifact-http-cache.mjs
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_channel_admission_sqlite -- --nocapture
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_server_fn_sqlite -- --nocapture
cargo test -p rustok-pages --test native_storefront_cache_contract -- --nocapture
cargo test -p rustok-pages --test artifact_http_cache_sqlite -- --nocapture
cargo check -p rustok-pages-storefront --features ssr --all-targets
cargo check -p rustok-channel --all-targets
cargo check -p rustok-pages --all-targets
```
