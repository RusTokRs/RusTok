# Pages / Page Builder rollout runtime matrix harness — 2026-08-08

Status: `source-ready / maintainer-execution-pending / owner-review-pending / gate-unaccepted`.

Base: `main@edd68d44d726085252a0c0b9ae426a3ec016032f`.

## Purpose

This source packet makes the four Pages reference-consumer rollout profiles reproducible through production ownership after the rollout authority and standalone browser-intent guard were corrected in PR #3337.

It does not execute the matrix and does not accept `pages_reference_consumer_gate`.

## Execution boundary

The maintainer-run Playwright harness is defined by:

- `crates/rustok-pages/contracts/evidence/pages-builder-rollout-runtime-matrix-execution-contract.json`
- `apps/next-admin/playwright.pages-builder-rollout-matrix.config.ts`
- `apps/next-admin/tests/pages-builder-rollout-matrix/runtime-matrix.spec.ts`
- `crates/rustok-pages/contracts/evidence/pages-builder-rollout-runtime-matrix-harness-source.json`
- `crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-runtime-matrix-harness.mjs`

It requires an already passing Pages inline-edit browser evidence packet from the exact same source commit and origin. The immutable deployment RepoDigest is inherited from that predecessor so the rollout matrix cannot silently execute against a different deployment.

## Production settings authority

The harness does not use direct SQL or database credentials.

It reads the current Pages tenant-module snapshot through production GraphQL `tenantModules` and writes each profile through production `updateModuleSettings`. The existing module lifecycle owner therefore remains responsible for validation and persistence.

Before the first profile the harness snapshots the complete Pages settings object. Profile application changes only:

```text
builder.enabled
builder.preview.enabled
builder.properties.enabled
builder.publish.enabled
```

Unknown and non-builder settings are preserved.

The original settings are restored through `updateModuleSettings` inside `finally`, and the harness re-reads `tenantModules` to require semantic equality and the original canonical hash before any evidence packet can be produced.

## Four profiles

The four profiles are source-declared in one fixed order and must all complete before an output packet is retained.

The matrix covers exactly:

- `all_on`: builder/preview/properties/publish enabled; provider state remains `unobserved` because no health telemetry is claimed;
- `publish_off`: preview/properties remain available, publish is denied, provider control state is `degraded`;
- `preview_off`: preview and publish are disabled, properties remain available, provider control state is `degraded`;
- `builder_off`: provider is `unavailable`, editor capabilities narrow to read-only, preview/publish/properties authoring paths are denied.

## Per-profile evidence

Each profile checks the same production boundaries:

1. `pageBuilderRolloutSnapshot` returns the exact persisted flags for the requested tenant and `providerHealthObserved=false`.
2. Pages-owned list and selected-document GraphQL reads still succeed.
3. The real Pages admin workspace exposes the expected provider control state and provider health stays `unobserved`.
4. Preview behavior is checked against authoritative `/api/fn/pages/page-builder-capability` dispatch. `all_on` captures a real successful request in memory; `preview_off` and `builder_off` replay that exact request shape after their persisted settings are active and require the typed `capability disabled: preview` result.
5. Disabled publish is checked through `/api/admin/pages/{page_id}/builder/intents` and must return `403 / FLY_CAPABILITY_DENIED / publish` before draft mutation.
6. `builder_off` also requires a typed `properties` denial for a handcrafted `rename_page` browser intent.

The `all_on` publish probe is deliberately non-mutating: it requires the publish capability to be enabled in the UI but does not send a Save intent or publish a document.

## Privacy and retained data

The output retains only exact source identity, immutable deployment digest, hashes/sizes of external inputs, hashed fixture identities, response statuses/body sizes/body hashes, booleans, and the canonical hash of the original settings.

It does not retain tenant slugs or ids, page ids, admin routes, cookies, authorization headers, storage-state contents, tokens, session ids, raw module settings, raw GraphQL bodies, raw preview request/response bodies, raw browser-intent bodies, raw HTML, traces, screenshots, or videos.

The Playwright config fixes one worker, zero retries, and disables trace/screenshots/video.

## Output

Default output:

```text
target/pages-builder-rollout-runtime-matrix.json
```

Identity:

```text
format: pages_builder_rollout_runtime_matrix_v1
status: four_profile_runtime_matrix_passed_owner_review_pending
```

A successful matrix packet remains an execution candidate for owner review. It does not automatically accept the Pages reference gate, accept Forum Wave, claim observed provider SLO health, mutate canonical source, or promote FFA/FBA.

## Maintainer execution

After the exact-source browser predecessor and reviewed fixture inputs exist:

```bash
cd apps/next-admin
npx --no-install playwright test --config playwright.pages-builder-rollout-matrix.config.ts
```

The source guard is:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-runtime-matrix-harness.mjs
```

No tests, Node verifiers, Cargo commands, formatting, database scenarios, GraphQL/HTTP requests, Playwright/browser runs, workflows, CI, or `git diff --check` were executed by this implementation slice.
