# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-08  
Status: source-parity-current / pages-repeated-artifact-loss-recovery-source-ready / provider-degraded-controls-source-ready / contribution-registry-version-parity-source-ready / observed-health-open / module-metadata-generation-open / execution-browser-rollout-pending
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

## 2026-08-08 current source reconciliation

This section overrides older source-state/cursor wording retained below for compatibility with historical static guards.

The recheck includes all relevant merged source after the former PR #3063 cursor, especially:

- PR #3191 — repeated physical loss of rebuilt immutable Pages artifacts can recover again from the latest accepted repair state per locale while preserving bounded repair/rollback lineage; execution remains open;
- PR #3196 — Page Builder admin provider rollout/degraded controls are connected through the Pages consumer; absent live SLO evidence remains explicitly `unobserved` and no health state is fabricated;
- the existing `fly-ui` contribution path already has separate admin/storefront factories, tenant/permission/capability/provider-policy/provider-health filtering, and duplicate/missing-provider/missing-dependency/cycle diagnostics;
- the contribution static guard still required owner/target version parity while the live Rust manifest model had regressed to an unversioned provider set.

The current source slice restores that version boundary without changing Pages persistence or publication ownership:

- `ModuleContributionManifest` carries an exact `owner_version` and exact target-provider version map;
- manifest-routed contributions must declare `metadata.providerVersion`;
- missing/invalid provider versions fail closed;
- allowed provider names with the wrong version fail with `contribution_target_provider_version_mismatch`;
- Pages pins `rustok.pages@<crate version>` and `fly.builtin@1` while retaining its existing landing-block and executable metadata-property contributions.

Rechecked Page Builder Phase 9 source state:

- [x] Separate admin/storefront factories.
- [ ] Generate complete contribution manifests directly from canonical module metadata; Pages still builds its contribution manifest in module-owned Rust and only cross-checks selected capability metadata against `rustok-module.toml`.
- [x] Filter by tenant, permission, capability, provider policy and health.
- [x] Duplicate, missing-provider, missing-dependency, cycle and provider-version diagnostics.

The next source cursor is therefore module-metadata generation for contribution manifests. Real provider health remains a separate open composition/runtime cursor because the repository still has no authoritative live Page Builder SLO observation source. Execution evidence remains maintainer-owned.

Detailed evidence for this reconciliation is retained in `docs/modules/pages-page-builder-contribution-parity-actualization-2026-08-08.md`.

## Rechecked merged cursor

The following #2955–#3063 list is a retained historical snapshot; the 2026-08-08 reconciliation above is authoritative for current source state.

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
- `event-delivery-profile-parity-source-ready` — Outbox/OutboxIggy parity source-ready.
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
- `pages-repeated-artifact-loss-recovery-source-ready` — repeated physical loss of a rebuilt immutable artifact can recover from the latest accepted repair lineage.
- `provider-degraded-controls-source-ready` — rollout/degraded provider controls are connected without fabricating observed health.
- `contribution-registry-version-parity-source-ready` — contribution owner/target provider versions are pinned and fail closed on missing/mismatched versions.

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

### Repeated immutable-artifact loss recovery: source-ready

Pages keeps repair identity and latest accepted repair state per locale bounded. A previously rebuilt immutable artifact that is physically lost again can be reconstructed from the latest accepted repair lineage, and rollback continuity uses the same authority. This remains source-ready only until maintainer execution evidence is produced.

### Provider degraded controls and observed health: source-ready / observation-open

Page Builder provider flags can only narrow an already authorized capability set. Invalid/disabled builder state is unavailable, partial rollout or degraded health suppresses publish, and Pages exposes the same flags used by server composition. No live SLO source exists yet, so Pages health remains explicitly `unobserved`.

### Contribution registry version parity: source-ready / metadata-generation-open

Admin/storefront assembly, policy filters and structural diagnostics are source-ready. Owner and target provider versions are exact and fail closed on missing/mismatched contribution metadata. Complete generation from canonical module metadata remains open; handwritten module-owned contribution manifests are not treated as generated authority.

## Parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Registered metadata and owner port | Complete | Browser/conflict execution pending |
| Reviewed publish and immutable rollback | Complete | DB/runtime execution pending |
| Repeated immutable-artifact loss recovery | Source-ready | Repair/rollback execution pending |
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
| Provider degraded controls | Source-ready | Live observed-health evidence pending |
| Version-pinned contribution registry | Source-ready | Metadata generation / execution pending |
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

The historical deployment slice below remains unchanged; the 2026-08-08 reconciliation additionally restores contribution version parity and updates the source cursor.

It does not:

- change anonymous public Pages rendering;
- accept delegated or service principals;
- create another document persistence path;
- edit immutable published documents;
- change database, GraphQL, REST, event, publish, rollback, artifact or cache schemas;
- enable same-origin launch in standalone admin builds;
- add runtime build tooling to `apps/server/Dockerfile.release`;
- claim verifier, Cargo, npm, Trunk, WASM, server, Docker, workflow, HTTP, browser or rollout execution;
- fabricate Page Builder provider-health observations;
- promote FFA or FBA.

## Next cursor

Source continuation:

1. Generate complete Fly contribution manifests from canonical module metadata instead of maintaining a parallel handwritten declaration.
2. Keep current Pages repair/recovery and Page Builder degraded-control source boundaries unchanged while that registry authority is centralized.
3. Connect real provider-health observation only after an authoritative Page Builder SLO source exists.

Maintainer-owned execution evidence remains pending for the earlier release, browser, database and recovery slices.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node scripts/verify/verify-fly-ui-contributions.mjs
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
