# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-06  
Status: source-parity-current / authenticated-authoring-route-source-ready / inline-edit-asset-delivery-source-ready / release-integration-admin-launch-browser-pending
Scope: `rustok-pages` admin/storefront FFA and `rustok-page-builder` document, publication, artifact, routing, cache and authenticated inline-authoring boundaries

## Source-of-truth policy

This is the canonical shared continuation cursor. Historical dated packets remain evidence for the source slices that produced the present state, but they do not override this plan.

`source-ready` means that code, contracts or retained harness source exists. It does not mean that tests, Cargo, formatting, verifiers, databases, HTTP routes, server functions, production event topology, WASM, browsers, workflows, CI, built artifacts or tenant rollout were executed.

Across every retained source packet, execution remains pending until a maintainer records reproducible command output and artifact evidence.

Pages and Page Builder continue as one vertical pipeline with explicit owners. Pages owns persistence, lifecycle, immutable bindings, localized route identity, cache policy, public reads, authenticated inline grants/save transport and the module-owned inline asset HTTP contract. Page Builder/Fly owns the reviewed document, sanitizer, runtime materialization, renderer, artifact producer contracts and reusable real-DOM inline adapter/session. Navigation owns menu identity and active-menu policy. SEO providers own resolved SEO documents. Hosts compose owner results, authenticated route admission, CSP and HTTP response policy but do not recreate Pages document, route or asset policy.

Optional external event infrastructure is outside the active Pages cursor. Optional external delivery infrastructure is outside the active Pages cursor.

## Rechecked merged cursor

Current `main` through PR #3056 contains:

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
- PR #3032 — exact rendered Pages/Navigation/SEO private revalidation ETag;
- PR #3039 — reusable authenticated real-DOM adapter and canonical Fly patch session;
- PR #3049 — Pages-owned signed inline grants, bootstrap/commit transport and document-only save consumer;
- PR #3056 — authenticated authoring module route, admission/response policy and client export/build source.

The present source slice adds a Pages-owned feature-gated router for the three fixed authoring assets, compile-time binary embedding, exact MIME/revalidation/ETag/CORP behavior and an exact-lock client/server build orchestrator. Release workflow and admin launch integration remain pending. Client/server build, HTTP delivery and browser execution remain pending.

## Retained source marker index

This index preserves exact stable source markers consumed by retained static guards. It is descriptive only and does not promote execution evidence.

- `public-list-locale-fallback-source-ready`; Public list tenant locale fallback: source-ready. The native and GraphQL public detail/list reads share tenant fallback policy, and the cache variant already binds the fallback locale.
- `published-slug-route-alias-source-ready`; Published slug route aliases: source-ready. Localized canonical Pages routes remain the public identity model. The public host response is now source-ready.
- `host-route-response-source-ready`; Pages host route response: source-ready. The route decision precedes SEO and SSR rendering.
- `native-storefront-reviewed-artifact-source-ready`; Native reviewed immutable artifact selection: source-ready. The full Page Builder materialization envelope, durable `NodePublished`, and registered native storefront miss/refill remain source-ready.
- `native-storefront-channel-admission-source-ready`; Routed-channel admission before native lookup: source-ready. A populated composite cache cannot bypass channel module admission; the verified immutable Page Builder artifact and durable `NodePublished` relay delivery remain downstream boundaries.
- `selected-immutable-artifact-source-ready`; Selected immutable artifact after draft mutation: source-ready. The current Fly body is not public render authority.
- `production-relay-generation-gate-source-ready`; Production relay-to-Pages generation gate: source-ready. The production ordering remains: synchronous Pages invalidation now precedes downstream transport acceptance. The gate uses process-bounded dedupe. The retained continuity harness uses a custom synchronous relay target and does not replace production-gate execution evidence.
- `production-relay-native-route-source-ready`; Production relay gate to registered native route: source-ready. The retained route sequence covers new-key miss/refill/hit; execution remains pending.
- `production-gate-postgres-restart-source-ready`; Production gate PostgreSQL publish/rollback restart: source-ready. The retained source covers a post-invalidation downstream failure; historical owner-transaction and pre-handler restart packets remain separate.
- `event-delivery-profile-parity-source-ready`; Memory and OutboxLocal factory profile parity: source-ready.
- `anonymous-storefront-graph-source-ready`; Anonymous storefront authoring exclusion: source-ready. The source guard uses feature-resolved `cargo metadata`; bundle artifact execution remains pending.
- `anonymous-storefront-ssr-delivery-source-ready`; Anonymous storefront SSR delivery: source-ready. The current public Pages host is SSR-only, and the client bundle gate is conditional.
- `delete-route-tombstone-source-ready`; Delete route tombstones: source-ready.
- `route-history-import-source-ready`; Historical route import: source-ready. The owner accepts explicit bounded provenance records; automatic historical inference remains deliberately unsupported.
- `storefront-composition-etag-source-ready`; Pages storefront Navigation/SEO composition ETag: source-ready. Exact canonical SSR binds Pages generations, channel identity, actual Navigation/SEO owner payloads and exact rendered HTML. Canonical responses use `Cache-Control: private, no-cache`. Terminal Pages route responses continue to use `private, no-store`.
- `authenticated-inline-adapter-source-ready`; Authenticated real-DOM inline adapter: source-ready. Fly remains the sole document authority.
- `authenticated-inline-consumer-source-ready`; Pages authenticated inline consumer: source-ready. Signed grants, direct-user/session binding, opt-in server functions and the existing document-save owner are composed.
- `authenticated-authoring-route-source-ready`; Authenticated authoring route: source-ready. The existing module-page route mounts the consumer behind direct-user/session/`pages:update` admission and private non-indexable response policy; client artifact build and browser execution remain pending.
- `inline-edit-asset-delivery-source-ready`; Dedicated authoring asset delivery: source-ready. Pages conditionally embeds and serves the fixed bootstrap/JS/WASM contract; release workflow and admin launch integration remain pending.

Historical host-route marker retained for source-guard compatibility: `Delete tombstones and historical backfill remain open` was the correct PR #3020 boundary and is superseded by current tombstone and explicit import statuses.

Historical adapter marker retained for source-guard compatibility: `Pages consumer grant issuance and document-only save mount remain open` was the correct PR #3039 boundary and is superseded by the current consumer status.

Historical consumer marker retained for source-guard compatibility: `authenticated route mount remains open` was the correct PR #3049 boundary and is superseded by the authenticated authoring route source.

## Current parity state

### Registered metadata surfaces: source-complete

Draft Pages workspaces and published Pages-owned metadata surfaces share the registered six-field consumer-property contribution. The bespoke `PageMetadataEditor` and its direct workspace metadata transport write remain absent.

Focused stale-revision, metadata-only transport and dirty-Fly isolation regressions are source-ready. Browser and execution evidence remain pending.

### Reviewed publication and immutable rollback: source-complete

Pages owns reviewed publication from exact metadata/body revisions and promoted scenario review through authoritative sanitization, runtime materialization, immutable artifact persistence/binding, published lifecycle, transactional events and durable receipts.

Rollback verifies and selects a prior immutable publish manifest, replaces locale bindings and commits lifecycle events plus its receipt without compiling the current draft.

### Cache, route and production delivery: source-ready

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

### Delete route tombstones and historical route import: source-ready

Pages retains forward-only published-route snapshots, writes immutable gone aliases during delete, preserves redirect history and prevents reuse of formerly public route claims. Never-published drafts do not reserve a public claim.

`PageRouteHistoryImportService::import_public_routes` is the explicit repair owner for history that cannot be reconstructed safely. Exact replay is idempotent. Provenance drift and ownership overlap fail closed with `PAGE_ROUTE_HISTORY_IMPORT_CONFLICT`. Automatic scans of old translations, Page Builder artifacts or current draft/archived rows remain deliberately unsupported.

Execution remains pending.

### Storefront Navigation/SEO composition ETag: source-ready

For an exact canonical Pages request, the host loads Navigation Header/Footer through the Navigation-owned transport and resolved SEO context through the SEO owner. The final private revalidation ETag binds canonical identity, channel, Pages generations, actual owner payloads and a SHA-256 hash of the exact final HTML.

No shared/CDN document cache is introduced. Execution remains pending.

### Reusable authenticated real-DOM inline adapter: source-ready

`fly-leptos` owns the temporary real-DOM interaction adapter. `rustok-page-builder-storefront` owns the feature-gated canonical session. Provider-owned, composite, templated, interactive and runtime-owned subtrees remain read-only. Unchanged focusout requests do not consume a grant. Changed requests pass grant/hash/sequence/component checks, then a consumer authorization port immediately before one canonical `EditorCommand::Patch`.

The adapter slice remains independently source-ready. Execution remains pending.

### Pages authenticated inline consumer: source-ready

Pages owns a versioned HMAC-SHA256 grant contract binding tenant, user, direct authenticated session, a separate fresh edit-session UUID, channel identity, Pages page, stable Fly page, exact locale, current body revision, project hash and expiry.

The grant keyring has no fallback secret. Missing host configuration leaves the runtime unavailable; invalid explicit configuration fails registration. Secret material and proofs are redacted from `Debug`.

Bootstrap and commit are feature-gated Leptos server functions. Both require a direct user principal, matching tenant/user request context, explicit `pages.builder.inline_edit.enabled`, `pages:update`, an unpublished exact-locale GrapesJS body and stable Fly page/component ids.

Commit verification order is:

```text
initial HMAC verification
→ exact auth/edit-session/channel/page/locale/revision/hash match
→ current authorized document reload
→ current revision/hash proof
→ HMAC verification at mutation time
→ canonical AuthenticatedInlineEditSession
→ consumer authorization port
→ one Fly patch
→ tenant capability recheck
→ PageService::save_document(expected_revision)
→ committed revision
→ fresh replacement grant
```

The storefront transport does not write page-body rows. The existing optimistic document owner remains the final concurrency/replay fence and retains its transaction and `NodeUpdated` behavior.

Profiles `rustok-pages-storefront/inline-edit`, `rustok-storefront/pages-inline-edit`, `rustok-storefront/pages-inline-edit-hydrate` and `rustok-server/pages-inline-edit` are non-default. The authenticated inline profiles are opt-in. Retained anonymous default/CSR/hydrate/SSR profiles do not enable them.

### Authenticated authoring route: source-ready

The existing storefront module-page owner registers `pages-authoring` only under `rustok-storefront/pages-inline-edit`:

```text
/modules/pages-authoring?page_id=<pages UUID>&lang=<locale>
/{locale}/modules/pages-authoring?page_id=<pages UUID>
```

The registered page remains owned by module slug `pages`, so normal tenant module enablement applies. The auth middleware additionally requires a direct user principal, non-nil authenticated session and effective `pages:update` before authoring HTML or the bootstrap/commit endpoints execute. Exact owner-aware authorization remains downstream in Pages.

Every authoring HTML and inline server-function response uses `Cache-Control: private, no-store`. HTML also uses `X-Robots-Tag: noindex, nofollow, noarchive`. The existing outer nonce-backed UI CSP remains authoritative. The route emits only a same-origin external bootstrap module; proof material is not written to DOM.

`pages-inline-edit-bootstrap.js` imports the dedicated JS/WASM module only after finding the bounded authoring root. The target-gated `start_pages_inline_edit_client` export reads page/locale identity, removes the SSR shell and mounts one client surface.

Client artifact build and browser execution remain pending. No authenticated route execution or browser edit result is claimed.

### Dedicated authoring asset delivery: source-ready

Fixed contract:

```text
/assets/pages-inline-edit-bootstrap.js
/assets/pages-inline-edit/rustok_storefront.js
/assets/pages-inline-edit/rustok_storefront_bg.wasm
```

`rustok-pages/inline-edit-assets` conditionally merges a module-owned Axum router. The three generated files are compiled into the binary with `include_bytes!`; missing files fail compilation for that profile. Runtime-only release packaging therefore needs no `target/site` directory.

JavaScript uses `text/javascript; charset=utf-8`; WASM uses `application/wasm`. Stable paths use `Cache-Control: public, max-age=0, must-revalidate`, full SHA-256 ETags, exact/weak `If-None-Match` handling with `304`, and `Cross-Origin-Resource-Policy: same-origin`.

The client builder now reads the exact `wasm-bindgen` version from `Cargo.lock`, rejects a mismatched CLI, uses Cargo `--locked`, respects external `CARGO_TARGET_DIR`, validates outputs and atomically publishes the generated pair. `scripts/build/build-pages-inline-edit-server.sh` installs/verifies that exact CLI, prepares all assets and then builds `rustok-server --features pages-inline-edit-assets`.

Release workflow and admin launch integration remain pending. Production Docker builder integration remains pending. No client artifact, embedded server binary, HTTP asset response or browser result is claimed.

Source evidence:

- `apps/storefront/src/modules/core.rs`;
- `apps/storefront/public/assets/pages-inline-edit-bootstrap.js`;
- `apps/storefront/scripts/build-pages-inline-edit-client.mjs`;
- `apps/server/src/middleware/auth_context.rs`;
- `apps/server/Cargo.toml`;
- `crates/rustok-pages/src/http.rs`;
- `crates/rustok-pages/src/http/inline_edit_assets.rs`;
- `scripts/build/build-pages-inline-edit-server.sh`;
- `crates/rustok-pages/contracts/evidence/pages-authenticated-authoring-route-source.json`;
- `crates/rustok-pages/contracts/evidence/pages-inline-edit-asset-delivery-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-authenticated-authoring-route.mjs`;
- `crates/rustok-pages/scripts/verify/verify-pages-inline-edit-asset-delivery.mjs`;
- `docs/modules/pages-page-builder-authenticated-authoring-route-packet-2026-08-06.md`;
- `docs/modules/pages-page-builder-inline-edit-asset-delivery-packet-2026-08-06.md`.

### Anonymous storefront boundary: source-ready

The public Pages route remains SSR-only. Retained source guards exclude Pages/Page Builder/Fly authoring dependencies and executable hydration/bootstrap markers from selected anonymous host profiles. The explicit built-artifact inspector remains source-ready; build and artifact inspection remain pending.

The authenticated inline and asset profiles are opt-in and are not part of the six retained anonymous dependency graphs. Anonymous public HTML does not reference the authoring bootstrap source.

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
| Pages authenticated grant + document save transport | Source-ready | Rust/SSR/WASM/browser/conflict execution pending |
| Authenticated authoring route mount | Source-ready | HTTP/browser execution pending |
| Dedicated authoring client export | Source-ready | WASM build and artifact inspection pending |
| Binary-embedded authoring asset router | Source-ready | Server build and HTTP `200`/`304` evidence pending |
| Release workflow/Docker integration | Open | Not implemented |
| Admin-owned launch link | Open | Not implemented |
| Artifact HTTP cache | Source-ready | SQLite/Axum execution pending |
| Native storefront route/cache/admission | Source-ready | Route-set execution pending |
| Selected immutable artifact vs draft body | Source-ready | Focused SQLite execution pending |
| Production generation gate and native route | Source-ready | Server execution pending |
| PostgreSQL retry after post-invalidation failure | Source-ready | PostgreSQL execution pending |
| Memory and OutboxLocal factory profiles | Source-ready | SQLite profile execution pending |
| Anonymous dependency graph | Source-ready | `cargo metadata` execution pending |
| Anonymous SSR document boundary | Source-ready | Source regression pending |
| Anonymous SSR built artifact | Inspector source-ready | Build and inspection pending |
| Anonymous Pages client bundle | Not mounted | Gate reopens only if public host introduces one |

## Boundaries

This slice changes only Pages-owned asset HTTP source, opt-in server feature composition and exact-lock client/server build orchestration around the existing authenticated consumer and route.

It does not:

- mount editing in the anonymous storefront or change the public Pages route;
- accept delegated or service principals;
- create a second Pages document persistence path;
- edit published immutable documents;
- change database schemas or migrations;
- add GraphQL or REST mutations;
- change publish, rollback, artifact, cache or event schemas;
- integrate release workflow, production Docker or admin navigation;
- claim tests, Cargo, formatting, verifiers, SSR/WASM builds, generated assets, server binaries, HTTP execution, browsers, dependency graphs, workflows, CI or rollout execution;
- promote FFA or FBA.

## Next cursor

1. Integrate `scripts/build/build-pages-inline-edit-server.sh` into deterministic release build and reproducibility jobs while retaining pinned actions and binary reproducibility.
2. Integrate the same exact-lock path into the production Docker builder.
3. Add an admin-owned launch link under a matching opt-in admin feature.
4. Run the asset-delivery, authenticated-route and consumer static guards plus focused Cargo/Rust checks.
5. Build the client and embedded server binary; retain JS/WASM/binary hashes, sizes and import evidence.
6. Prove same-origin `200`/`304`, MIME, ETag, CORP and production CSP behavior.
7. Execute direct-user allowed and anonymous/service/delegated/permission-denied HTTP cases.
8. Observe browser edit, save, replacement grant, stale revision, replay and expiry behavior.
9. Re-run anonymous dependency graph and built-artifact exclusion evidence.
10. Complete workflow and observed tenant rollout evidence before promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-inline-edit-asset-delivery.mjs
node crates/rustok-pages/scripts/verify/verify-pages-authenticated-authoring-route.mjs
node crates/rustok-pages/scripts/verify/verify-pages-authenticated-inline-consumer.mjs
node apps/storefront/scripts/build-pages-inline-edit-client.mjs --print-wasm-bindgen-version
bash scripts/build/build-pages-inline-edit-server.sh
cargo test -p rustok-pages --features inline-edit-assets --all-targets -- --nocapture
cargo test -p rustok-pages-storefront --features inline-edit,ssr --all-targets -- --nocapture
cargo check -p rustok-storefront --no-default-features \
  --features pages-inline-edit-hydrate --target wasm32-unknown-unknown
cargo check -p rustok-server --features pages-inline-edit-assets
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-ssr-delivery.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-authenticated-inline-edit-adapter.mjs
```

Execution evidence remains pending.
