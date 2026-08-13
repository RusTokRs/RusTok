# Pages published metadata browser evidence harness actualization — 2026-08-13

Status: `source-ready / maintainer-browser-execution-pending / consumer-properties-admission-pending`.

## Cursor

The canonical Page Builder FBA terminal inventory has 11 pending evidence nodes after static sanitization execution was admitted. The first remaining provider blocker is `/provider/consumer_properties_contract/executed_evidence`.

The consumer-properties source contract is source-connected. Exact-main Rust/source execution succeeded in run `31696862980` for `9b5e6e57e0ddf8e968f1118e3372091b2929fd7b`, with retained status `rust_source_execution_passed_browser_evidence_pending`. This workflow slice changes the required browser-evidence source set, so after merge the exact-main Rust/source workflow must mint a successor receipt on the new source commit before final admission.

The selected published metadata surface still requires a retained browser packet against a reviewed deployment. This slice operationalizes that maintainer execution path; it does not execute Chromium itself and does not change `page-builder-consumer-properties.json` from `executed_evidence: pending`.

## Browser execution contract

The retained contract is:

`crates/rustok-pages/contracts/evidence/pages-published-metadata-browser-execution-contract.json`

A successful maintainer run may write only:

- format: `pages_published_metadata_browser_execution_v1`;
- status: `browser_execution_passed_consumer_properties_admission_pending`.

The runner requires the supplied source commit to equal checkout `HEAD`. It also hashes every required source file at execution time. The immutable deployment RepoDigest is a maintainer-supplied reviewed identity; the browser does not independently attest that the opened route is served by that digest, so deployment provenance remains an external admission requirement.

## Dispatch-only workflow

The maintainer execution entry point is the dispatch-only workflow:

`.github/workflows/pages-published-metadata-browser-evidence.yml`

It is intentionally not triggered by `push`, `pull_request`, or `pull_request_target`. A run must be dispatched from `main`, the reviewed `source_commit` input must equal the dispatch `GITHUB_SHA`, and `reviewed_deployment_identity=true` is required before the evidence steps proceed.

The job is bound to the protected environment `pages-published-metadata-browser-evidence`. That environment must be configured by maintainers with main-only deployment branch protection and required reviewers before it is used for evidence. The editor fixture is supplied only through the protected environment secret `RUSTOK_PAGES_PUBLISHED_METADATA_EDITOR_STORAGE_STATE_B64`; the workflow decodes it into a mode-restricted temporary file, never commits it, never uploads it, and removes it after the run.

The workflow masks the reviewed RepoDigest and four reviewed route inputs before browser execution. The only uploaded artifact is the bounded JSON packet under `target/pages-published-metadata-browser-evidence.json`, retained for 90 days. Trace, screenshots, video, Playwright reports, raw test-result directories and the editor storage-state are not uploaded by this workflow.

The workflow source guard is:

`node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-browser-execution-workflow.mjs`

This workflow makes the maintainer run reproducible but does not establish deployment provenance. The required protected-environment policy and reviewed external fixtures remain maintainer-owned execution inputs.

## Reviewed profiles

Maintainers provide one external editor storage-state file and four reviewed Pages admin profile URLs:

1. `published` — a selected published page whose standalone registered metadata surface is expected to be admitted;
2. `draft` — a selected draft page where the published-only registered surface must be absent;
3. `archived` — a selected archived page where the surface must be absent;
4. `missing` — a route with no selected page where the surface must be absent.

The harness does not seed pages, tenants, sessions or metadata. The URLs and storage state are external reviewed fixtures.

## Published proof

The published profile must observe the existing production DOM contract:

- `data-pages-published-metadata-surface=registered`;
- `data-pages-published-metadata-admission=published-only`;
- `data-pages-fly-canvas-mounted=false`;
- `data-pages-document-authoring=false`;
- `data-pages-metadata-runtime=registered`;
- `data-pages-metadata-persistence=owner-port`.

Inside that surface the existing `ConsumerPropertiesPanel` must reach `data-fly-consumer-properties=ready`, bind `rustok.pages.metadata.editor`, expose the registered `title` and `slug` controls, and expose the ordinary `Save properties` action.

The browser harness deliberately **does not click `Save properties`**. Metadata revision conflict, metadata-only patch shape and dirty Fly isolation remain covered by the separate source packet and focused Rust regressions; this browser packet proves production route composition/admission without mutating a reviewed published fixture.

## Hidden-profile proof

Draft, archived and missing profiles must expose neither the registered published metadata surface nor its error surface. The production component itself remains responsible for page loading and published-status admission; the harness does not duplicate that decision in a test-only route.

## Retention boundary

The output retains only:

- exact source commit;
- maintainer-supplied reviewed immutable deployment RepoDigest;
- source-file SHA-256 hashes;
- storage-state SHA-256 hash and byte size;
- SHA-256 hashes of the four reviewed profile URLs;
- bounded boolean/count observations;
- Node and Playwright versions.

It does not retain raw profile URLs, cookies, Authorization headers, storage-state contents, raw DOM/HTML, metadata field values, tenant IDs or actor IDs. Global setup removes only the bounded prior output under repository `target/`, so a failed run cannot leave a stale success packet.

## Harness

The runner uses the repository's existing pinned Playwright package:

- `apps/next-admin/playwright.pages-published-metadata.config.ts`;
- `apps/next-admin/tests/pages-published-metadata/global-setup.ts`;
- `apps/next-admin/tests/pages-published-metadata/browser-evidence.spec.ts`.

Chromium is single-worker, retry-free, and trace/screenshots/video are disabled.

The source-only harness guard is:

`node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-browser-evidence-harness.mjs`

A direct maintainer browser run remains:

`cd apps/next-admin && npx --no-install playwright test --config playwright.pages-published-metadata.config.ts`

The dispatch workflow wraps the same command with exact-source, reviewed-deployment, protected-fixture, bounded-packet and artifact gates.

## Governance boundary

A passing browser packet is only one input for eventual consumer-properties evidence admission. It does not by itself prove deployment provenance, does not mutate metadata, does not set `consumer_properties_contract.executed_evidence=verified`, and does not promote Pages FFA or Page Builder FBA.

This source slice does not change `executed_evidence`. No browser execution is claimed by this source slice. No live tenant mutation, database operation, owner approval or platform approval is claimed.
