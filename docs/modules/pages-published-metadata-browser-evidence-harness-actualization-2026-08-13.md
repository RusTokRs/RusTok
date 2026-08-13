# Pages published metadata browser evidence harness actualization — 2026-08-13

Status: `source-ready / maintainer-browser-execution-pending / consumer-properties-admission-pending`.

## Cursor

The canonical Page Builder FBA terminal inventory now has 11 pending evidence nodes after static sanitization execution was admitted. The first remaining provider blocker is `/provider/consumer_properties_contract/executed_evidence`.

The consumer-properties source contract is already source-connected and retains two lower-level source packets: metadata revision/isolation and the selected published metadata surface. Their focused Rust regressions remain execution work, and the parity continuation plan also keeps the published browser packet open. No retained browser harness previously existed for that published metadata surface.

This slice closes only that source-architecture gap. It does not execute Chromium and does not change `page-builder-consumer-properties.json` from `executed_evidence: pending`.

## Browser execution contract

The retained contract is:

`crates/rustok-pages/contracts/evidence/pages-published-metadata-browser-execution-contract.json`

A successful maintainer run may write only:

- format: `pages_published_metadata_browser_execution_v1`;
- status: `browser_execution_passed_consumer_properties_admission_pending`.

The runner requires the supplied source commit to equal checkout `HEAD`. It also hashes every required source file at execution time. The immutable deployment RepoDigest is a maintainer-supplied reviewed identity; the browser does not independently attest that the opened route is served by that digest, so deployment provenance remains an external admission requirement.

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

The source-only guard is:

`node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-browser-evidence-harness.mjs`

A maintainer browser run is:

`cd apps/next-admin && npx --no-install playwright test --config playwright.pages-published-metadata.config.ts`

## Governance boundary

A passing browser packet is only one input for eventual consumer-properties evidence admission. It does not by itself prove the focused Rust revision/isolation regressions, does not establish deployment provenance, does not mutate metadata, does not set `consumer_properties_contract.executed_evidence=verified`, and does not promote Pages FFA or Page Builder FBA.

No browser execution is claimed by this source slice. No live tenant mutation, database operation, GraphQL/HTTP evidence run, owner approval or platform approval is claimed.
