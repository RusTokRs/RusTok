# Implementation Plan for `rustok-pages`

Date: 2026-08-08  
Status: `in_progress / contribution-manifest-generation-source-ready / authenticated-authoring-route-source-ready / inline-edit-asset-delivery-source-ready / admin-launch-source-ready / release-composition-source-ready / artifact-repair-multilocale-recovery-source-ready / rollback-activated-artifact-loss-recovery-source-ready / rollback-activated-repair-rollback-continuity-source-ready / repeated-artifact-loss-recovery-source-ready / execution-rollout-pending`

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Evidence: `scripts/verify/verify-pages-ui-boundary.mjs` locks the UI boundary; no legacy editor is retained.

## Scope

Pages owns page identity, localization, document revisions, lifecycle, route history, immutable published bindings, artifact audit/rebuild/activation/rollback, caches, authenticated inline grants/save transport and the module-owned inline asset HTTP contract.

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
- enabling the same-origin admin launch in standalone/token-based admin builds;
- mutable current draft content as immutable artifact repair authority;
- automatic audit -> rebuild -> activation -> rollback chaining;
- provenance-only rollback targets without live immutable artifacts and their required historical manifest;
- a handwritten Pages Fly contribution descriptor tree beside canonical module metadata.

Fly remains the only visual document and command authority. Pages owns page identity, localization, document revisions, lifecycle, route history, immutable published bindings, artifact audit/rebuild/activation/rollback, caches, authenticated inline grants/save transport and the module-owned inline asset HTTP contract. Pages admin owns the optional same-origin launch control. Release engineering owns deterministic composition and evidence, not Pages document policy.

`source-ready` means code/contracts exist. It does not mean tests, Cargo, formatting, verifiers, databases, Trunk, npm, WASM, server binaries, Docker images, HTTP, browsers, workflows, CI or tenant rollout were executed.

## Current state

### Metadata, publication and persistence ownership

- Registered Pages metadata uses the shared consumer-property contribution.
- The bespoke metadata editor and direct workspace metadata write remain removed.
- `PageService::save_document` remains the only document-only persistence owner.
- Published Fly documents remain immutable without an explicit draft lifecycle.
- Reviewed Page Builder materialization remains the required publish path.
- Rollback selects a prior immutable manifest without compiling the current draft.
- Public/admin publication and rollback transports remain delegated to Pages owner services.

### Canonical Page Builder contribution metadata: source-ready

`rustok-module.toml` is now the single Pages source for the Page Builder contribution declaration. It owns the owner/target provider identity, contribution ids, capabilities, Fly landing block ids, messages, metadata property editor identity/accessibility and the full `page_builder_consumer_properties_v1` schema.

`admin/build.rs` validates and normalizes that metadata at build time. Owner version is derived from `[module].version`; exact target-provider versions are retained from the module manifest; `ownerProvider` and `providerVersion` are injected by generation and may not be hand-authored in contribution metadata.

The generator emits stable Rust constants plus the normalized `ModuleContributionManifest` JSON into `OUT_DIR`. `admin/src/contributions.rs` includes that generated source, lazily deserializes it and preserves the existing public helper API. The runtime has no TOML parser dependency and no handwritten `ModuleContributionManifest`, `ContributionDescriptor`, `PropertyEditorDescriptor` or consumer field tree.

This changes metadata authority only. Pages persistence, lifecycle, reviewed publication, rollback, recovery and cache ownership are unchanged.

The shared Page Builder plan tracks generalization of this Pages-specific build generator into reusable module tooling before a second production contribution consumer is connected.

### Immutable artifact integrity, rebuild, activation and rollback continuity

The Pages correlation gate follows `builder write -> Pages publish -> storefront
read`: a builder write remains an authenticated authoring action, reviewed
publication creates and binds immutable artifacts, and storefront read resolves
only the published bound artifact without requiring an authoring capability.

The reviewed publish transaction persists three independent authority layers:

```text
page_publish_operations
+ page_publish_operation_artifacts
+ page_publish_rebuild_sources
```

The immutable artifact itself remains public render authority. Retained rebuild provenance is deliberately independent of artifact-row lifetime so a tenant administrator can investigate and explicitly reproduce a lost immutable storage instance without reading mutable current draft content as repair authority.

The source-ready recovery chain is:

```text
bounded integrity audit
-> retained exact publish provenance
-> explicit append-only rebuild
-> explicit one-locale activation
-> publish-or-exact-rollback activation anchor
-> bounded missing-binding physical-loss recovery
-> sequential same-publish recovery across locales
-> repeated artifact-loss recovery using latest repair state per locale
-> repair-aware current rollback cursor with durable publish-or-rollback activation-lineage revalidation
```

Important boundaries:

- audit is read-only and tenant-wide `pages:manage`;
- rebuild appends `instance_key = rebuild:<operation-id>` and never changes the active binding, page version, events or caches;
- activation requires tenant-wide `pages:manage`, exact page version, exact rebuild receipt/provenance and exact expected historical source artifact id;
- an existing locale binding must still match the retained source body/artifact exactly and never falls through to loss recovery;
- missing-binding recovery requires the original source artifact to be physically absent, retained source body identity to remain present and the exact source publish operation to remain available;
- `expected_current_artifact_id` remains the historical source artifact identity on missing-binding recovery, even after a rebuilt replacement has been lost; it is a retained-provenance fence, not a claim that the source artifact was immediately bound;
- direct-publish recovery still admits the source publish `result_version` itself as the first activation anchor;
- if rollback made that exact publish set current again, the latest matching `page_rollback_operation` may instead anchor recovery only when tenant/page, target publish id, target artifact-set hash and canonical rollback request hash all revalidate exactly;
- rollback anchor request identity derives its original expected version as `result_version - 1`; a SHA-shaped but noncanonical receipt is rejected;
- a rollback receipt changes only the recovery version anchor and never replaces retained publish provenance as rebuild authority;
- any version gap after either anchor may be bridged only through a bounded contiguous sequence of prior activation receipts from that exact publish, with request/rebuild/provenance identity revalidated at every step;
- the activation lineage tracks the latest repair state per locale instead of requiring unique locales;
- a locale may repeat only after its previously activated rebuilt artifact has been physically lost; deleting only its binding while leaving the rebuilt row alive remains rejected;
- before a repeated activation, the target locale must remain unbound and its latest prior rebuilt instance must be absent;
- every non-target locale represented in the lineage must remain bound to its latest rebuilt artifact and that latest artifact must still match the receipt-bound instance/hash/materialization identity;
- the activation scan is physically capped at 257 rows while at most 256 sequential recovery steps are accepted;
- any unexplained page-version increment, including post-rollback drift, foreign publish, repeated locale with a still-live prior rebuilt artifact, changed latest non-target binding or drifted latest rebuilt artifact remains fail-closed;
- each activation changes one locale only, advances `pages.version` exactly once and writes `NodeUpdated` + `NodePublished` transactionally;
- cache effects remain event-driven after commit;
- rollback tries the original publish manifest first; repair fallback is current-cursor-only and requires exact retained provenance plus rebuild/activation receipts;
- rollback reconstruction recomputes canonical activation request hashes and proves the minimal contiguous physical-loss activation lineage needed to explain missing current manifest locales;
- rollback reconstruction resolves that lineage from the same publish-or-exact-rollback activation model as activation admission: direct current publish falls back to `publish.result_version`, while a later exact rollback-to-that-publish receipt must revalidate tenant/page, target publish id, target artifact-set hash, result-version bounds and canonical request hash before its `result_version` can become the lineage cursor;
- rollback reconstruction also tracks latest repair state per locale: a repeated locale requires the superseded rebuilt instance to be absent, and a missing-manifest locale is proven only when the prefix reaches the activation whose replacement artifact id equals the artifact currently bound for that locale;
- once every required current locale is proven, each latest rebuilt instance represented in the prefix must match the current repaired artifact set and its receipt;
- a missing or corrupted rollback anchor therefore falls back to the old publish cursor and fails closed when the durable repair activation actually began after rollback;
- surviving current manifest rows must still match retained provenance;
- a repaired current locale may lack its manifest row only when its historical source artifact is also absent;
- historical rollback targets still require their original manifest and live immutable artifact records;
- no repair command automatically triggers the next command.

No new schema is required. Existing `page_rollback_operations` provide the exact later rollback activation anchor, while `page_artifact_rebuild_operations` and `page_artifact_binding_replacement_operations` retain append-only rebuild/activation lineage without foreign keys to immutable artifact rows. Their version indexes are sufficient to prove a bounded repeated-loss chain after artifact-row loss.

Source packets:

- `contracts/evidence/pages-explicit-artifact-binding-replacement-source.json`;
- `contracts/evidence/pages-multilocale-repair-rollback-evidence-source.json`;
- `contracts/evidence/pages-rollback-activated-artifact-loss-recovery-source.json`;
- `contracts/evidence/pages-rollback-activated-repair-rollback-continuity-source.json`;
- `contracts/evidence/pages-repeated-artifact-loss-recovery-source.json`;
- `docs/explicit-immutable-artifact-loss-activation-recovery.md`;
- `tests/artifact_loss_activation_recovery_postgres.rs`;
- `tests/artifact_loss_multilocale_activation_recovery_postgres.rs`;
- `tests/artifact_repair_rollback_continuity_postgres.rs`;
- `tests/artifact_multilocale_repair_rollback_evidence_postgres.rs`;
- `tests/artifact_loss_after_rollback_activation_recovery_postgres.rs`;
- `tests/artifact_rollback_activated_repair_rollback_continuity_postgres.rs`;
- `tests/artifact_repeated_loss_recovery_postgres.rs`;
- `scripts/verify/verify-pages-explicit-artifact-binding-replacement.mjs`;
- `scripts/verify/verify-pages-artifact-loss-activation-recovery-postgres.mjs`;
- `scripts/verify/verify-pages-artifact-loss-multilocale-activation-recovery-postgres.mjs`;
- `scripts/verify/verify-pages-artifact-repair-rollback-continuity.mjs`;
- `scripts/verify/verify-pages-multilocale-repair-rollback-evidence.mjs`;
- `scripts/verify/verify-pages-rollback-activated-artifact-loss-recovery.mjs`;
- `scripts/verify/verify-pages-rollback-activated-repair-rollback-continuity.mjs`;
- `scripts/verify/verify-pages-repeated-artifact-loss-recovery.mjs`.

Execution remains unvalidated. The source packets do not claim that PostgreSQL, SQLite, request, cache or browser scenarios passed.

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
- `NodeUpdated`, `NodePublished`, `NodeUnpublished` and `NodeDeleted` drive bounded generation rotation for route/page/artifact caches after commit.
- Publish, rollback and repair activation reuse the same event-driven cache boundary; no repair path writes cache state inline.

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

Release, infrastructure and hardening workflows use the allow-listed full action SHAs. The `release-infra-approved` policy protects the common orchestrator, both downstream builders and the dedicated client builder. Release readiness requires hashes, sizes, HTTP and browser evidence rather than source inspection alone.

No release workflow, reproducibility job, Docker build or artifact was executed in this source slice.

### Anonymous storefront boundary: source-ready

Authenticated inline, asset and admin-launch profiles remain non-default. Anonymous default/CSR/hydrate/SSR profiles do not enable them. Public Pages HTML does not reference the authoring bootstrap.

Dependency graph and built-artifact execution evidence remain pending.

## Source evidence

- `rustok-module.toml`
- `admin/build.rs`
- `admin/src/contributions.rs`
- `scripts/verify/verify-pages-metadata-properties.mjs`
- `../../scripts/verify/verify-fly-ui-contributions.mjs`
- `src/services/page/inline_edit.rs`
- `src/services/page/inline_edit_feature.rs`
- `src/services/page/inline_edit_runtime.rs`
- `src/services/page/artifact_integrity_audit.rs`
- `src/services/page/artifact_rebuild.rs`
- `src/services/page/artifact_binding_replacement.rs`
- `src/services/page/artifact_set.rs`
- `src/services/page/rollback.rs`
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
- `contracts/evidence/pages-explicit-artifact-binding-replacement-source.json`
- `contracts/evidence/pages-multilocale-repair-rollback-evidence-source.json`
- `contracts/evidence/pages-rollback-activated-artifact-loss-recovery-source.json`
- `contracts/evidence/pages-rollback-activated-repair-rollback-continuity-source.json`
- `contracts/evidence/pages-repeated-artifact-loss-recovery-source.json`

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

## Milestones

### Remaining Pages work: execution evidence only

Shared cross-module contribution-generator generalization is tracked in the central Page Builder plan; the Pages reference consumer source boundary is complete in this plan.

### P0 — artifact repair and rollback evidence

- [ ] Run the explicit artifact binding replacement source guard.
- [ ] Run the single-locale physical-loss activation PostgreSQL packet.
- [ ] Run the multi-locale sequential physical-loss activation PostgreSQL packet.
- [ ] Run the rollback-activated physical-loss recovery source guard and PostgreSQL packet.
- [ ] Run the repeated artifact-loss recovery source guard and PostgreSQL packet.
- [ ] Retain the repeated-locale live-prior-rebuilt-artifact rejection and latest-state-other-locale success cases.
- [ ] Retain the rollback-anchor request-hash and unexplained post-rollback version-drift negatives.
- [ ] Run the repair-to-rollback continuity PostgreSQL packet after a repaired current set.
- [ ] Run the multi-locale repair-to-rollback durable-evidence packet.
- [ ] Run the rollback-activated repair-to-rollback continuity source guard and three-publish PostgreSQL packet.
- [ ] Retain rollback success after repeated recovery plus corrupted rollback-anchor rejection during repaired-current rollback reconstruction.
- [ ] Retain historical-target missing-manifest, current-manifest corruption/source-present, noncanonical activation request-hash and noncontiguous-prefix negatives.
- [ ] Run prior provenance, audit, rebuild, repair atomicity/failure/cache and transport packets.

### P1 — protected source review and focused validation

- [ ] Apply and review the required `release-infra-approved` label for the protected workflow/build changes.
- [ ] Review the exact action pins, occurrence counts and base-owned approval behavior.
- [ ] Run Fly/Page Builder contribution metadata guards.
- [ ] Run Pages inline consumer, route, asset, launch and release-composition static guards.
- [ ] Run release infrastructure, supply-chain and readiness guards.
- [ ] Run focused Cargo checks/tests for Pages admin/storefront/server profiles.
- [ ] Re-run anonymous dependency graph and built-artifact exclusion checks.
- [ ] Run metadata revision/isolation and cache continuity packets.

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

No tests, static verifiers, formatting, Cargo checks, npm installs, Trunk builds, WASM builds, native builds, Docker builds, HTTP hosts, browsers, dependency graphs, PostgreSQL scenarios, workflows or CI were executed by the implementation agent.

Suggested commands, intentionally not run:

```bash
node scripts/verify/verify-fly-ui-contributions.mjs
node crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-binding-replacement.mjs
node crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-activation-recovery-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-multilocale-activation-recovery-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-repeated-artifact-loss-recovery.mjs
node crates/rustok-pages/scripts/verify/verify-pages-artifact-repair-rollback-continuity.mjs
node crates/rustok-pages/scripts/verify/verify-pages-multilocale-repair-rollback-evidence.mjs
node crates/rustok-pages/scripts/verify/verify-pages-rollback-activated-artifact-loss-recovery.mjs
node crates/rustok-pages/scripts/verify/verify-pages-rollback-activated-repair-rollback-continuity.mjs
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... cargo test -p rustok-pages --test artifact_loss_activation_recovery_postgres -- --nocapture
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... cargo test -p rustok-pages --test artifact_loss_multilocale_activation_recovery_postgres -- --nocapture
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... cargo test -p rustok-pages --test artifact_repeated_loss_recovery_postgres -- --nocapture
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... cargo test -p rustok-pages --test artifact_repair_rollback_continuity_postgres -- --nocapture
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... cargo test -p rustok-pages --test artifact_multilocale_repair_rollback_evidence_postgres -- --nocapture
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... cargo test -p rustok-pages --test artifact_loss_after_rollback_activation_recovery_postgres -- --nocapture
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... cargo test -p rustok-pages --test artifact_rollback_activated_repair_rollback_continuity_postgres -- --nocapture
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


## Verification

- `cargo test -p rustok-pages`
- `cargo xtask module validate pages`

## Change rules

All Pages visual authoring contracts must adhere to Fly visual document authority without legacy fallback editors.
