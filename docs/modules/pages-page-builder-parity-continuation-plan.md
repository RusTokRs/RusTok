# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-06  
Status: source-parity-current / authenticated-inline-adapter-source-ready / consumer-mount-open / execution-evidence-pending
Scope: `rustok-pages` admin/storefront FFA and `rustok-page-builder` consumer-property, publication, artifact, event, routing, cache and authenticated inline-adapter boundaries

## Source-of-truth policy

This is the canonical shared continuation cursor. Historical dated packets remain evidence for the source slices that produced the present state, but they do not override this plan.

`source-ready` means that code, contracts or retained harness source exists. It does not mean that tests, Cargo, formatting, verifiers, databases, HTTP routes, server functions, production event topology, WASM, browsers, workflows, CI, built artifacts or tenant rollout were executed.

Across every retained source packet, execution remains pending until a maintainer records reproducible command output and artifact evidence.

Pages and Page Builder continue as one vertical pipeline with explicit owners. Pages owns persistence, lifecycle, immutable bindings, localized route identity, cache policy, public reads and future consumer inline grants/save transport. Page Builder/Fly owns the reviewed document, sanitizer, runtime materialization, renderer, artifact producer contracts and reusable inline adapter/session. Navigation owns menu identity and active-menu policy. SEO providers own resolved SEO documents. Hosts compose owner results but do not recreate their policies.

Optional external event infrastructure is outside the active Pages cursor. Optional external delivery infrastructure is outside the active Pages cursor.

## Rechecked merged cursor

Current `main` through PR #3032 contains:

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
- PR #3029 — explicit bounded historical route import with provenance receipts;
- PR #3032 — exact rendered Pages/Navigation/SEO private revalidation ETag.

The present source slice adds a reusable feature-gated authenticated real-DOM inline adapter and canonical Fly patch session. Pages consumer grant issuance and document-only save mount remain open.

## Retained source marker index

This compact index preserves the exact stable source markers consumed by retained static guards. It is descriptive only and does not promote execution evidence.

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
- `storefront-composition-etag-source-ready`; Pages storefront Navigation/SEO composition ETag: source-ready. Exact canonical SSR binds Pages generations, channel identity, actual Navigation/SEO owner payloads and the exact rendered HTML. Canonical responses use `Cache-Control: private, no-cache`. Terminal Pages route responses continue to use `private, no-store`.
- `authenticated-inline-adapter-source-ready`; Authenticated real-DOM inline adapter: source-ready. Fly remains the sole document authority; Pages consumer grant issuance and document-only save mount remain open.

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

The command requires `pages:manage`, accepts one normalized source and 1–100 route items, and commits the batch in one transaction. Every accepted item creates or verifies an immutable provenance receipt, canonical SHA-256 request hash, exact retained route claim and direct `gone` alias when the page was already missing and the route was unclaimed.

Exact replay is idempotent. Provenance payload drift and current/snapshot/alias ownership overlap fail closed with `PAGE_ROUTE_HISTORY_IMPORT_CONFLICT`. Existing same-page redirects remain immutable. Automatic scans of old translations, Page Builder artifacts or current draft/archived rows are deliberately unsupported because they do not prove complete historical public ownership.

Execution remains pending.

### Storefront Navigation/SEO composition ETag: source-ready

For an exact localized canonical Pages request, the route adapter exposes channel identity and current Pages route/page/artifact generations only after channel-module admission plus publication/channel visibility rechecks.

The host loads Navigation Header/Footer through the Navigation-owned transport and the resolved SEO context through the SEO owner. A `StorefrontNavigationSnapshot` is supplied to the same Leptos SSR owner, so navigation components reuse the preloaded menus.

The host renders before deciding a conditional response. The final `pages_storefront_composition_v1` ETag binds canonical identity, channel, all three Pages generations, actual Navigation/SEO payloads and a SHA-256 hash of the exact final HTML. Matching strong, weak or comma-separated `If-None-Match` returns `304 Not Modified` only after reconstructing the same document. Request-specific nonce-bearing HTML fails closed to ordinary SSR without an ETag.

No shared/CDN document cache is introduced. Execution remains pending.

### Authenticated real-DOM inline adapter: source-ready

`fly-leptos` now provides a reusable real-DOM adapter whose trusted grant binds session, stable selected page, consumer revision, exact Fly project hash, opaque authorization proof and expiry. The proof is redacted from `Debug` and never rendered into DOM.

Only an explicit allow-list of instrumented stable static leaf text nodes receives `contenteditable="plaintext-only"`. Provider-owned nodes, composite nodes with children and template-backed nodes remain read-only. Every node inside a runtime-owned binding, condition or repeater subtree is also excluded, while a static leaf inside an ordinary unowned layout remains eligible. The DOM is a temporary interaction buffer: a single bounded normalized plain-text request is emitted on bubbling `focusout`, and listener/attribute cleanup is deterministic.

`rustok-page-builder-storefront` exposes this surface only behind the optional `inline-edit` feature. Its existing read-only renderer continues to force component instrumentation off, and current Pages anonymous features do not enable `inline-edit`.

`AuthenticatedInlineEditSession` validates grant identity/expiry, monotonic sequence, selected page, exact project hash and component eligibility, then calls a consumer `InlineEditAuthorizationPort` immediately before the sole mutation:

```text
EditorCommand::Patch
  → ComponentPatch::set_field("content", plain_text)
  → Fly history, validation and new project hash
```

The result carries the complete current project, previous/new hash and command sequence. The grant is intentionally one-commit because the canonical hash changes; a consumer must persist the result and issue a fresh grant.

Pages consumer grant issuance and document-only save mount remain open. No Pages transport, auth policy or anonymous mount is claimed by this slice.

Source evidence:

- `crates/fly-leptos/src/real_dom_inline.rs`;
- `crates/rustok-page-builder-storefront/src/inline_edit.rs`;
- `crates/rustok-page-builder/contracts/evidence/page-builder-authenticated-inline-edit-adapter-source.json`;
- `crates/rustok-page-builder/scripts/verify/verify-page-builder-authenticated-inline-edit-adapter.mjs`;
- `docs/modules/pages-page-builder-authenticated-inline-edit-adapter-packet-2026-08-06.md`;
- `docs/modules/page-builder-parity-actualization-2026-08-06-inline-edit.md`.

Execution remains pending.

### Anonymous storefront boundary: source-ready

The current public Pages host is SSR-only. Retained source guards exclude Pages/Page Builder/Fly authoring dependencies and executable hydration/bootstrap markers from selected anonymous host profiles. The explicit built-artifact inspector remains source-ready; build and artifact inspection remain pending.

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
| Delete route tombstones | Source-ready | SQLite/PostgreSQL/host execution pending |
| Explicit historical route import | Source-ready | SQLite/PostgreSQL/operator execution pending |
| Pages Navigation/SEO composition ETag | Source-ready | SSR/conditional request/browser execution pending |
| Reusable authenticated real-DOM inline adapter | Source-ready | Rust/WASM/browser/auth-port execution pending |
| Pages consumer inline grant + document save mount | Open | Not implemented |
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

## Boundaries

This slice changes only reusable Fly browser contracts and feature-gated Page Builder storefront inline source.

It does not:

- authenticate users or issue a Pages grant;
- add a Pages save server function, GraphQL mutation, HTTP route or persistence path;
- mount editing in the anonymous storefront;
- treat DOM as a document tree or hidden authority;
- edit rich text, nested markup, provider components, composite component content or runtime-owned subtrees;
- change Pages persistence, lifecycle, route claims, caches or events;
- change Page Builder publish, rollback, sanitizer, materialization or artifacts;
- change database schemas or migrations;
- claim tests, Cargo, formatting, verifiers, WASM, browsers, dependency graphs, workflows, CI or rollout execution;
- promote FFA or FBA.

## Next cursor

1. Implement the Pages authenticated inline grant issuer and document-only save transport/mount.
2. Run the authenticated inline adapter static verifier, Rust tests and WASM/browser packet.
3. Re-run the anonymous dependency graph and built-artifact exclusion checks with inline feature source present.
4. Run the storefront composition ETag verifier and focused storefront tests.
5. Run route-history import, delete tombstone, host route, slug alias and locale fallback evidence.
6. Run native cache/admission, immutable artifact and production generation-gate packets.
7. Run metadata conflict/isolation and published metadata browser packets.
8. Complete workflow and observed tenant rollout evidence before promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-page-builder/scripts/verify/verify-page-builder-authenticated-inline-edit-adapter.mjs
cargo test -p fly-leptos --all-targets -- --nocapture
cargo test -p rustok-page-builder-storefront \
  --features inline-edit,ssr --all-targets -- --nocapture
cargo check -p rustok-page-builder-storefront \
  --features inline-edit,hydrate --target wasm32-unknown-unknown
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs

node crates/rustok-pages/scripts/verify/verify-pages-storefront-composition-etag.mjs
cargo test -p rustok-storefront --features ssr --lib -- --nocapture
cargo test -p rustok-navigation-storefront --features ssr --all-targets -- --nocapture

node crates/rustok-pages/scripts/verify/verify-pages-route-history-import.mjs
cargo test -p rustok-pages --test page_route_history_import_sqlite -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-delete-route-tombstone.mjs
cargo test -p rustok-pages --test page_delete_route_tombstone_sqlite -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-host-route-response.mjs
cargo test -p rustok-pages-storefront --features ssr \
  --test host_route_decision_sqlite -- --nocapture
```

Any failure or owner-model change must update this shared cursor before FFA/FBA promotion.
