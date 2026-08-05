# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-05
Status: source-parity-current / public-list-locale-fallback-source-ready / execution-evidence-pending
Scope: `rustok-pages` admin/storefront FFA and `rustok-page-builder` consumer-property, publication, artifact, event and cache boundaries

## Source-of-truth policy

This is the canonical shared continuation cursor. Historical dated packets remain evidence of the source slices that produced the present state, but they do not override this plan.

`source-ready` means that code, contracts or retained harness source exists. It does not mean that tests, Cargo, formatting, verifiers, databases, HTTP routes, server functions, production event topology, browsers, workflows, CI, built artifacts or tenant rollout were executed.

Across every retained source packet, execution remains pending until a maintainer records reproducible command output and artifact evidence.

Pages and Page Builder continue as one vertical pipeline with explicit owners. Pages owns persistence, lifecycle, immutable bindings, routing, cache policy and public reads. Page Builder/Fly owns the reviewed document, sanitizer, runtime materialization, renderer and artifact producer contracts.

Optional external event infrastructure is outside the active Pages cursor.

## Rechecked merged cursor

Current `main` contains:

- PR #2955 — publish/rollback event-correlation and generation miss/refill contract;
- PR #2971 — source-ready PostgreSQL publish/rollback outbox-to-cache packet;
- PR #2974 — source-ready durable relay failure/restart packet;
- PR #2979 — source-ready SQLite/Axum public artifact HTTP cache packet;
- PR #2985 — native storefront cache source packet; execution evidence remains pending;
- PR #2988 — source-ready registered Leptos storefront route;
- PR #2990 — source-ready routed-channel admission before cache lookup;
- PR #2992 — source-ready reviewed immutable artifact selection;
- PR #2995 — source-ready synchronous test-target relay continuity;
- PR #2997 — production-listener topology correction;
- PR #3001 — production synchronous Pages generation gate and process-bounded dedupe;
- PR #3004 — production gate to registered native route source;
- PR #3006 — production-gate PostgreSQL retry source;
- PR #3008 — Memory and OutboxLocal factory profile parity source;
- PR #3010 — selected immutable artifact authority after draft mutation;
- PR #3011 — anonymous storefront dependency-graph source boundary;
- PR #3014 — anonymous SSR delivery boundary and explicit artifact inspector source.

The current slice corrects a Pages public-read inconsistency: selected detail and list results now use the same requested-locale, tenant-default-locale and platform-fallback chain in both native and GraphQL public transports.

## Current parity state

### Registered metadata surfaces: source-complete

Published pages mount the same registered panel without an editable Fly canvas. The bespoke `PageMetadataEditor` and its direct workspace metadata transport write are removed.

Focused stale-revision and dirty-Fly isolation regressions are source-ready. Their execution and the published browser packet remain open.

Metadata revision/isolation source packet: ready, unvalidated. A stale metadata revision short-circuits before patch transport; the metadata-only transport request excludes document data; dirty Fly state is not accepted by the metadata owner port. Execution evidence remains pending. Verifier: `verify-pages-metadata-revision-isolation.mjs`.

### Reviewed publication: source-complete

Pages owns the reviewed publish transaction from exact metadata/body revisions and promoted scenario review through authoritative sanitization, runtime materialization, immutable artifact persistence/binding, published state, transactional `NodeUpdated`/`NodePublished` events and the durable publish receipt plus exact artifact manifest.

### Immutable rollback: source-complete

Pages owns the idempotent rollback command and receipt. Rollback verifies and selects a prior immutable publish manifest, replaces locale bindings and commits lifecycle events plus its receipt without compiling the current draft.

### Public artifact HTTP cache: source-ready

The retained route packet covers generation-bound miss/refill/hit, conditional `304`, immutable verification before fill and old-generation physical retention. Execution remains pending.

### Native storefront registered route set: source-ready

The route set covers cache miss/refill/hit, registered `/api/fn/pages/storefront-data`, channel admission before cache, reviewed immutable selection, integrity-before-fill and old-key retention.

Native storefront registered server function: source-ready; the real registered Leptos endpoint is retained. Routed-channel module admission remains open for execution, and durable `NodePublished` relay delivery is now connected at source level.

### Public list tenant locale fallback: source-ready

`public-list-locale-fallback-source-ready`; Public list tenant locale fallback: source-ready.

Pages now exposes a fallback-aware public list owner method. It normalizes the requested and explicit tenant fallback locales and resolves each list translation through:

```text
requested locale
  → tenant default locale
  → platform fallback locale
```

The existing `list_public_visible` method remains as a platform-fallback wrapper for callers that do not supply tenant policy.

The native and GraphQL public detail/list reads now pass the same tenant default locale to the owner. The native cache variant already binds the fallback locale, so this behavior correction does not change namespace, generation, concrete key shape, TTL or capacity. Published-only and channel-visibility filtering remain unchanged.

Source evidence:

- `crates/rustok-pages/src/services/page/read.rs`;
- `crates/rustok-pages/src/graphql/query.rs`;
- `crates/rustok-pages/storefront/src/transport/native_server_adapter.rs`;
- `crates/rustok-pages/tests/page_locale_fallback.rs`;
- `crates/rustok-pages/contracts/evidence/pages-public-list-locale-fallback-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-public-list-locale-fallback.mjs`;
- `docs/modules/pages-page-builder-public-list-locale-fallback-packet-2026-08-05.md`.

Execution evidence remains pending.

### Native reviewed immutable artifact selection: source-ready

`native-storefront-reviewed-artifact-source-ready`; Native reviewed immutable artifact selection: source-ready. Verification reconstructs the full Page Builder materialization envelope before a registered native storefront miss/refill. Durable `NodePublished` delivery remains connected at source level.

### Routed-channel admission before native lookup: source-ready

`native-storefront-channel-admission-source-ready`; Routed-channel admission before native lookup: source-ready. A populated composite cache cannot bypass channel module admission, and successful reads retain a verified immutable Page Builder artifact.

### Selected immutable artifact after draft mutation: source-ready

`selected-immutable-artifact-source-ready`; Selected immutable artifact after draft mutation: source-ready. The current Fly body is not public render authority. Exact and fallback public reads remain bound to the selected immutable published artifact until reviewed publish or rollback replaces the binding.

### Reviewed publish to native refill through synchronous test target: source-ready

The retained PR #2995 harness uses a custom synchronous relay target and proves owner/outbox/handler/registered-route continuity. It is a test-target packet and does not replace production-gate execution evidence.

### Production relay-to-Pages generation gate: source-ready

`production-relay-generation-gate-source-ready`; Production relay-to-Pages generation gate: source-ready. Synchronous Pages invalidation now precedes downstream transport acceptance and uses process-bounded dedupe. The asynchronous module listener remains registered and becomes a same-event rotation no-op.

The handler request and receipt retain event/correlation-bound receipt identity. Cache rotations retain old-generation values physically while current generation keys move to miss/refill.

### Production relay gate to registered native route: source-ready

`production-relay-native-route-source-ready`; Production relay gate to registered native route: source-ready. The source retains new-key miss/refill/hit after `NodePublished`, with one production `CacheService` owning generations and bytes. Execution remains pending.

### Production gate PostgreSQL publish/rollback restart: source-ready

`production-gate-postgres-restart-source-ready`; Production gate PostgreSQL publish/rollback restart: source-ready. A post-invalidation downstream failure leaves the durable row pending. Process-bounded dedupe prevents a second rotation when a new relay instance retries the same event in one process. The historical owner-transaction and pre-handler restart packets remain separate.

### Memory and OutboxLocal factory profile parity: source-ready

`event-delivery-profile-parity-source-ready`; Memory and OutboxLocal factory profile parity: source-ready. Memory rotates before listener delivery without a durable row. OutboxLocal writes a pending row first and rotates inside the real relay target before acknowledgement. Optional external delivery infrastructure is outside the active Pages cursor.

### Anonymous storefront authoring exclusion: source-ready

`anonymous-storefront-graph-source-ready`; Anonymous storefront authoring exclusion: source-ready.

The retained verifier resolves six feature-resolved `cargo metadata` graphs and follows only normal/build edges; dev-dependencies are excluded. It forbids `rustok-pages-admin`, `rustok-page-builder-admin`, `rustok-admin`, `fly-browser`, `fly-ui` and `fly-leptos` at every reachable depth.

The current host client profiles keep the optional Pages module disabled. The direct Pages hydrate graph remains a library capability check. Compiled bundle artifact execution remains pending.

### Anonymous storefront SSR delivery: source-ready

`anonymous-storefront-ssr-delivery-source-ready`; Anonymous storefront SSR delivery: source-ready.

The current public Pages host is SSR-only:

```text
anonymous request
  → apps/storefront SSR router
  → Leptos render-to-HTML
  → Pages read-only storefront composition
  → document + /assets/app.css
  → no executable client bootstrap
```

The source regression rejects module scripts, module preload, WASM URLs, hydration entrypoints and Pages/Page Builder/Fly authoring markers in the rendered public document source.

The artifact inspector requires an explicit built SSR artifact, reruns the feature-resolved dependency-graph verifier, records SHA-256 and fails on authoring markers. Missing artifacts cannot pass.

The client bundle gate is conditional: it reopens immediately when host CSR/hydrate enables Pages, a client bootstrap is introduced, or deployable Pages WASM/JS artifacts begin shipping. No client bundle proof is claimed for a bundle that does not currently exist.

Source evidence:

- `apps/storefront/tests/pages_anonymous_ssr_delivery.rs`;
- `crates/rustok-pages/contracts/evidence/pages-anonymous-storefront-ssr-delivery-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-ssr-delivery.mjs`;
- `crates/rustok-pages/scripts/verify/inspect-pages-anonymous-storefront-ssr-artifact.mjs`;
- `docs/modules/pages-page-builder-anonymous-storefront-ssr-delivery-packet-2026-08-05.md`.

Execution evidence remains pending.

## Parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Metadata schema and owner port | Complete | Conflict/isolation and browser packets pending |
| Draft/published registered metadata | Complete | Browser execution pending |
| Reviewed publish and immutable manifest | Complete | Database/runtime evidence pending |
| Immutable rollback | Complete | Database/runtime evidence pending |
| Public detail/list tenant locale fallback parity | Source-ready | Focused SQLite/native/GraphQL execution pending |
| Artifact HTTP cache | Source-ready | SQLite/Axum execution pending |
| Native storefront route/cache/admission | Source-ready | Route-set execution pending |
| Selected immutable artifact vs draft body | Source-ready | Focused SQLite execution pending |
| Production generation gate and native route | Source-ready | Server execution pending |
| PostgreSQL retry after post-invalidation failure | Source-ready | PostgreSQL execution pending |
| Memory and OutboxLocal factory profiles | Source-ready | SQLite profile execution pending |
| Anonymous dependency graph | Source-ready | `cargo metadata` execution pending |
| Anonymous SSR document boundary | Source-ready | Source regression pending |
| Anonymous SSR built artifact | Inspector source-ready | Build and inspection pending |
| Anonymous Pages client bundle | Not currently mounted by host | Gate reopens if introduced |
| Authenticated real-DOM inline editing | Not implemented | Open |

## Boundaries

This slice changes production Pages public-list translation selection in the owner service, registered native storefront and unauthenticated GraphQL list.

It does not:

- change Page Builder or Fly behavior;
- change persistence, migrations, schemas, DTOs, GraphQL schema, artifacts or bindings;
- change public routes, canonical URL policy, redirects or route aliases;
- change channel visibility or module-admission policy;
- change cache namespaces, generation scopes, concrete key shape, TTL or capacity;
- change event delivery or optional external event infrastructure;
- claim tests, Cargo, formatting, verifiers, SQLite, native server functions, GraphQL, browsers, workflows, CI or rollout execution;
- promote FFA or FBA.

## Next cursor

1. Run the public list locale fallback verifier and focused Pages locale regression.
2. Run the native cache, registered server-function and channel-admission guards with their route harnesses.
3. Run the anonymous dependency-graph and SSR delivery packets plus explicit built-artifact inspection.
4. Run the selected immutable artifact and complete native SQLite/Axum route set.
5. Run production generation-gate, native-route and PostgreSQL retry packets.
6. Run metadata conflict/isolation and published metadata browser packets.
7. Complete canonical URLs, redirects and route-collision policy as a separate Pages routing slice.
8. Complete workflow and observed tenant rollout evidence before promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-public-list-locale-fallback.mjs
cargo test -p rustok-pages --test page_locale_fallback -- --nocapture

node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-cache.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-server-fn.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-channel-admission.mjs

node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-ssr-delivery.mjs

cargo test -p rustok-storefront --no-default-features --features ssr \
  --test pages_anonymous_ssr_delivery -- --nocapture

CARGO_TARGET_DIR=target/pages-anonymous-storefront-ssr \
  cargo build -p rustok-storefront --no-default-features --features ssr --lib

node crates/rustok-pages/scripts/verify/inspect-pages-anonymous-storefront-ssr-artifact.mjs \
  --profile host-storefront-ssr \
  --artifact target/pages-anonymous-storefront-ssr/debug/deps/librustok_storefront-<hash>.rlib \
  --output /tmp/pages-anonymous-storefront-ssr-artifact.json

node crates/rustok-pages/scripts/verify/verify-pages-selected-immutable-artifact.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-reviewed-artifact.mjs
node crates/rustok-pages/scripts/verify/verify-pages-production-relay-generation-gate.mjs
node crates/rustok-pages/scripts/verify/verify-pages-production-relay-native-route.mjs
node crates/rustok-pages/scripts/verify/verify-pages-production-gate-postgres-restart.mjs
node crates/rustok-pages/scripts/verify/verify-pages-event-delivery-profile-parity.mjs
node crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs
node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-surface.mjs
```

Any failure or owner-model change must update this shared cursor before FFA/FBA promotion.
