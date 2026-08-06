# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-06  
Status: source-parity-current / storefront-composition-etag-source-ready / execution-evidence-pending
Scope: `rustok-pages` admin/storefront FFA and `rustok-page-builder` consumer-property, publication, artifact, event, routing and cache boundaries

## Source-of-truth policy

This is the canonical shared continuation cursor. Historical dated packets remain evidence for the source slices that produced the present state, but they do not override this plan.

`source-ready` means that code, contracts or retained harness source exists. It does not mean that tests, Cargo, formatting, verifiers, databases, HTTP routes, server functions, production event topology, browsers, workflows, CI, built artifacts or tenant rollout were executed.

Across every retained source packet, execution remains pending until a maintainer records reproducible command output and artifact evidence.

Pages and Page Builder continue as one vertical pipeline with explicit owners. Pages owns persistence, lifecycle, immutable bindings, localized route identity, cache policy and public reads. Page Builder/Fly owns the reviewed document, sanitizer, runtime materialization, renderer and artifact producer contracts. Navigation owns menu identity and active-menu policy. SEO providers own resolved SEO documents. The storefront host composes those owner results but does not recreate their policies.

Optional external event infrastructure is outside the active Pages cursor. Optional external delivery infrastructure is outside the active Pages cursor.

## Rechecked merged cursor

Current `main` through PR #3029 contains:

- PR #2955 — publish/rollback event-correlation and generation miss/refill contract;
- PR #2971 — source-ready PostgreSQL publish/rollback outbox-to-cache packet;
- PR #2974 — source-ready durable relay failure/restart packet;
- PR #2979 — source-ready SQLite/Axum public artifact HTTP cache packet;
- PR #2985 — native storefront cache source packet;
- PR #2988 — registered Leptos storefront route source;
- PR #2990 — routed-channel admission before cache lookup;
- PR #2992 — reviewed immutable artifact selection;
- PR #2995 — synchronous test-target relay continuity;
- PR #2997 — production-listener topology correction;
- PR #3001 — production synchronous Pages generation gate and process-bounded dedupe;
- PR #3004 — production gate to registered native route source;
- PR #3006 — production-gate PostgreSQL retry source;
- PR #3008 — Memory and OutboxLocal factory profile parity source;
- PR #3010 — selected immutable artifact authority after draft mutation;
- PR #3011 — anonymous storefront dependency-graph source boundary;
- PR #3014 — anonymous SSR delivery boundary and explicit artifact inspector source;
- PR #3016 — public detail/list tenant locale fallback parity;
- PR #3018 — immutable published slug route aliases and localized canonical URLs;
- PR #3020 — registered host route decision and canonical/redirect/gone response composition;
- PR #3026 — forward public-route snapshots and delete tombstone composition;
- PR #3029 — explicit bounded historical route import with provenance receipts.

The present source slice composes Pages generations, channel identity, Navigation-owned menus, SEO-owned output and the exact rendered canonical document into a deterministic ETag without changing Page Builder/Fly behavior.

## Retained source marker index

This compact index preserves the exact stable source markers consumed by the retained static guards. It is descriptive only and does not promote execution evidence.

- `public-list-locale-fallback-source-ready`; Public list tenant locale fallback: source-ready. The native and GraphQL public detail/list reads share tenant fallback policy, and the cache variant already binds the fallback locale.
- `published-slug-route-alias-source-ready`; Published slug route aliases: source-ready. Localized canonical Pages routes remain the public identity model. The public host response is now source-ready.
- `host-route-response-source-ready`; Pages host route response: source-ready. The route decision precedes SEO and SSR rendering.
- `native-storefront-reviewed-artifact-source-ready`; Native reviewed immutable artifact selection: source-ready. The full Page Builder materialization envelope, durable `NodePublished`, and registered native storefront miss/refill remain source-ready.
- `native-storefront-channel-admission-source-ready`; Routed-channel admission before native lookup: source-ready. A populated composite cache cannot bypass channel module admission; the verified immutable Page Builder artifact and durable `NodePublished` relay delivery remain downstream boundaries.
- `selected-immutable-artifact-source-ready`; Selected immutable artifact after draft mutation: source-ready. The current Fly body is not public render authority.
- `production-relay-generation-gate-source-ready`; Production relay-to-Pages generation gate: source-ready. The production ordering remains: synchronous Pages invalidation now precedes downstream transport acceptance. The gate uses process-bounded dedupe. The retained continuity harness uses a custom synchronous relay target; the test-target packet and does not replace production-gate execution evidence.
- `production-relay-native-route-source-ready`; Production relay gate to registered native route: source-ready. The retained route sequence covers new-key miss/refill/hit; execution remains pending.
- `production-gate-postgres-restart-source-ready`; Production gate PostgreSQL publish/rollback restart: source-ready. The retained source covers a post-invalidation downstream failure; historical owner-transaction and pre-handler restart packets remain separate.
- `event-delivery-profile-parity-source-ready`; Memory and OutboxLocal factory profile parity: source-ready.
- `anonymous-storefront-graph-source-ready`; Anonymous storefront authoring exclusion: source-ready. The source guard uses feature-resolved `cargo metadata`; bundle artifact execution remains pending.
- `anonymous-storefront-ssr-delivery-source-ready`; Anonymous storefront SSR delivery: source-ready. The current public Pages host is SSR-only, and the client bundle gate is conditional.
- `delete-route-tombstone-source-ready`; Delete route tombstones: source-ready.
- `route-history-import-source-ready`; Historical route import: source-ready. The owner accepts explicit bounded provenance records; automatic historical inference remains deliberately unsupported.
- `storefront-composition-etag-source-ready`; Pages storefront Navigation/SEO composition ETag: source-ready. Exact canonical SSR binds Pages generations, channel identity, actual Navigation/SEO owner payloads and the exact rendered HTML.

Historical host-route marker retained for source-guard compatibility: `Delete tombstones and historical backfill remain open` was the correct PR #3020 boundary and is superseded by the current tombstone and explicit import statuses above.

## Current parity state

### Registered metadata surfaces: source-complete

Draft Pages workspaces and published Pages-owned metadata surfaces share the registered six-field consumer-property contribution. Published Fly authoring remains unmounted. The bespoke `PageMetadataEditor` and its direct workspace metadata transport write remain absent.

Focused stale-revision, metadata-only transport and dirty-Fly isolation regressions are source-ready. Browser and execution evidence remain pending.

### Reviewed publication and immutable rollback: source-complete

Pages owns reviewed publication from exact metadata/body revisions and promoted scenario review through authoritative sanitization, runtime materialization, immutable artifact persistence/binding, published lifecycle, transactional events and durable receipts.

Rollback verifies and selects a prior immutable publish manifest, replaces locale bindings and commits lifecycle events plus its receipt without compiling the current draft.

### Cache, native route and production delivery: source-ready

The retained source covers generation-bound artifact and storefront miss/refill/hit, conditional `304`, immutable verification before fill, old-generation physical retention, registered native server functions, channel-module admission before lookup, production generation rotation before transport acknowledgement, process-bounded same-event dedupe, PostgreSQL retry and Memory/OutboxLocal profile parity.

Execution remains pending.

### Public locale and immutable artifact authority: source-ready

Public detail and list reads resolve requested locale → tenant default locale → platform fallback. Exact and fallback public reads remain bound to the selected immutable published artifact after draft mutation until reviewed publish or rollback replaces the binding.

Execution remains pending.

### Published slug aliases and localized canonical routes: source-ready

Pages owns an append-only `page_route_aliases` ledger and a transport-neutral canonical/redirect/gone resolver. Published slug changes append redirects in the metadata transaction. Draft-only renames do not create public history. Old published route claims cannot be reused.

Localized canonical Pages routes and SEO alternates use:

```text
/{locale}/modules/pages?slug={slug}
```

The legacy unprefixed module path remains parseable but is not emitted as canonical. Current/history overlap and payload drift fail closed with `PAGE_ROUTE_RESOLUTION_CONFLICT`.

Execution remains pending.

### Pages host route response: source-ready

The registered `/api/fn/pages/route-decision` storefront adapter performs trusted tenant/request resolution, channel-module admission, locale fallback and target publication/channel rechecks before SEO and SSR.

```text
exact localized canonical → continue SSR
legacy/noncanonical/alias → 308 Permanent Redirect
immutable gone → 410 Gone
unknown or channel-ineligible → 404 Not Found
ambiguous current/history ownership → 409 Conflict
operational decision failure → 503 Service Unavailable
```

Terminal Pages responses use `Cache-Control: private, no-store`.

Execution remains pending.

### Delete route tombstones: source-ready

Pages retains a forward-only `page_route_publications` snapshot ledger. A page leaving `published` through unpublish or archive records each localized public route before lifecycle mutation. The ledger has no page foreign key and therefore survives physical deletion.

A never-published draft creates no public snapshot and its slug remains reusable after delete.

For an admitted non-published delete, `PageService::delete` records missing `gone` aliases with stable reason `Page deleted` before deleting bodies, translations and the page row, and before committing `NodeDeleted`. Existing immutable redirect rows are preserved rather than rewritten. When their target page is physically absent and has a retained tombstone, route resolution folds those historical redirects into `Gone`, so every formerly public route reaches the host's existing `410` response.

Execution remains pending.

### Historical route import: source-ready

`PageRouteHistoryImportService::import_public_routes` is the explicit repair owner for public route history that cannot be reconstructed safely from current Pages state.

The command requires `pages:manage`, accepts one normalized source and 1–100 route items, and commits the batch in one transaction. Every accepted item creates or verifies:

- an immutable `page_route_history_imports` provenance receipt keyed by tenant, source and source-record identifier;
- a canonical SHA-256 request hash over page, locale and slug;
- the exact `page_route_publications` retained claim;
- a direct `gone` alias when the page was already missing and the route was unclaimed.

Exact replay is idempotent. Provenance payload drift and current/snapshot/alias ownership overlap fail closed with `PAGE_ROUTE_HISTORY_IMPORT_CONFLICT`.

Existing same-page published-slug redirects remain immutable. A missing page with redirect-only history must already have, or the same batch must add, at least one direct terminal `gone` route; otherwise the entire batch rolls back. Existing pages receive a snapshot only and enter `Gone` through the normal delete owner later.

Automatic scans of old translations, Page Builder artifacts or current draft/archived rows are not claimed because those sources do not prove complete historical public ownership.

Execution remains pending.

### Storefront Navigation/SEO composition ETag: source-ready

For an exact localized canonical Pages request, the route adapter now exposes channel identity and the current Pages route, page and artifact generations only after channel-module admission plus publication/channel visibility rechecks.

The storefront host then loads Navigation Header and Footer through the existing Navigation-owned transport and loads the resolved SEO page context through the existing SEO owner path. A `StorefrontNavigationSnapshot` is supplied to the same Leptos SSR owner, so `NavigationHeaderMenu` and `NavigationView` reuse the preloaded menus rather than issuing duplicate SSR requests.

The host renders the canonical document before deciding a conditional response. This is deliberate: the Pages component may read the owner cache during SSR, so an earlier route-decision generation snapshot alone cannot prove which body was rendered during a concurrent rotation.

The final `pages_storefront_composition_v1` ETag binds:

- canonical page id, slug and effective locale;
- request locale and channel identity;
- route, page and artifact generations;
- the actual resolved Navigation header/footer payloads;
- the actual resolved SEO page context;
- a SHA-256 hash of the exact final rendered HTML.

The deterministic serialized payload is hashed with SHA-256. A matching strong, weak or comma-separated `If-None-Match` returns `304 Not Modified` only after reconstructing that exact document identity. Canonical ETag responses use `Cache-Control: private, no-cache`; terminal Pages route responses continue to use `private, no-store` and never claim a composition ETag.

If the Pages generation runtime is absent or the generation read fails, SSR continues without an ETag. This avoids a false cache identity while preserving fail-open rendering after all authorization and route checks have succeeded.

No shared/CDN full-document cache is introduced, and the conditional request does not skip SSR work. Navigation menu policy and SEO resolution remain with their owners.

Source evidence:

- `crates/rustok-pages/storefront/src/transport/host_route_adapter.rs`;
- `crates/rustok-navigation/storefront/src/model.rs`;
- `crates/rustok-navigation/storefront/src/ui/menu.rs`;
- `apps/storefront/src/shared/context/pages_composition.rs`;
- `apps/storefront/src/lib.rs`;
- `crates/rustok-pages/contracts/evidence/pages-storefront-composition-etag-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-storefront-composition-etag.mjs`;
- `docs/modules/pages-page-builder-storefront-composition-etag-packet-2026-08-06.md`.

Execution remains pending.

### Anonymous storefront boundary: source-ready

The current public Pages host is SSR-only. Retained source guards exclude Pages/Page Builder/Fly authoring dependencies and executable hydration/bootstrap markers from the selected anonymous host profiles. The explicit built-artifact inspector remains source-ready; build and artifact inspection remain pending.

### Authenticated real-DOM inline editing: open

Authenticated real-DOM inline editing is not implemented and is the next unimplemented Pages storefront source boundary after execution evidence.

## Parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Metadata schema and owner port | Complete | Conflict/isolation and browser packets pending |
| Draft/published registered metadata | Complete | Browser execution pending |
| Reviewed publish and immutable manifest | Complete | Database/runtime evidence pending |
| Immutable rollback | Complete | Database/runtime evidence pending |
| Public detail/list tenant locale fallback | Source-ready | Focused SQLite/native/GraphQL execution pending |
| Published slug alias ledger and localized canonical URLs | Source-ready | SQLite/PostgreSQL/SEO execution pending |
| Host canonical/redirect/gone response | Source-ready | Registered server-function and host SSR execution pending |
| Delete route tombstones for new lifecycle transitions | Source-ready | SQLite/PostgreSQL/host execution pending |
| Explicit historical route import | Source-ready | SQLite/PostgreSQL/operator execution pending |
| Automatic historical route inference | Deliberately unsupported | External provenance required |
| Pages Navigation/SEO composition ETag | Source-ready | SSR/conditional request/browser execution pending |
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
| Authenticated real-DOM inline editing | Open | Not implemented |

## Boundaries

This slice changes Pages route-decision output, Navigation storefront composition contracts and exact canonical Pages host response composition.

It does not:

- change Pages persistence, route claims or lifecycle policy;
- change Navigation menu identity, bindings, locale fallback or database ownership;
- change SEO providers, targets or schemas;
- add a shared/CDN full-document cache;
- skip SSR work for conditional requests;
- change Page Builder or Fly behavior;
- change page bodies, immutable artifacts, publish or rollback receipts;
- change GraphQL, REST or admin surfaces;
- change event schemas or optional external event infrastructure;
- claim tests, Cargo, formatting, verifiers, SQLite, PostgreSQL, hosts, browsers, workflows, CI or rollout execution;
- promote FFA or FBA.

## Next cursor

1. Run the storefront composition ETag verifier and focused storefront tests.
2. Run the route history import verifier and focused SQLite regression.
3. Run the delete route tombstone verifier and focused SQLite regression.
4. Run the host route response verifier and registered SQLite/Axum server-function regression.
5. Run the published slug alias verifier and focused SQLite regression.
6. Run the public list locale fallback verifier and focused Pages locale regression.
7. Run the native cache, registered server-function and channel-admission guards with their route harnesses.
8. Run the anonymous dependency-graph and SSR delivery packets plus explicit built-artifact inspection.
9. Run the selected immutable artifact and complete native SQLite/Axum route set.
10. Run production generation-gate, native-route and PostgreSQL retry packets.
11. Run metadata conflict/isolation and published metadata browser packets.
12. Implement authenticated real-DOM inline editing as a separate source slice.
13. Complete workflow and observed tenant rollout evidence before promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-storefront-composition-etag.mjs
cargo test -p rustok-storefront --features ssr --lib -- --nocapture
cargo test -p rustok-navigation-storefront --features ssr --all-targets -- --nocapture

node crates/rustok-pages/scripts/verify/verify-pages-route-history-import.mjs
cargo test -p rustok-pages \
  --test page_route_history_import_sqlite -- --nocapture

node crates/rustok-pages/scripts/verify/verify-pages-delete-route-tombstone.mjs
cargo test -p rustok-pages \
  --test page_delete_route_tombstone_sqlite -- --nocapture
cargo test -p rustok-pages \
  --test page_published_slug_route_alias_sqlite -- --nocapture
cargo check -p rustok-pages --all-targets

node crates/rustok-pages/scripts/verify/verify-pages-host-route-response.mjs
cargo test -p rustok-pages-storefront --features ssr \
  --test host_route_decision_sqlite -- --nocapture
cargo test -p rustok-storefront --features ssr --lib -- --nocapture

node crates/rustok-pages/scripts/verify/verify-pages-public-list-locale-fallback.mjs
cargo test -p rustok-pages --test page_locale_fallback -- --nocapture

node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-cache.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-server-fn.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-channel-admission.mjs

node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-ssr-delivery.mjs
```

Any failure or owner-model change must update this shared cursor before FFA/FBA promotion.
