# Pages / Page Builder Inline Edit Execution Plan

Date: 2026-08-06  
Status: `artifact-http-evidence-harness-source-ready / artifact-http-browser-rollout-execution-pending`
Parent cursor: `docs/modules/pages-page-builder-parity-continuation-plan.md`

## Purpose

This is the active execution subplan for the source-ready Pages inline-authoring pipeline. It does not replace the parent parity plan or historical source packets. It narrows the remaining work to reproducible execution evidence and tenant rollout.

Source readiness is not execution evidence. No checkbox in this file may be promoted from source inspection alone.

## Current marker

```text
inline-edit-artifact-http-evidence-harness-source-ready
```

Artifact/HTTP evidence harness: source-ready.

The machine contract, build snapshot capture, production image capture, HTTP capture and same-commit aggregate assembler now exist. No artifact, Docker, HTTP, browser, workflow or rollout result is claimed.

## Gate A — source guards

- [ ] Run the authenticated adapter, consumer, route, asset, admin launch and release-composition source guards.
- [ ] Run `verify-pages-inline-edit-artifact-http-evidence-harness.mjs`.
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

- [ ] Confirm launch is visible for an allowed editor on an unpublished page.
- [ ] Confirm launch is hidden for published, missing, locale-less, unauthorized and standalone-admin states.
- [ ] Confirm navigation is same-origin and uses the selected document's exact locale.
- [ ] Confirm the bounded authoring root mounts the dedicated JS/WASM client.
- [ ] Edit one eligible real-DOM text node.
- [ ] Confirm one canonical Fly patch and one Pages document save.
- [ ] Reload and confirm the saved document revision.
- [ ] Confirm a fresh replacement grant after success.
- [ ] Confirm stale revision fails without a partial document write.
- [ ] Confirm replayed grant/sequence fails.
- [ ] Confirm expired grant fails.
- [ ] Confirm provider-owned, composite, templated, interactive and runtime-owned subtrees remain read-only.
- [ ] Confirm no grant, proof, signing material or session identifier appears in DOM, URL or browser logs.

Gate G passing state:

```text
inline-edit-browser-edit-save-replay-expiry-observed
```

## Gate H — rollout

- [ ] Record tenant, environment, image digest and configuration profile.
- [ ] Confirm `pages.builder.inline_edit.enabled` only for the intended tenant cohort.
- [ ] Confirm Pages module enablement, direct-user session and `pages:update` remain mandatory.
- [ ] Monitor save conflicts, authorization denials, grant verification failures and client load failures.
- [ ] Define rollback owner and image digest.
- [ ] Promote FFA only after artifact/HTTP/browser evidence is retained and reviewed.
- [ ] Promote FBA only after the FFA observation window and rollback rehearsal.

Browser edit/save/replay/expiry and tenant rollout remain pending.

## Privacy boundary

Retained evidence must not contain:

- Authorization or Cookie values;
- bearer/session tokens or session IDs;
- grants, proofs or signing keys;
- raw authoring HTML or denial bodies;
- raw build logs;
- raw Docker image request references;
- full Docker inspect documents;
- tenant secrets or database credentials.

Hashes, sizes, selected headers, environment variable names, immutable RepoDigests, source identities and bounded pass/fail facts are allowed.

## Current state

```text
source pipeline: ready
artifact/HTTP evidence harness: source-ready
artifact execution: pending
HTTP execution: pending
browser execution: pending
rollout: pending
FFA/FBA: not promoted
```
