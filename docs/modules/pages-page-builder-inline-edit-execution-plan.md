# Pages / Page Builder Inline Edit Execution Plan

Date: 2026-08-06  
Status: `artifact-http-evidence-harness-source-ready / session-dom-boundary-source-fixed / browser-evidence-harness-source-ready / rollout-evidence-harness-source-ready / artifact-http-browser-rollout-execution-pending`
Parent cursor: `docs/modules/pages-page-builder-parity-continuation-plan.md`

## Purpose

This is the active execution subplan for the source-ready Pages inline-authoring pipeline. It does not replace the parent parity plan or historical source packets. It narrows the remaining work to reproducible execution evidence and tenant rollout.

Source readiness is not execution evidence. No checkbox in this file may be promoted from source inspection alone.

## Current marker

```text
inline-edit-rollout-evidence-harness-source-ready
```

Rollout evidence harness: source-ready.

The two-phase FFA/FBA machine contract, bounded external observation assembler, source evidence, fail-closed guard and maintainer packet now exist. The assembler observes externally performed rollout actions and cannot mutate deployment, configuration, promotion or rollback state.

Retained browser marker:

```text
inline-edit-browser-evidence-harness-source-ready
```

The browser harness remains the required rollout predecessor and must pass for the exact same source commit and immutable deployment RepoDigest.

Retained artifact/HTTP marker:

```text
inline-edit-artifact-http-evidence-harness-source-ready
```

The build snapshot, production image, HTTP capture and same-commit aggregate tooling remain the required browser predecessor. No artifact, Docker, HTTP, browser, workflow or rollout result is claimed.

Corrective security marker:

```text
inline-edit-session-dom-boundary-source-fixed
```

The Page Builder authoring root no longer derives its DOM id from the grant session and no longer emits `data-inline-session`. The deterministic hydration id now uses Fly page identity plus expected project hash. Maintainer validation and browser inspection remain pending.

## Gate A — source guards

- [ ] Run the authenticated adapter, consumer, route, asset, admin launch and release-composition source guards.
- [ ] Run `verify-page-builder-authenticated-inline-edit-adapter.mjs` and confirm it rejects session-derived DOM identity.
- [ ] Run `verify-page-builder-inline-session-dom-boundary.mjs`.
- [ ] Confirm the source contains neither `data-inline-session` nor `dom_id(grant.session_id())`.
- [ ] Run `verify-pages-inline-edit-artifact-http-evidence-harness.mjs`.
- [ ] Run `verify-pages-inline-edit-browser-evidence-harness.mjs`.
- [ ] Run `verify-pages-inline-edit-rollout-evidence-harness.mjs`.
- [ ] Run release infrastructure, supply-chain and readiness guards.
- [ ] Record the exact source commit and command output hashes.

Gate A does not promote runtime state.

## Gate B — two isolated builds

- [ ] Build from the exact same source commit in two isolated clean roots/worktrees.
- [ ] Capture `build-a` and `build-b` with `capture-pages-inline-edit-build-snapshot.mjs`.
- [ ] Confirm exact Node, Cargo, rustc, Trunk and wasm-bindgen versions.
- [ ] Confirm identical source hashes.
- [ ] Confirm identical embedded admin index/CSS hashes and full admin dist manifests.
- [ ] Confirm identical authoring bootstrap/JS/WASM hashes and sizes.
- [ ] Confirm identical native server binary hash and size.
- [ ] Retain only command-log hashes and sizes in the evidence packet.

Gate B passing state:

```text
inline-edit-two-build-reproducibility-observed
```

## Gate C — production image

- [ ] Build or obtain the production server image for the same source commit.
- [ ] Capture it with `capture-pages-inline-edit-docker-evidence.mjs`.
- [ ] Require an immutable RepoDigest.
- [ ] Require `linux/amd64`, UID/GID `10001:10001` and `/app/rustok-server` entrypoint.
- [ ] Require the OCI revision label to equal the source commit.
- [ ] Record the bounded projection and hash of the raw inspect output; do not retain the full inspect document.
- [ ] Retain only a hash/size identity for the requested image argument, never its raw value.

Gate C passing state:

```text
inline-edit-production-image-identity-observed
```

## Gate D — asset and authoring HTTP

- [ ] Run `capture-pages-inline-edit-http-evidence.mjs` against an explicit deployed origin.
- [ ] Supply the immutable deployment RepoDigest recorded in Gate C.
- [ ] Require the aggregate to find that exact RepoDigest in the Docker packet.
- [ ] Record that this binds the maintainer deployment identity but does not independently attest the external orchestrator.
- [ ] Prove `200`, MIME, cache policy, CORP and body-bound strong ETag for all three fixed assets.
- [ ] Prove empty `304` for exact and weak `If-None-Match` for every asset.
- [ ] Prove ETag, cache policy and CORP remain exact on both conditional responses.
- [ ] Bind HTTP asset body hashes and sizes to the built bootstrap/JS/WASM artifacts.
- [ ] Prove anonymous `401`.
- [ ] Prove direct-user `200`.
- [ ] Prove service `403`.
- [ ] Prove delegated `403`.
- [ ] Prove missing-session `401`.
- [ ] Prove permission-denied `403`.
- [ ] Prove `private, no-store` and `noindex, nofollow, noarchive` on every authoring response.
- [ ] Prove direct-user HTML binds exact `data-pages-page-id` and `data-pages-locale` attributes without token, proof, session or signing-secret markers.
- [ ] Persist credential environment names only, never values.

Gate D passing state:

```text
inline-edit-asset-authoring-http-observed
```

## Gate E — anonymous artifact exclusion

- [ ] Build an explicit anonymous storefront artifact for the same source commit.
- [ ] Run `inspect-pages-anonymous-storefront-ssr-artifact.mjs`.
- [ ] Require a passing feature-resolved dependency graph.
- [ ] Require at least one explicit inspected artifact.
- [ ] Require no authoring/admin/Fly markers.
- [ ] Do not treat absence of a client artifact as a passing artifact inspection.

Gate E passing state:

```text
inline-edit-anonymous-artifact-exclusion-observed
```

## Gate F — aggregate artifact and HTTP evidence

- [ ] Assemble build A, build B, Docker, HTTP and anonymous artifact packets with `assemble-pages-inline-edit-artifact-http-evidence.mjs`.
- [ ] Require every packet to bind the current source commit.
- [ ] Require the HTTP deployment RepoDigest to exist in the Docker RepoDigest set.
- [ ] Require all input SHA-256 digests and sizes.
- [ ] Review `target/pages-inline-edit-artifact-http-evidence.json`.
- [ ] Confirm the status is exactly:

```text
artifact_http_execution_passed_browser_rollout_pending
```

This status closes only artifact, production-image, HTTP and anonymous-artifact execution. Browser and rollout remain open.

## Gate G — authenticated browser behavior

Source owner:

```text
apps/next-admin/playwright.pages-inline-edit.config.ts
apps/next-admin/tests/pages-inline-edit/browser-evidence.spec.ts
```

Source guard:

```text
node crates/rustok-pages/scripts/verify/verify-pages-inline-edit-browser-evidence-harness.mjs
```

Execution requires reviewed external storage-state files and fixture routes. Trace, screenshots and video remain disabled. The retained packet stores only hashes, sizes, statuses, counters and bounded facts.

- [ ] Supply the passing Gate F packet for the exact same source commit, deployed origin and immutable RepoDigest.
- [ ] Supply direct-editor, unauthorized and standalone-admin storage states without copying their contents into evidence.
- [ ] Confirm launch is visible for an allowed editor on an unpublished page.
- [ ] Confirm launch is hidden for published, missing, locale-less, unauthorized and standalone-admin states.
- [ ] Confirm navigation is relative, same-origin and uses the selected document's exact locale.
- [ ] Confirm the bounded authoring root mounts the dedicated bootstrap/JS/WASM client without critical request, console or page failures.
- [ ] Inspect both SSR HTML and hydrated DOM: no grant session, proof, bearer token, signing material or authenticated session identifier may appear in ids, attributes or URLs.
- [ ] Confirm the authoring root id uses Fly page identity plus expected project hash rather than grant/session identity.
- [ ] Confirm only the reviewed static leaf receives `data-fly-inline-editable="content"` and `contenteditable="plaintext-only"`.
- [ ] Confirm provider-owned, composite, templated, interactive and runtime-owned fixtures remain read-only.
- [ ] Edit one eligible real-DOM text node and confirm one changed `focusout` emits exactly one commit request.
- [ ] Confirm a successful save replaces revision and project hash.
- [ ] Reload and confirm the saved text, revision and project hash.
- [ ] Use a second preloaded tab to confirm stale revision fails without a partial document write.
- [ ] Replay the exact successful request in memory and confirm rejection.
- [ ] Delay a fresh tab beyond the reviewed short grant TTL and confirm expiry rejection without a partial document write.
- [ ] Review `target/pages-inline-edit-browser-evidence.json` and confirm no raw storage state, credentials, session IDs, grants, proofs, HTML, request/response bodies, console text, page IDs, component IDs, edited text, traces, screenshots or video were retained.
- [ ] Confirm the status is exactly:

```text
browser_execution_passed_rollout_pending
```

Gate G passing state:

```text
inline-edit-browser-edit-save-replay-expiry-observed
```

This status closes only browser evidence for the reviewed source commit, origin and image digest. Rollout remains open.

## Gate H — rollout

Source owner:

```text
scripts/evidence/assemble-pages-inline-edit-rollout-evidence.mjs
```

Source guard:

```text
node crates/rustok-pages/scripts/verify/verify-pages-inline-edit-rollout-evidence-harness.mjs
```

The assembler consumes bounded maintainer observations. It does not query deployment/configuration/monitoring systems and does not mutate configuration, deploy, promote or roll back.

- [ ] Supply the passing Gate G browser packet for the exact same source commit and immutable deployment RepoDigest.
- [ ] Record bounded environment and configuration-profile identities; retain only their SHA-256 values.
- [ ] Confirm `pages.builder.inline_edit.enabled` only for the intended enabled tenant cohort and retain tenant identities as SHA-256 only.
- [ ] Retain at least one disabled control tenant and confirm enabled/control cohorts are disjoint.
- [ ] Confirm Pages module enablement, direct user, authenticated session and `pages:update` remain mandatory.
- [ ] Record exact positive FFA observation-window timestamps and duration.
- [ ] Record reviewed observed counts and thresholds for save conflicts, authorization denials, grant verification failures and client load failures.
- [ ] Require every observed count to remain at or below its reviewed threshold.
- [ ] Record a SHA-256 rollback owner identity and immutable rollback image distinct from the active image.
- [ ] Review browser, configuration and monitoring evidence and obtain rollout-owner approval.
- [ ] Assemble FFA evidence with `assemble-pages-inline-edit-rollout-evidence.mjs --phase ffa`.
- [ ] Confirm the FFA status is exactly:

```text
ffa_observation_passed_fba_pending
```

- [ ] Start the FBA observation window only after the FFA window has ended.
- [ ] Perform and review a successful rollback rehearsal through the external operational owner.
- [ ] Review the previous FFA packet.
- [ ] Assemble FBA evidence with `assemble-pages-inline-edit-rollout-evidence.mjs --phase fba --ffa <packet>`.
- [ ] Confirm the terminal status is exactly:

```text
fba_rollout_evidence_complete
```

Gate H terminal state:

```text
inline-edit-rollout-ffa-fba-evidence-complete
```

Artifact, HTTP, browser and rollout execution remain pending. FFA/FBA are not promoted by source inspection or by running the assembler alone.

## Privacy boundary

Retained evidence must not contain:

- Authorization or Cookie values;
- storage-state contents;
- bearer/session tokens or session IDs;
- grants, proofs or signing keys;
- raw authoring HTML or denial bodies;
- raw request or response bodies;
- raw console message text;
- admin paths, page IDs, component IDs or edited text;
- raw tenant IDs or tenant names;
- raw environment, configuration-profile or rollout-owner values;
- deployment credentials, configuration secrets or database URLs;
- raw monitoring logs or alert payloads;
- traces, screenshots or video;
- raw build logs;
- raw Docker image request references;
- full Docker inspect documents;
- tenant secrets or database credentials.

Hashes, sizes, selected headers, environment variable names, immutable RepoDigests, source identities, observation timestamps, counters, thresholds and bounded pass/fail facts are allowed.

## Current state

```text
source pipeline: ready
artifact/HTTP evidence harness: source-ready
session DOM exposure: source-fixed, validation pending
browser evidence harness: source-ready
rollout evidence harness: source-ready
artifact execution: pending
HTTP execution: pending
browser execution: pending
rollout execution: pending
FFA/FBA: not promoted
```
