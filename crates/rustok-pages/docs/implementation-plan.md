# Implementation Plan for `rustok-pages`

Date: 2026-08-06  
Status: `in_progress / authenticated-authoring-route-source-ready / inline-edit-asset-delivery-source-ready / admin-launch-source-ready / release-composition-source-ready / execution-browser-rollout-pending`

## Policy: current code only

Pages keeps no legacy compatibility editor, component mirror, block table, shadow document authority or migration shim.

Forbidden:

- a JSON/CRUD editor beside Fly;
- the deleted Next/GrapesJS page-builder route;
- `frames[0].component` as a component-tree mirror;
- `PageBlock`, `BlockService`, `page_blocks` or block mutations;
- storefront block fallback rendering;
- UI access to raw transport adapters;
- host-owned Pages persistence, route-claim, cache-key, asset or document policy;
- direct DOM-to-database persistence;
- fallback signing secrets or unsigned inline-edit claims;
- moving bearer tokens, sessions, grants or proofs through authoring URLs or DOM attributes;
- enabling the same-origin admin launch in standalone/token-based admin builds.

Fly remains the only visual document and command authority. Pages owns page identity, localization, document revisions, lifecycle, route history, immutable published bindings, caches, authenticated inline grants/save transport and the module-owned inline asset HTTP contract. Pages admin owns the optional same-origin launch control. Release engineering owns deterministic composition and evidence, not Pages document policy.

`source-ready` means code/contracts exist. It does not mean tests, Cargo, formatting, verifiers, databases, Trunk, npm, WASM, server binaries, Docker images, HTTP, browsers, workflows, CI or tenant rollout were executed.

## Current source state

### Metadata, publication and persistence ownership

- Registered Pages metadata uses the shared consumer-property contribution.
- The bespoke metadata editor and direct workspace metadata write remain removed.
- `PageService::save_document` remains the only document-only persistence owner.
- Published Fly documents remain immutable without an explicit draft lifecycle.
- Reviewed Page Builder materialization remains the required publish path.
- Rollback selects a prior immutable manifest without compiling the current draft.
- Database, GraphQL, REST, publish, rollback and event schemas are unchanged by the inline-authoring slices.

### Exact Translation metadata target

`pages/page_metadata` is an owner-registered Translation pilot for exact
`title`, review-only `slug`, optional `meta_title`, and optional
`meta_description`. It does not include Fly/GrapesJS body content.

`page_translations.revision` provides target/source locale CAS while
`pages.version` provides the resource CAS. `PageService` applies a merged
exact-locale patch atomically, validates the localized slug against Pages
routing ownership, advances both revisions, emits the existing `NodeUpdated`
outbox event, records a content-free `pages_translation_changes` cursor entry,
and completes the shared owner-operation receipt in that transaction. Normal
metadata, lifecycle, reviewed-publish, rollback, and delete writes emit the
same cursor evidence. Archived Pages are readable as archived evidence but are
not listed for active translation work and reject apply.

Translation has no direct Pages table access and no runtime-locale fallback in
this target. Production enablement still requires retained PostgreSQL
migration, concurrent CAS, and change-cursor recovery evidence.

### Public locale, route and cache authority

- Public detail/list use requested locale → tenant default → platform fallback.
- Published slug aliases, delete tombstones and explicit historical import remain source-ready.
- Localized canonical routes and host canonical/redirect/gone decisions remain Pages-owned.
- Navigation and SEO owner payloads remain bound into the exact private revalidation ETag.
- The selected immutable published artifact remains public render authority after draft mutation.
- The anonymous public Pages route remains SSR-only and unchanged.

### Authenticated real-DOM adapter: source-ready

`fly-leptos` and `rustok-page-builder-storefront` own the feature-gated real-DOM buffer and canonical Fly patch session.

- only eligible static leaf text is editable;
- proof material is not written to DOM;
- unchanged focusout does not consume a grant;
- changed values become one canonical `EditorCommand::Patch` after consumer authorization.

### Pages authenticated inline consumer: source-ready

Pages owns versioned HMAC-SHA256 grants binding tenant, direct user, authenticated session, fresh edit session, channel, Pages page, stable Fly page, exact locale, revision, project hash and expiry.

Bootstrap and commit require:

- direct user principal;
- matching non-nil authenticated session;
- tenant capability `pages.builder.inline_edit.enabled`;
- effective `pages:update`;
- unpublished exact-locale GrapesJS body;
- stable Fly page/component identities.

Commit still ends at `PageService::save_document(expected_revision)` and returns a fresh replacement grant. The storefront transport does not write page-body rows.

### Authenticated authoring route and shell: source-ready

The existing storefront module route owner registers:

```text
/modules/pages-authoring?page_id=<pages UUID>&lang=<locale>
/{locale}/modules/pages-authoring?page_id=<pages UUID>
```

The route requires direct user, non-nil session, `pages:update` and Pages module admission before render. Exact owner-aware authorization remains downstream in Pages.

HTML and inline server-function responses use `private, no-store`. Authoring HTML also uses `X-Robots-Tag: noindex, nofollow, noarchive`. The outer nonce-backed CSP remains authoritative. No proof is written to DOM.

### Inline edit asset delivery: source-ready

Fixed same-origin paths:

```text
/assets/pages-inline-edit-bootstrap.js
/assets/pages-inline-edit/rustok_storefront.js
/assets/pages-inline-edit/rustok_storefront_bg.wasm
```

The Pages HTTP owner conditionally embeds these files into `rustok-server` under `rustok-pages/inline-edit-assets`. Missing generated files fail profile compilation.

The router uses explicit JavaScript/WASM MIME types, `public, max-age=0, must-revalidate`, SHA-256 ETags, exact/weak `If-None-Match`, `304` and `Cross-Origin-Resource-Policy: same-origin`.

The dedicated client builder uses Cargo `--locked`, resolves exact `wasm-bindgen` from `Cargo.lock`, rejects a mismatched CLI, validates outputs and atomically publishes the generated pair.

### Admin-owned inline edit launch: source-ready

Non-default features:

```text
rustok-pages-admin/inline-edit-launch
rustok-admin/pages-inline-edit-launch
```

The component renders only when the build explicitly sets:

```text
RUSTOK_PAGES_INLINE_EDIT_ADMIN_SAME_ORIGIN=true
```

It reloads the selected page through the existing Pages admin transport, hides missing/published/locale-less documents, uses the canonical non-nil UUID and exact translation/body locale, and emits only a relative encoded authoring URL.

Tokens, sessions, grants, proofs, arbitrary origins and signing material are absent from the href and DOM. Backend admission remains authoritative.

### Release composition: source-ready

The single source owner is:

```text
scripts/build/build-pages-inline-edit-deployment.sh
```

It composes:

```text
embedded admin with pages-inline-edit-launch and explicit same-origin acknowledgement
→ dedicated pages-inline-edit-hydrate JS/WASM
→ rustok-server with pages-inline-edit-assets
→ output validation
```

The same owner is used by:

- the deterministic release build;
- the independent reproducibility rebuild;
- the production builder in `apps/server/Dockerfile`.

The standard embedded admin build explicitly clears the same-origin acknowledgement. The development server container keeps that standard profile. The standalone admin Dockerfile and runtime-only `apps/server/Dockerfile.release` remain unchanged.

Cross-target flags are separated:

```text
RUSTOK_EMBEDDED_ADMIN_RUSTFLAGS
RUSTOK_PAGES_INLINE_EDIT_CLIENT_RUSTFLAGS
RUSTFLAGS
```

Admin WASM and dedicated authoring WASM do not inherit native linker flags. Native reproducibility flags are restored for the server binary.

Release, infrastructure and hardening workflows now use the allow-listed full action SHAs. The `release-infra-approved` policy protects the common orchestrator, both downstream builders and the dedicated client builder. Release readiness requires hashes, sizes, HTTP and browser evidence rather than source inspection alone.

No release workflow, reproducibility job, Docker build or artifact was executed in this source slice.

### Anonymous storefront boundary: source-ready

Authenticated inline, asset and admin-launch profiles remain non-default. Anonymous default/CSR/hydrate/SSR profiles do not enable them. Public Pages HTML does not reference the authoring bootstrap.

Dependency graph and built-artifact execution evidence remain pending.

## Source evidence

- `src/services/page/inline_edit.rs`
- `src/services/page/inline_edit_feature.rs`
- `src/services/page/inline_edit_runtime.rs`
- `src/http/inline_edit_assets.rs`
- `storefront/src/inline_edit.rs`
- `admin/src/inline_edit_launch.rs`
- `apps/storefront/src/modules/core.rs`
- `apps/storefront/scripts/build-pages-inline-edit-client.mjs`
- `scripts/build/build-embedded-admin.sh`
- `scripts/build/build-pages-inline-edit-server.sh`
- `scripts/build/build-pages-inline-edit-deployment.sh`
- `.github/workflows/release.yml`
- `apps/server/Dockerfile`
- `contracts/evidence/pages-authenticated-inline-consumer-source.json`
- `contracts/evidence/pages-authenticated-authoring-route-source.json`
- `contracts/evidence/pages-inline-edit-asset-delivery-source.json`
- `contracts/evidence/pages-inline-edit-admin-launch-source.json`
- `contracts/evidence/pages-inline-edit-release-composition-source.json`

## Historical source markers

These exact phrases remain only for retained static guard compatibility and describe earlier PR boundaries:

- `authenticated route mount remains open` — PR #3049 snapshot, superseded by the authenticated route source.
- `client artifact build and browser execution remain pending` — PR #3056 snapshot; source build/delivery composition is now ready, execution remains pending.
- `release workflow integration remains pending` — PR #3060 snapshot, superseded by release-composition source.
- `admin-owned launch link remains pending` — PR #3060 snapshot, superseded by admin-launch source.
- `admin asset build integration remains pending` — PR #3063 snapshot, superseded by release-composition source.
- `release workflow and admin launch integration remain pending` — PR #3060 snapshot, both source slices are now ready.
- `authenticated authoring route and shell: source-ready`.
- `inline edit asset delivery: source-ready`.
- `Admin-owned inline edit launch: source-ready`.
- `release-composition-source-ready`.

## Remaining work: execution evidence only

### P0 — protected source review

- [ ] Apply and review the required `release-infra-approved` label for the protected workflow/build changes.
- [ ] Review the exact action pins, occurrence counts and base-owned approval behavior.

### P1 — focused validation

- [ ] Run Pages inline consumer, route, asset, launch and release-composition static guards.
- [ ] Run release infrastructure, supply-chain and readiness guards.
- [ ] Run focused Cargo checks/tests for Pages admin/storefront/server profiles.
- [ ] Re-run anonymous dependency graph and built-artifact exclusion checks.

### P2 — deterministic artifacts

- [ ] Run `build-pages-inline-edit-deployment.sh` twice in isolated target directories.
- [ ] Retain embedded admin JS/WASM hashes and sizes.
- [ ] Retain dedicated authoring JS/WASM hashes and sizes.
- [ ] Retain native server binary and packaged archive hashes and sizes.
- [ ] Confirm the two release archives have the same digest.
- [ ] Build the production Docker target and retain its digest.

### P3 — HTTP and browser evidence

- [ ] Prove asset `200`/`304`, MIME, ETag, cache and CORP headers.
- [ ] Prove production CSP accepts the same-origin bootstrap/client/WASM path without global weakening.
- [ ] Prove launch visible/hidden states and exact-locale navigation.
- [ ] Execute direct-user allowed and anonymous/service/delegated/permission-denied cases.
- [ ] Observe edit, save, replacement grant, stale revision, replay and expiry behavior.
- [ ] Prove anonymous public Pages HTML does not reference or fetch authoring assets.

### P4 — rollout

- [ ] Record reviewed workflow runs and artifacts.
- [ ] Record tenant capability rollout and rollback evidence.
- [ ] Promote FFA/FBA only after observed evidence is accepted.

## Execution status

No tests, static verifiers, formatting, Cargo checks, npm installs, Trunk builds, WASM builds, native builds, Docker builds, HTTP hosts, browsers, dependency graphs, workflows or CI were executed by the implementation agent.

Suggested commands, intentionally not run:

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
