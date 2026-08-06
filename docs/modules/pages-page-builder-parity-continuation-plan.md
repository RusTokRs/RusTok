# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-06  
Status: source-parity-current / authenticated-authoring-route-source-ready / inline-edit-asset-delivery-source-ready / admin-launch-source-ready / release-composition-source-ready / execution-browser-rollout-pending
Scope: `rustok-pages` admin/storefront FFA and `rustok-page-builder` document, publication, artifact, routing, cache, authenticated inline-authoring and deterministic deployment boundaries

## Source-of-truth policy

This is the canonical shared continuation cursor. Historical dated packets remain evidence for the source slices that produced the current state, but they do not override this plan.

`source-ready` means code, contracts, build source or retained harness source exists. It does not mean tests, Cargo, formatting, verifiers, databases, HTTP routes, server functions, Trunk, npm, WASM, native binaries, Docker images, browsers, workflows, CI or tenant rollout were executed.

Pages and Page Builder remain one vertical pipeline with explicit owners:

- Pages owns persistence, lifecycle, immutable bindings, localized route identity, cache policy, public reads, authenticated inline grants/save transport and the module-owned authoring asset HTTP contract.
- Pages admin owns the optional same-origin authoring launch control.
- Page Builder/Fly owns the reviewed document, sanitizer, runtime materialization, renderer, artifact producer contracts and reusable real-DOM inline adapter/session.
- Navigation and SEO own their resolved payloads.
- Hosts own route admission, CSP and HTTP composition, not Pages document, route, asset or launch policy.
- Release engineering owns deterministic composition and durable evidence, not runtime authorization or persistence.

Optional external event and delivery infrastructure remain outside the active Pages cursor.

## Rechecked merged cursor

Current `main` through PR #3063 contains the retained Pages/Page Builder sequence:

- #2955 — publish/rollback event correlation and generation contract;
- #2971, #2974 — PostgreSQL outbox/cache and durable relay restart source;
- #2979, #2985, #2988, #2990 — artifact HTTP cache, native storefront cache, registered route and channel admission;
- #2992, #3010 — reviewed immutable artifact authority after draft mutation;
- #2995, #2997, #3001, #3004, #3006, #3008 — relay continuity, production gate, native route, PostgreSQL retry and profile parity;
- #3011, #3014 — anonymous dependency and SSR delivery boundaries;
- #3016, #3018, #3020, #3026, #3029 — locale fallback, route aliases, host responses, tombstones and explicit history import;
- #3032 — exact Pages/Navigation/SEO private revalidation ETag;
- #3039 — reusable authenticated real-DOM adapter;
- #3049 — Pages signed inline grants and document-only save transport;
- #3056 — authenticated authoring route and target-gated client export source;
- #3060 — Pages-owned binary-embedded authoring assets and exact-lock client/server builder source;
- #3063 — Pages admin-owned same-origin launch source.

The present source slice adds one deterministic deployment composition owner and connects it to release build, release reproducibility and the production server Docker builder. It also aligns action pins with the existing allow-list, protects all inline-edit build owners behind `release-infra-approved`, and updates release readiness evidence requirements.

No build, workflow, Docker, HTTP or browser execution is claimed.

## Retained source marker index

- `public-list-locale-fallback-source-ready` — public detail/list locale fallback source-ready.
- `published-slug-route-alias-source-ready` — immutable published route aliases source-ready.
- `host-route-response-source-ready` — canonical/redirect/gone host response source-ready.
- `native-storefront-reviewed-artifact-source-ready` — immutable reviewed artifact authority source-ready.
- `native-storefront-channel-admission-source-ready` — channel admission before cache lookup source-ready.
- `selected-immutable-artifact-source-ready` — current draft body is not public authority.
- `production-relay-generation-gate-source-ready` — synchronous generation gate source-ready.
- `production-relay-native-route-source-ready` — gate-to-native-route composition source-ready.
- `production-gate-postgres-restart-source-ready` — PostgreSQL retry source-ready.
- `event-delivery-profile-parity-source-ready` — OutboxLocal/OutboxIggy parity source-ready.
- `anonymous-storefront-graph-source-ready` — anonymous dependency exclusion source-ready.
- `anonymous-storefront-ssr-delivery-source-ready` — anonymous SSR-only delivery source-ready.
- `delete-route-tombstone-source-ready` — delete tombstones source-ready.
- `route-history-import-source-ready` — explicit bounded route-history import source-ready.
- `storefront-composition-etag-source-ready` — exact Navigation/SEO/rendered HTML ETag source-ready.
- `authenticated-inline-adapter-source-ready` — reusable real-DOM adapter source-ready.
- `authenticated-inline-consumer-source-ready` — Pages grant and document-save consumer source-ready.
- `authenticated-authoring-route-source-ready` — authenticated module route source-ready.
- `inline-edit-asset-delivery-source-ready` — dedicated binary-embedded authoring assets source-ready.
- `inline-edit-admin-launch-source-ready` — admin launch source-ready.
- `inline-edit-release-composition-source-ready` — release/rebuild/Docker composition source-ready.

## Current parity state

### Metadata, reviewed publication and immutable rollback: source-complete

Draft workspaces and published Pages metadata share the registered consumer-property contribution. The bespoke metadata editor remains absent.

Pages remains the sole document persistence owner. Reviewed Page Builder materialization remains required for publish. Rollback selects a prior immutable manifest without compiling the current draft.

Database, GraphQL, REST, publish, rollback and event schemas are unchanged by the authenticated inline-authoring sequence.

Execution evidence remains pending.

### Public locale, route and cache authority: source-ready

Public detail/list resolve requested locale → tenant default → platform fallback. Published slug aliases, delete tombstones, explicit history import, canonical/redirect/gone responses and exact Pages/Navigation/SEO ETags remain Pages-owned.

The selected immutable published artifact remains public render authority after draft mutation.

Execution evidence remains pending.

### Reusable authenticated real-DOM adapter: source-ready

`fly-leptos` owns the bounded real-DOM interaction adapter. `rustok-page-builder-storefront` owns the canonical patch session. Provider-owned, composite, templated, interactive and runtime-owned subtrees remain read-only. Changed eligible text becomes one `EditorCommand::Patch` only after consumer authorization.

Execution evidence remains pending.

### Pages authenticated inline consumer: source-ready

Pages grants bind tenant, direct user, authenticated session, fresh edit session, channel, Pages page, stable Fly page, exact locale, revision, project hash and expiry.

Bootstrap/commit require direct user/session, tenant capability, `pages:update`, unpublished exact-locale GrapesJS body and stable Fly identities. Commit ends at `PageService::save_document(expected_revision)` and returns a fresh grant.

No second persistence path exists.

Execution evidence remains pending.

### Authenticated authoring route: source-ready

The existing storefront module route owner registers:

```text
/modules/pages-authoring?page_id=<pages UUID>&lang=<locale>
/{locale}/modules/pages-authoring?page_id=<pages UUID>
```

Coarse admission requires direct user, non-nil session, effective `pages:update` and Pages module enablement. Exact owner-aware authorization remains downstream in Pages.

Authoring HTML and inline server-function responses use `private, no-store`. HTML also uses `noindex, nofollow, noarchive`. The existing nonce-backed CSP remains authoritative. No proof material enters DOM.

Client artifact build and browser execution remain pending as historical route-packet wording; source build and deployment composition are now ready, while actual artifacts and browser execution remain pending.

### Dedicated authoring asset delivery: source-ready

Fixed paths:

```text
/assets/pages-inline-edit-bootstrap.js
/assets/pages-inline-edit/rustok_storefront.js
/assets/pages-inline-edit/rustok_storefront_bg.wasm
```

Pages conditionally embeds the files into the server binary. The asset profile fails compilation when generated inputs are absent.

JavaScript/WASM use explicit MIME types, stable-path revalidation, SHA-256 ETags, exact/weak `If-None-Match`, `304` and same-origin CORP.

The client builder derives exact `wasm-bindgen` from `Cargo.lock`, rejects mismatched tooling, uses Cargo `--locked`, validates outputs and publishes atomically.

Dedicated authoring asset delivery: source-ready. HTTP and artifact execution remain pending.

### Admin-owned inline authoring launch: source-ready

Non-default feature chain:

```text
rustok-admin/pages-inline-edit-launch
└── rustok-pages-admin/inline-edit-launch
```

The control requires `RUSTOK_PAGES_INLINE_EDIT_ADMIN_SAME_ORIGIN=true`, reloads the selected page through the Pages admin transport, hides missing/published/locale-less documents, and builds only the fixed relative authoring route from canonical UUID and exact document locale.

Credentials, sessions, grants, proofs, arbitrary origins and signing material are absent from href and DOM. Route/server authorization remains authoritative.

Admin-owned inline authoring launch: source-ready. Render/navigation execution remains pending.

### Deterministic release composition: source-ready

Single source owner:

```text
scripts/build/build-pages-inline-edit-deployment.sh
```

Composition:

```text
exact Trunk embedded admin build
  + pages-inline-edit-launch
  + explicit same-origin acknowledgement
→ Cargo.lock-selected wasm-bindgen authoring client build
→ rustok-server --features pages-inline-edit-assets
→ bounded output validation
```

The same owner is called by:

- deterministic release build;
- independent reproducibility rebuild;
- production server Docker builder.

Standard embedded admin builds clear the same-origin acknowledgement. The development server container remains on that standard profile. Standalone admin Docker and runtime-only release Docker remain unchanged.

Cross-target Rust flags are isolated:

```text
RUSTOK_EMBEDDED_ADMIN_RUSTFLAGS
RUSTOK_PAGES_INLINE_EDIT_CLIENT_RUSTFLAGS
RUSTFLAGS
```

The release workflow retains signed tag validation, immutable release checks, archive reproducibility, SPDX generation, container provenance, SBOM/checksum attestations and exact five-asset publication.

All GitHub actions remain pinned to allow-listed full commit SHAs. Release/infrastructure/hardening counts are source-guarded.

The `release-infra-approved` gate now protects the common composition owner, embedded admin builder, dedicated client builder and server builder.

Release readiness now requires durable hashes, sizes, HTTP headers, browser behavior and rollout evidence. Source inspection does not satisfy readiness.

Execution evidence remains pending.

### Anonymous storefront boundary: source-ready

Anonymous default/CSR/hydrate/SSR profiles do not enable inline edit, authoring assets or admin launch. Public Pages HTML remains SSR-only and does not reference authoring bootstrap/client/WASM.

Graph and built-artifact execution evidence remain pending.

## Parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Registered metadata and owner port | Complete | Browser/conflict execution pending |
| Reviewed publish and immutable rollback | Complete | DB/runtime execution pending |
| Public locale fallback | Source-ready | Native/GraphQL execution pending |
| Published aliases, tombstones and history import | Source-ready | SQLite/PostgreSQL/host execution pending |
| Host canonical/redirect/gone response | Source-ready | HTTP/SSR execution pending |
| Navigation/SEO composition ETag | Source-ready | Conditional request/browser execution pending |
| Reusable real-DOM adapter | Source-ready | Rust/WASM/browser execution pending |
| Pages signed grant and document save transport | Source-ready | Auth/conflict/replay execution pending |
| Authenticated authoring route | Source-ready | HTTP/browser execution pending |
| Dedicated authoring client export | Source-ready | WASM artifact evidence pending |
| Binary-embedded asset router | Source-ready | Server build and HTTP evidence pending |
| Admin-owned same-origin launch | Source-ready | Admin artifact/navigation evidence pending |
| Release build composition | Source-ready | Workflow execution pending |
| Reproducibility composition | Source-ready | Two-build digest evidence pending |
| Production server Docker composition | Source-ready | Docker build/digest evidence pending |
| Runtime-only release image | Unchanged/source-ready | Image execution pending |
| Anonymous dependency graph | Source-ready | `cargo metadata` execution pending |
| Anonymous SSR built artifact | Inspector source-ready | Build/inspection pending |
| Tenant rollout and FFA/FBA | Open | Not promoted |

## Historical compatibility markers

These exact phrases are retained only because earlier static guards consume the source snapshot wording:

- `Delete tombstones and historical backfill remain open` — superseded by tombstone and explicit import source.
- `Pages consumer grant issuance and document-only save mount remain open` — superseded by consumer source.
- `authenticated route mount remains open` — superseded by authenticated authoring route source.
- `release workflow and admin launch integration remain pending` — correct for PR #3060, superseded by admin-launch and release-composition source.
- `admin asset build integration remains pending` — correct for PR #3063, superseded by release-composition source.
- `release workflow integration remains pending` — correct for PR #3060, superseded by release-composition source.
- `admin-owned launch link remains pending` — correct for PR #3060, superseded by admin-launch source.
- `Authenticated authoring route: source-ready`.
- `Dedicated authoring asset delivery: source-ready`.
- `Admin-owned inline authoring launch: source-ready`.
- `client artifact build and browser execution remain pending`.

## Boundaries

This slice changes deterministic build and release composition source only.

It does not:

- change anonymous public Pages rendering;
- accept delegated or service principals;
- create another document persistence path;
- edit immutable published documents;
- change database, GraphQL, REST, event, publish, rollback, artifact or cache schemas;
- enable same-origin launch in standalone admin builds;
- add runtime build tooling to `apps/server/Dockerfile.release`;
- claim verifier, Cargo, npm, Trunk, WASM, server, Docker, workflow, HTTP, browser or rollout execution;
- promote FFA or FBA.

## Next cursor: execution evidence

1. Apply and review `release-infra-approved` for the protected source changes.
2. Run Pages consumer, route, asset, launch and release-composition static guards.
3. Run release infrastructure, supply-chain and readiness guards.
4. Run focused Cargo/tests for opt-in admin/storefront/server profiles.
5. Build the full composition twice in isolated target directories.
6. Retain embedded admin JS/WASM, authoring JS/WASM, server binary and archive hashes/sizes.
7. Confirm identical archive digest across build and reproducibility jobs.
8. Build production Docker and retain image digest.
9. Prove authoring asset `200`/`304`, MIME, ETag, cache, CORP and CSP behavior.
10. Prove launch visible/hidden states and exact-locale same-origin navigation.
11. Execute direct-user allowed and anonymous/service/delegated/permission-denied cases.
12. Observe edit/save/replacement grant/stale revision/replay/expiry behavior.
13. Re-run anonymous graph and built-artifact exclusion evidence.
14. Record tenant rollout and rollback evidence before promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-inline-edit-release-composition.mjs
node crates/rustok-pages/scripts/verify/verify-pages-inline-edit-admin-launch.mjs
node crates/rustok-pages/scripts/verify/verify-pages-inline-edit-asset-delivery.mjs
node crates/rustok-pages/scripts/verify/verify-pages-authenticated-authoring-route.mjs
node crates/rustok-pages/scripts/verify/verify-pages-authenticated-inline-consumer.mjs
node scripts/verify/verify-release-infra-self-test.mjs
node scripts/verify/verify-release-supply-chain-contract.mjs
node scripts/verify/verify-release-readiness-contract.mjs
bash scripts/build/build-pages-inline-edit-deployment.sh
```

Execution evidence remains pending.
