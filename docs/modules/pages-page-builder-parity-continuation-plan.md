# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-12  
Status: source-parity-current / pages-repeated-artifact-loss-recovery-source-ready / provider-degraded-controls-source-ready / provider-health-runtime-source-ready / provider-health-observed-acceptance-source-ready / contribution-registry-version-parity-source-ready / contribution-module-metadata-generation-source-ready / shared-contribution-tooling-source-ready / forum-runtime-composition-source-ready / forum-evidence-harness-source-ready / pages-reference-consumer-gate-source-ready / pages-reference-consumer-gate-acceptance-source-ready / pages-gate-owner-runner-synthetic-ready / forum-wave-admission-source-ready / forum-wave-admission-runner-synthetic-ready / forum-wave-live-lineage-source-ready / forum-wave-observed-owner-acceptance-source-ready / generic-editor-accessibility-source-ready / generic-accessibility-browser-packet-verifier-source-ready / execution-acceptance-pending
Scope: `rustok-pages` admin/storefront FFA and `rustok-page-builder` document, publication, artifact, routing, cache, authenticated inline-authoring and deterministic deployment boundaries

## Source-of-truth policy

This is the canonical shared continuation cursor. Historical dated packets remain evidence for the source slices that produced the current state, but they do not override this plan.

`source-ready` means code, contracts, build source or retained harness source exists. It does not mean tests, Cargo, formatting, verifiers, databases, HTTP routes, server functions, Trunk, npm, WASM, native binaries, Docker images, browsers, workflows, CI or tenant rollout were executed.

Pages and Page Builder remain one vertical pipeline with explicit owners:

- Pages owns persistence, lifecycle, immutable bindings, localized route identity, cache policy, public reads, authenticated inline grants/save transport and the module-owned authoring asset HTTP contract.
- Pages admin owns the optional same-origin authoring launch control and consumes build-generated contribution metadata.
- Platform build tooling owns the reusable parsing/normalization contract for canonical module contribution metadata; it does not own runtime registry policy.
- Forum owns Forum category/topic/reply lifecycle, visibility, widget contracts, widget validation and widget authorization even when its Page Builder contribution metadata and Fly identities are composed through shared tooling.
- Page Builder/Fly owns the reviewed document, sanitizer, runtime materialization, renderer, artifact producer contracts, reusable real-DOM inline adapter/session, canonical provider-health policy/observation/evaluator contracts and provider-neutral contribution host seams.
- Pages owns the fail-closed binding of maintainer-accepted deployment-health evidence into its consumer surfaces; missing, invalid or expired binding remains `unobserved`.
- Navigation and SEO own their resolved payloads.
- Hosts own route admission, CSP and HTTP composition, not Pages/Forum domain persistence, route policy, asset policy or launch policy.
- Release engineering owns deterministic composition and durable evidence, not runtime authorization or persistence.

Optional external event and delivery infrastructure remain outside the active Pages cursor.

## 2026-08-10 current source reconciliation

This section overrides older source-state/cursor wording retained below for compatibility with historical static guards. It is current through the 2026-08-12 recheck; the heading date is intentionally retained because existing source guards use it as a stable section marker.

The recheck includes all relevant merged source after the former PR #3063 cursor, especially:

- PR #3191 — repeated physical loss of rebuilt immutable Pages artifacts can recover again from the latest accepted repair state per locale while preserving bounded repair/rollback lineage; execution remains open;
- PR #3196 — Page Builder admin provider rollout/degraded controls are connected through the Pages consumer; absent validated deployment health remains explicitly `unobserved` and no health state is fabricated;
- the existing `fly-ui` contribution path already has separate admin/storefront factories, tenant/permission/capability/provider-policy/provider-health filtering, and duplicate/missing-provider/missing-dependency/cycle diagnostics;
- PR #3205 restored exact owner/target provider versions and fail-closed manifest-routed provider-version admission;
- PR #3215 moved the Pages reference-consumer declaration into canonical `rustok-module.toml` and generated the version-pinned runtime manifest at build time;
- PR #3222 moved generic contribution parsing/normalization into platform `rustok-build` tooling and reused it from Pages generation plus module publish readiness;
- PR #3227 onboarded Forum as the second production consumer through canonical shared contribution metadata;
- PR #3239 added the real Forum Fly component/block registry and `ContributionAdapter`;
- PR #3247 composed Forum-owned preview reads through provider-neutral Page Builder host ports on the real Pages admin route;
- PR #3254 completed owner-backed Forum widget property schema/validation and normalized Fly property patching;
- PR #3264 retained the exact-source Forum browser evidence harness without executing it;
- PR #3266 retained direct Forum runtime authorization/visibility evidence source without executing it;
- PR #3274 retained deployed native server-function attestation source without executing it;
- PR #3320 made the existing `pages_reference_consumer_gate` blocker explicit as a machine-readable fail-closed source contract and bound Forum Wave source evidence to it;
- PRs #3389, #3391, #3395 and #3399 added bounded provider Preview/Publish observation, deployment-aggregatable metrics/freshness, exact source/deployment identity plus expected target inventory, and the reset-aware deployment evaluator. Process-local samples are not deployment authority;
- PRs #3403, #3410, #3414, #3417 and #3420 added typed Pages provider-health transport, binding-owner acceptance with exact remaining `health_valid_until`, fail-closed server binding, health-aware workspace/SSR/browser-intent consumers and non-mutating capability preflight using shared effective runtime flags;
- PRs #3424 and #3426 retained the exact-source observed-health runtime harness and explicit retrospective observed-evidence owner acceptance without executing them or asserting current provider health;
- PR #3429 added explicit Pages reference-consumer gate acceptance source over the rollout-only candidate plus owner-accepted observed-health evidence, with separate owner and rollback decisions;
- PR #3435 added Forum Wave admission source requiring an accepted Pages gate plus exact-source Forum browser/runtime/server-function evidence before the observed control-plane Wave may start;
- PR #3444 closed the generic Page Builder editor source-level accessibility gap with programmatic field names, scoped repeated-action names, selected-state semantics and `scripts/verify/verify-page-builder-admin-accessibility.mjs`;
- PR #3453 added executable native SSR rendered-DOM evidence for selected-page state, add-page naming and disabled capability fieldsets;
- PR #3456 retained the exact-source/deployment Playwright accessibility browser harness for keyboard/focus/accessibility-tree/read-only semantics without fabricating a deployed run;
- PR #3458 added a fail-closed verifier plus synthetic cases for future retained accessibility browser packets while keeping screen-reader execution and WCAG conformance unclaimed;
- PR #3459 added executable synthetic fail-closed coverage for the observed provider-health owner-acceptance runner and repaired stale source-guard pointers;
- PR #3460 added executable synthetic fail-closed coverage for the Pages reference-consumer gate owner-decision runner;
- PR #3461 added executable synthetic fail-closed coverage for the Forum Wave admission runner;
- PR #3464 added a live-Wave lineage verifier that binds a future observed control-plane packet to the exact retained admission packet and its source/RepoDigest;
- PR #3465 added the separate retrospective Forum observed-Wave owner-acceptance source/runner. Accepted output is promotion-review input only and does not itself promote FFA/FBA.

The current Forum contribution/runtime source boundary is complete without changing Forum persistence, visibility, widget validation or authorization ownership:

- `crates/rustok-forum/rustok-module.toml` declares complementary `rustok.forum.widget-catalog` and `rustok.forum.widget-preview` contributions through the shared `[fba.builder_consumer.contribution_manifest]` shape;
- `forum.topic_list`, `forum.topic_detail` and `forum.reply_stream` have real Fly block/component identities;
- owner schema references remain metadata-only while actual schemas and validation stay behind `ForumWidgetContractService`;
- `adapter_state = "fly_contract_ready"`, `preview_data_state = "owner_preview_transport_ready"` and `property_data_state = "owner_property_editor_ready"` are the current source states;
- preview renderer admission is capability-separated from tree/property admission so `preview_off` does not erase otherwise-authorized property editing;
- Forum owner preview rechecks tenant/module/RBAC/visibility and performs bounded owner reads;
- owner-backed property editing validates only through the Forum owner contract and patches normalized object props through ordinary Fly history;
- browser, runtime-authorization and deployed-attestation harnesses exist but remain unexecuted;
- `pages_reference_consumer_gate` source remains fail closed, while its explicit acceptance source and synthetic production-runner coverage are ready and still require maintainer candidate + observed-health execution and owner/rollback decisions;
- Forum Wave admission source and synthetic production-runner coverage are ready;
- a future live observed Wave must pass exact retained-admission lineage plus freshness verification before the separate observed-Wave owner decision;
- the observed-Wave owner-acceptance source is ready, but live Pages/Forum/Wave execution and the real owner decision remain maintainer-owned; accepted owner evidence is only eligible input for a separate FFA/FBA promotion review.

Rechecked Page Builder Phase 9/rollout source state:

- [x] Separate admin/storefront factories.
- [x] Generate the complete Pages reference-consumer contribution manifest from canonical module metadata at build time.
- [x] Filter by tenant, permission, capability, provider policy and health.
- [x] Duplicate, missing-provider, missing-dependency, cycle and provider-version diagnostics.
- [x] Generalize canonical contribution metadata parsing/normalization into platform build tooling and module publish validation.
- [x] Onboard Forum as the second production consumer to canonical shared contribution metadata.
- [x] Define and connect the real Forum Fly component/block/adapter runtime.
- [x] Connect Forum owner preview through provider-neutral Page Builder host ports.
- [x] Connect Forum owner-backed property editing without moving owner validation/persistence into Page Builder.
- [x] Retain Forum browser/runtime/deployment evidence harness source without claiming execution.
- [x] Define the machine-readable Pages reference-consumer gate source contract.
- [x] Implement provider-health observation, metrics/freshness, exact deployment identity, evaluator, Pages binding/consumer narrowing, runtime harness and observed-evidence owner-acceptance source.
- [x] Define explicit Pages reference-consumer gate acceptance source over rollout candidate + accepted observed health with owner/rollback decision.
- [x] Exercise provider-health and Pages gate owner-decision runners against fail-closed synthetic exact-checkout packets.
- [x] Define Forum Wave admission source over accepted Pages gate + exact Forum evidence lineage.
- [x] Exercise the Forum Wave admission runner against fail-closed synthetic exact-checkout packets.
- [x] Define exact retained-admission lineage verification for a future live observed Forum Wave.
- [x] Define the separate retrospective Forum observed-Wave owner acceptance source/runner without promotion side effects.
- [x] Complete generic typed editor control programmatic accessibility semantics and retain a source anti-drift guard.
- [x] Retain native SSR accessibility evidence, a deployed browser harness and a fail-closed retained-browser-packet verifier source.
- [ ] Execute generic editor full/read-only browser evidence against the reviewed exact deployment, verify the retained packet and perform owner review; screen-reader evidence remains separate if retained by the project.
- [ ] Execute exact provider-health deployment/evidence/owner-decision packets.
- [ ] Execute the rollout-only reference candidate and take the Pages gate owner + rollback decision.
- [ ] Execute Forum browser/runtime/deployment evidence, run Forum Wave admission, retain the separate observed control-plane Wave, pass freshness + exact admission lineage and take the retrospective Wave owner decision.
- [ ] Only after accepted observed-Wave owner evidence, perform a separate explicit FFA/FBA promotion review.

No new provider-health, Pages gate, Forum-admission, retained-Wave-lineage or observed-Wave-owner-decision source architecture gap is identified by this reconciliation. The repository now has an authoritative **source architecture** for exact-target deployment-health observation/evaluation/binding and consumer narrowing. Current deployment health is still not asserted by source inspection: maintainers must execute the exact target chain, accepted health can expire, and missing/invalid/expired binding remains `unobserved`.

The generic editor source accessibility gap is also closed. Native SSR evidence, browser-harness source and retained-packet verification source exist; the real browser packet, owner review and optional separate screen-reader evidence remain execution work. Static/source/synthetic evidence does not establish WCAG conformance.

Detailed evidence for the current contribution, provider-health, gate, Forum admission and editor-accessibility slices is retained in:

- `docs/modules/pages-page-builder-contribution-parity-actualization-2026-08-08.md`;
- `docs/modules/pages-page-builder-module-metadata-contribution-generation-2026-08-08.md`;
- `docs/modules/pages-page-builder-shared-contribution-tooling-2026-08-08.md`;
- `docs/modules/forum-page-builder-contribution-metadata-actualization-2026-08-08.md`;
- `docs/modules/forum-page-builder-fly-adapter-actualization-2026-08-08.md`;
- `docs/modules/forum-page-builder-owner-preview-actualization-2026-08-08.md`;
- `docs/modules/forum-page-builder-owner-properties-actualization-2026-08-08.md`;
- `docs/modules/forum-page-builder-browser-evidence-harness-actualization-2026-08-08.md`;
- `docs/modules/forum-page-builder-runtime-authorization-evidence-actualization-2026-08-08.md`;
- `docs/modules/forum-page-builder-serverfn-deployment-attestation-actualization-2026-08-08.md`;
- `docs/modules/pages-page-builder-reference-consumer-gate-actualization-2026-08-08.md`;
- `docs/modules/page-builder-provider-health-runtime-observation-actualization-2026-08-09.md`;
- `docs/modules/page-builder-provider-health-deployment-metrics-actualization-2026-08-09.md`;
- `docs/modules/page-builder-provider-health-deployment-identity-actualization-2026-08-09.md`;
- `docs/modules/page-builder-provider-health-deployment-evaluator-actualization-2026-08-09.md`;
- `docs/modules/pages-page-builder-provider-health-transport-actualization-2026-08-09.md`;
- `docs/modules/pages-page-builder-provider-health-owner-acceptance-actualization-2026-08-09.md`;
- `docs/modules/pages-page-builder-provider-health-server-binding-actualization-2026-08-09.md`;
- `docs/modules/pages-page-builder-provider-health-consumer-binding-actualization-2026-08-09.md`;
- `docs/modules/pages-page-builder-provider-health-capability-preflight-actualization-2026-08-09.md`;
- `docs/modules/pages-page-builder-provider-health-runtime-evidence-harness-actualization-2026-08-09.md`;
- `docs/modules/pages-page-builder-provider-health-observed-acceptance-actualization-2026-08-10.md`;
- `docs/modules/pages-page-builder-reference-consumer-gate-acceptance-actualization-2026-08-10.md`;
- `docs/modules/forum-page-builder-wave-admission-actualization-2026-08-10.md`;
- `docs/modules/pages-page-builder-base-plan-reconciliation-actualization-2026-08-10.md`;
- `docs/modules/page-builder-admin-accessibility-actualization-2026-08-10.md`;
- `docs/modules/pages-page-builder-parity-accessibility-actualization-2026-08-12.md`;
- `docs/modules/forum-page-builder-wave-observed-acceptance-actualization-2026-08-12.md`.

## Rechecked merged cursor

The following #2955–#3063 list is a retained historical snapshot; the current reconciliation above is authoritative for current source state.

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

The historical release-composition source slice adds one deterministic deployment composition owner and connects it to release build, release reproducibility and the production server Docker builder. It also aligns action pins with the existing allow-list, protects all inline-edit build owners behind `release-infra-approved`, and updates release readiness evidence requirements.

No build, workflow, Docker, HTTP or browser execution is claimed by source inspection.

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
- `provider-health-runtime-source-ready` — bounded observation through exact-target evaluator, accepted binding and consumer narrowing source exists; execution/current health remain unclaimed.
- `provider-health-observed-acceptance-source-ready` — observed runtime evidence has an explicit retrospective owner-decision source without extending freshness.
- `contribution-registry-version-parity-source-ready` — contribution owner/target provider versions are pinned and fail closed on missing/mismatched contribution metadata.
- `contribution-module-metadata-generation-source-ready` — Pages contribution declarations and property schema are generated from canonical module metadata at build time.
- `shared-contribution-tooling-source-ready` — canonical contribution parsing/normalization is shared by platform build tooling, Pages generation and module publish readiness.
- `forum-second-consumer-discovery-source-ready` — historical discovery checkpoint; superseded by the runtime-composition markers below.
- `forum-fly-adapter-source-ready` — Forum Fly component/block registration and `ContributionAdapter` are source-ready.
- `forum-owner-preview-source-ready` — Forum owner preview transport and Pages host composition are source-ready.
- `forum-owner-properties-source-ready` — Forum owner-backed property schema/validation and normalized Fly patching are source-ready.
- `forum-evidence-harness-source-ready` — Forum browser/runtime/deployment evidence harnesses exist but remain unexecuted.
- `pages-reference-consumer-gate-source-ready` — rollout-only blocker source is explicit and fail-closed; committed acceptance remains false.
- `pages-reference-consumer-gate-acceptance-source-ready` — dual-input owner/rollback decision source exists over reference candidate + accepted observed health.
- `pages-gate-owner-runner-synthetic-ready` — production Pages gate owner runner has executable fail-closed synthetic exact-checkout coverage; real gate evidence/decision remain pending.
- `forum-wave-admission-source-ready` — accepted Pages gate and exact Forum evidence correlation source exists before the observed control-plane Wave.
- `forum-wave-admission-runner-synthetic-ready` — production Forum admission runner has executable fail-closed synthetic exact-checkout coverage; real admission remains pending.
- `forum-wave-live-lineage-source-ready` — a future live observed Wave must bind to the actual retained admission packet hash/source/RepoDigest before owner review.
- `forum-wave-observed-owner-acceptance-source-ready` — the retrospective observed-Wave owner-decision source exists; accepted evidence is promotion-review input only.
- `generic-editor-accessibility-source-ready` — generic editor field/action naming and selected-state semantics are source-ready and guarded.
- `generic-accessibility-browser-packet-verifier-source-ready` — deployed browser harness and fail-closed retained-packet verifier source exist; real browser/owner/screen-reader evidence remains pending.

## Current parity state

### Metadata, reviewed publication and immutable rollback: source-complete

Draft workspaces and published Pages metadata share the registered consumer-property contribution. The bespoke metadata editor remains absent.

Pages remains the sole document persistence owner. Reviewed Page Builder materialization remains required for publish. Rollback selects a prior immutable manifest without compiling the current draft.

Database, GraphQL, REST, publish, rollback and event schemas are unchanged by the contribution-generation/tooling slices.

Execution evidence remains pending.

### Canonical contribution metadata generation and shared tooling: source-ready

Pages owns one canonical contribution declaration in `rustok-module.toml`. Its build script materializes the version-pinned Fly manifest and metadata property schema into `OUT_DIR`; runtime consumes only generated Rust/JSON and retains no TOML dependency or handwritten descriptor tree.

Generic parsing, provider/version injection, capability admission and nested provider validation live once in `rustok-build` tooling and are reused by Pages build generation and `xtask` publish readiness. Forum is the second production consumer of that canonical metadata boundary and now also has real Fly component/block identities plus adapter, owner preview and owner property paths composed through provider-neutral Page Builder host seams.

Forum persistence, visibility, widget schemas, validation and authorization remain Forum-owned. Page Builder receives only the bounded contribution/preview/property contracts required for composition. Browser/runtime/deployment evidence harnesses exist but execution remains pending.

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

### Generic Page Builder editor accessibility: source-ready / execution-open

Generic asset/property/style/responsive-style/trait/page/palette/layer controls expose programmatic names and selected-state semantics at source level. Repeated actions are object-scoped rather than exposed only as generic `Use`, `Remove`, `Add` or `Drag` labels. The static guard rejects the previous placeholder-only and visual-only patterns.

Native SSR evidence now exercises selected-state/name/disabled semantics. The exact-source/deployment Playwright browser harness covers keyboard traversal/activation, accessibility-tree state/name inspection and read-only disabled semantics. A separate fail-closed verifier validates a future retained browser packet against checkout HEAD and an independently supplied reviewed RepoDigest. Real deployed browser execution, owner review and optional separate screen-reader evidence remain pending. No WCAG conformance is claimed by source, native SSR or synthetic verifier evidence.

### Provider degraded controls and deployment health: source-ready / execution-open

Page Builder provider flags can only narrow an already authorized capability set. Invalid/disabled builder state is unavailable, configured rollout can disable individual capabilities, Degraded observed health suppresses publish, and Unavailable observed health fails builder capabilities closed. Pages exposes the same configured flags used by server composition and consumes only validated accepted provider health through the fail-closed binding.

The source chain now exists from bounded process-local Preview/Publish observation through deployment metrics/freshness, exact target/source identity, reset-aware evaluator, owner acceptance, Pages server binding, typed transport, workspace/SSR/browser-intent narrowing, non-mutating capability preflight, observed-health runtime harness and retrospective owner acceptance. Current health is still a live fact: process-local samples alone are not deployment authority, accepted health can expire, and missing/invalid/expired binding is `unobserved`.

The production retrospective observed-health owner runner also has fail-closed synthetic exact-checkout coverage. That synthetic suite validates decision and lineage behavior only; it is not deployment/provider-health evidence and does not assert current health.

### Contribution registry version parity: source-ready

Admin/storefront assembly, policy filters and structural diagnostics are source-ready. Owner and target provider versions are exact and fail closed on missing/mismatched contribution metadata. Pages supplies those identities through canonical metadata and the shared build normalizer rather than a handwritten runtime manifest. Forum supplies canonical metadata plus real adapter/preview/property runtime source through the same owner-preserving composition boundary.

### Pages reference-consumer gate: source-ready / acceptance-source-ready / execution-open

`crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json` remains the rollout-only fail-closed blocker referenced by Forum Wave evidence. It requires the Pages `1.1` Page Builder contract, `all_on`, `publish_off`, `preview_off` and `builder_off` profiles, Pages-owned reads in every profile and the existing source guards plus maintainer execution evidence. That candidate branch intentionally keeps `provider_health = unobserved`.

`pages-reference-consumer-gate-acceptance-source.json` separately requires the exact reference candidate plus `pages_builder_provider_health_observed_acceptance_v1` on the same source commit and immutable RepoDigest, with explicit `accept_pages_reference_consumer_gate|reject` owner decision and explicit retain/rollback disposition. Committed source remains `accepted = false`; the decision source does not perform rollback, assert current provider health or promote Forum/FFA/FBA.

The production gate owner-decision runner now has fail-closed synthetic exact-checkout coverage. This proves the runner rejects owner/rollback/source-hash/argv/provider-health/RepoDigest and review-eligibility drift; it does not create a real candidate, accepted observed-health packet or owner decision.

### Forum Wave admission and observed-Wave review: source-ready / execution-open

`forum-page-builder-wave-admission-source.json` requires `pages_reference_consumer_gate_acceptance_v1 / owner_accepted_pages_reference_consumer_gate` plus the Forum browser, runtime-authorization and deployed server-function evidence packets on one exact checkout source. Deployment-bound packets must correlate to the same immutable RepoDigest while preserving the predecessor contracts' maintainer-reviewed, non-cryptographic deployment-identity boundary.

Successful admission produces only `forum_page_builder_wave_admission_v1 / forum_wave_inputs_admitted_observed_control_plane_pending`. The production admission runner has fail-closed synthetic exact-checkout coverage, but no live Forum admission is claimed.

A future live observed control-plane Wave remains a separate packet and must retain audit trail, fallback profiles, metrics/traces, rollback decision, approvals and waivers. Before owner review it must pass the production freshness verifier and the separate retained-admission lineage verifier, which opens the actual admission packet and rechecks its hash/source/RepoDigest rather than trusting a string carried by the Wave packet.

`forum_page_builder_wave_observed_acceptance_source_v1` then defines a retrospective explicit `accept_observed_wave_evidence|reject` decision. Accepted output is `forum_page_builder_wave_observed_acceptance_v1 / owner_accepted_observed_control_plane_wave_promotion_review_pending`; it does not mutate rollout/control-plane state, assert current provider health or promote FFA/FBA. Promotion remains a later explicit review.

## Parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Registered metadata and owner port | Complete | Browser/conflict execution pending |
| Reviewed publish and immutable rollback | Complete | DB/runtime execution pending |
| Repeated immutable-artifact loss recovery | Source-ready | Repair/rollback execution pending |
| Generic editor control accessibility | Source-ready + SSR/browser/verifier source | Real browser/owner/screen-reader evidence pending |
| Canonical contribution metadata generation | Source-ready (Pages + shared tooling) | Execution pending |
| Forum contribution/runtime composition | Source-ready (metadata + Fly adapter + owner preview + owner properties) | Browser/runtime/deployment evidence pending |
| Provider-health observation/evaluator/binding/consumer chain | Source-ready | Exact-target execution + owner decisions pending |
| Observed-health runtime harness/owner acceptance | Source-ready + synthetic runner coverage | Maintainer execution/decision pending |
| Pages reference-consumer gate source | Source-ready/fail-closed | Candidate execution pending |
| Pages reference-consumer gate acceptance | Source-ready + synthetic owner-runner coverage | Owner + rollback decision pending |
| Forum Wave admission | Source-ready + synthetic admission-runner coverage | Accepted-gate/Forum evidence correlation pending |
| Forum live Wave retained-admission lineage | Source-ready | Blocked on real admitted observed Wave packet |
| Forum observed-Wave owner acceptance | Source-ready | Real observed Wave + owner decision pending |
| FFA/FBA promotion review | Explicit downstream boundary | Blocked on accepted observed-Wave owner packet |
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
| Version-pinned contribution registry | Source-ready | Execution pending |
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

The historical deployment slice remains unchanged; the current reconciliation additionally restores contribution version parity, moves Pages contribution authority to canonical module metadata, centralizes metadata normalization in platform build tooling, completes the Forum second-consumer adapter/preview/property source path, completes the provider-health source chain through observed-evidence owner acceptance, defines explicit Pages gate acceptance source, defines Forum Wave admission source, adds synthetic production-runner coverage for the owner/admission boundaries, binds future live Wave evidence to the actual retained admission packet, defines the separate observed-Wave owner decision and records generic editor accessibility source/browser-verifier closure.

It does not:

- change anonymous public Pages rendering;
- accept delegated or service principals;
- create another Pages or Forum persistence path;
- edit immutable published Pages documents;
- change database, GraphQL, REST, event, publish, rollback, artifact or cache schemas;
- enable same-origin launch in standalone admin builds;
- add runtime build tooling to `apps/server/Dockerfile.release`;
- add a TOML parser to Pages or Forum admin/WASM runtime;
- make platform build tooling a runtime contribution registry or tenant policy owner;
- move Forum schema, validation, persistence, visibility or authorization into Page Builder;
- claim provider-health target capture/evaluator/binding/runtime/owner-decision execution;
- claim current provider health;
- claim Forum browser/runtime/deployment harness or real Wave-admission execution;
- claim the observed Forum control-plane Wave, retained live lineage, real owner decision or promotion review execution;
- claim the Pages reference-consumer gate is accepted;
- claim browser or screen-reader accessibility conformance from source/native/synthetic evidence;
- claim verifier, Cargo, npm, Trunk, WASM, server, Docker, workflow, HTTP, browser or rollout execution unless an exact retained execution result exists;
- promote FFA or FBA.

## Next cursor

Source architecture for the Pages reference consumer, provider-health chain, Pages gate acceptance, Forum Wave admission/lineage/owner acceptance and generic editor accessibility semantics is complete at this cursor. The next work is exact evidence/decision execution rather than another adapter, health, admission or generic-control architecture slice:

1. Execute exact provider-health target identity/metrics/evaluator, binding-owner acceptance, live remaining-lease Pages binding, observed-health consumer harness and retrospective observed-evidence owner decision.
2. Execute the rollout-only reference candidate and combine it with owner-accepted observed-health evidence for the explicit Pages gate owner + rollback decision.
3. Execute Forum browser/runtime/server-function evidence on the same exact source/deployment boundary and run Forum Wave admission.
4. Perform the separate observed control-plane Wave with audit trail, fallback profiles, metrics/traces, rollback decision, approvals and waivers; verify freshness and exact retained-admission lineage; then run the retrospective observed-Wave owner decision.
5. Execute the exact-source/deployment generic editor full/read-only browser packet, verify the retained packet against independently reviewed source/RepoDigest, perform owner review, and retain separate screen-reader evidence only if the project chooses that boundary.
6. Only after accepted observed-Wave owner evidence, perform a separate explicit FFA/FBA promotion review. Acceptance itself must not mutate rollout or promote.

Current provider health is not inferred by this plan. The fail-closed Pages binding returns `unobserved` when accepted health is absent, invalid or expired.

Maintainer-owned execution evidence remains pending for the contribution/tooling, provider-health, Pages gate, Forum rollout, accessibility, release, browser, database and recovery slices.

## Maintainer validation

Focused source verification is intended to run in CI for this cursor; environment-bound execution remains separate:

```bash
node scripts/verify/verify-page-builder-admin-accessibility.mjs
node scripts/verify/verify-page-builder-accessibility-browser-evidence-harness.mjs
node scripts/verify/verify-page-builder-accessibility-browser-packet-verifier.mjs
node scripts/evidence/verify-page-builder-accessibility-browser-packet.test.mjs
node scripts/verify/verify-pages-page-builder-accessibility-plan-sync.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-runtime-observation.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-metrics.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-identity.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-evaluator.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-server-binding.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-consumer-binding.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-runtime-harness.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-observed-acceptance.mjs
node scripts/evidence/accept-pages-builder-provider-health-runtime.test.mjs
node crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-acceptance.mjs
node scripts/evidence/accept-pages-reference-consumer-gate.test.mjs
node scripts/verify/verify-forum-page-builder-wave-admission.mjs
node scripts/evidence/admit-forum-page-builder-wave.test.mjs
node scripts/verify/verify-forum-wave-admission-lineage.mjs
node scripts/verify/verify-forum-wave-plan-sync.mjs
node scripts/verify/verify-forum-wave-evidence-freshness.mjs
node scripts/verify/verify-forum-page-builder-wave-observed-acceptance.mjs
node scripts/evidence/accept-forum-page-builder-wave.test.mjs
node scripts/verify/verify-fly-ui-contributions.mjs
node scripts/verify/verify-forum-page-builder-contribution-metadata.mjs
node crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs
cargo xtask module validate pages
cargo xtask module validate forum
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

Focused source guards may be executed by the parity/provider-health workflows; exact browser/provider-health/rollout/build evidence remains pending until its dedicated execution packet is retained.
