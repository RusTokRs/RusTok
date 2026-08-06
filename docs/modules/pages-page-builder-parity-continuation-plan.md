# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-06  
Status: source-parity-current / delete-route-tombstone-source-ready / execution-evidence-pending
Scope: `rustok-pages` admin/storefront FFA and `rustok-page-builder` consumer-property, publication, artifact, event, routing and cache boundaries

## Source-of-truth policy

This is the canonical shared continuation cursor. Historical dated packets remain evidence for the source slices that produced the present state, but they do not override this plan.

`source-ready` means that code, contracts or retained harness source exists. It does not mean that tests, Cargo, formatting, verifiers, databases, HTTP routes, server functions, production event topology, browsers, workflows, CI, built artifacts or tenant rollout were executed.

Across every retained source packet, execution remains pending until a maintainer records reproducible command output and artifact evidence.

Pages and Page Builder continue as one vertical pipeline with explicit owners. Pages owns persistence, lifecycle, immutable bindings, localized route identity, cache policy and public reads. Page Builder/Fly owns the reviewed document, sanitizer, runtime materialization, renderer and artifact producer contracts.

Optional external event infrastructure is outside the active Pages cursor.

## Rechecked merged cursor

Current `main` through PR #3020 contains:

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
- PR #3020 — registered host route decision and canonical/redirect/gone response composition.

The present source slice adds forward lifecycle ownership for delete route tombstones without changing Page Builder/Fly behavior.

## Retained source marker index

This compact index preserves the exact stable source markers consumed by the retained static guards. It is descriptive only and does not promote execution evidence.

- `public-list-locale-fallback-source-ready`; Public list tenant locale fallback: source-ready. The native and GraphQL public detail/list reads share tenant fallback policy, and the cache variant already binds the fallback locale.
- `published-slug-route-alias-source-ready`; Published slug route aliases: source-ready. Localized canonical Pages routes remain the public identity model. The public host response is now source-ready.
- `host-route-response-source-ready`; Pages host route response: source-ready. The route decision precedes SEO and SSR rendering.
- `native-storefront-reviewed-artifact-source-ready`; Native reviewed immutable artifact selection: source-ready.
- `native-storefront-channel-admission-source-ready`; Routed-channel admission before native lookup: source-ready.
- `selected-immutable-artifact-source-ready`; Selected immutable artifact after draft mutation: source-ready. The current Fly body is not public render authority.
- `production-relay-generation-gate-source-ready`; Production relay-to-Pages generation gate: source-ready. Synchronous Pages invalidation now precedes downstream transport acceptance and uses process-bounded dedupe.
- `production-relay-native-route-source-ready`; Production relay gate to registered native route: source-ready.
- `production-gate-postgres-restart-source-ready`; Production gate PostgreSQL publish/rollback restart: source-ready.
- `event-delivery-profile-parity-source-ready`; Memory and OutboxLocal factory profile parity: source-ready.
- `anonymous-storefront-graph-source-ready`; Anonymous storefront authoring exclusion: source-ready.
- `anonymous-storefront-ssr-delivery-source-ready`; Anonymous storefront SSR delivery: source-ready.
- `delete-route-tombstone-source-ready`; Delete route tombstones: source-ready. Historical route backfill/import policy remains open.

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

`delete-route-tombstone-source-ready`; Delete route tombstones: source-ready.

Pages now retains a forward-only `page_route_publications` snapshot ledger. A page leaving `published` through unpublish or archive records each localized public route before lifecycle mutation. The ledger has no page foreign key and therefore survives physical deletion.

A never-published draft creates no public snapshot and its slug remains reusable after delete.

For an admitted non-published delete, `PageService::delete` records missing `gone` aliases with stable reason `Page deleted` before deleting bodies, translations and the page row, and before committing `NodeDeleted`. Existing immutable redirect rows are preserved rather than rewritten. When their target page is physically absent and has a retained tombstone, route resolution folds those historical redirects into `Gone`, so every formerly public route reaches the host's existing `410` response.

Source evidence:

- `crates/rustok-pages/src/migrations/m20260806_000011_create_page_route_publications.rs`;
- `crates/rustok-pages/src/entities/page_route_publication.rs`;
- `crates/rustok-pages/src/services/page/route.rs`;
- `crates/rustok-pages/src/services/page/lifecycle.rs`;
- `crates/rustok-pages/tests/page_delete_route_tombstone_sqlite.rs`;
- `crates/rustok-pages/contracts/evidence/pages-delete-route-tombstone-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-delete-route-tombstone.mjs`;
- `docs/modules/pages-page-builder-delete-route-tombstone-packet-2026-08-06.md`.

Historical route backfill/import policy remains open. Execution evidence remains pending.

### Anonymous storefront boundary: source-ready

The current public Pages host is SSR-only. Retained source guards exclude Pages/Page Builder/Fly authoring dependencies and executable hydration/bootstrap markers from the selected anonymous host profiles. The explicit built-artifact inspector remains source-ready; build and artifact inspection remain pending.

### Authenticated real-DOM inline editing: open

Authenticated real-DOM inline editing is not implemented and remains outside this routing/lifecycle slice.

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
| Historical route backfill/import | Open | Not implemented |
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

This slice changes Pages route-history persistence, lifecycle composition and route resolution.

It does not:

- change Page Builder or Fly behavior;
- change page bodies, immutable artifacts, publish or rollback receipts;
- change GraphQL or REST schemas;
- add historical route backfill/import;
- change channel visibility or module-admission policy;
- change cache namespaces, generation scopes, key shape, TTL or capacity;
- change event schemas or optional external event infrastructure;
- claim tests, Cargo, formatting, verifiers, SQLite, PostgreSQL, hosts, browsers, workflows, CI or rollout execution;
- promote FFA or FBA.

## Next cursor

1. Run the delete route tombstone verifier and focused SQLite regression.
2. Run the host route response verifier and registered SQLite/Axum server-function regression.
3. Run the published slug alias verifier and focused SQLite regression.
4. Define bounded historical route backfill/import policy as a separate source slice.
5. Run the public list locale fallback verifier and focused Pages locale regression.
6. Run the native cache, registered server-function and channel-admission guards with their route harnesses.
7. Run the anonymous dependency-graph and SSR delivery packets plus explicit built-artifact inspection.
8. Run the selected immutable artifact and complete native SQLite/Axum route set.
9. Run production generation-gate, native-route and PostgreSQL retry packets.
10. Run metadata conflict/isolation and published metadata browser packets.
11. Complete workflow and observed tenant rollout evidence before promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
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
