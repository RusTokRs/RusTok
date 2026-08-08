# Pages / Page Builder reference-consumer gate evidence harness actualization — 2026-08-08

Status: `candidate-harness-source-ready / rollout-matrix-input-required / exact-source-input-chain-required / owner-review-pending / gate-acceptance-pending`

Base rechecked: `main@a08337a73bba49ee57dc932d7e0128ac545e2071`.

## Purpose

The four-profile Pages / Page Builder rollout matrix is now source-ready, but the general `pages_reference_consumer_gate` candidate previously accepted only artifact/HTTP and browser evidence. That left a source-level integrity gap: a candidate could be produced without supplying the matrix packet even though the canonical gate already lists the four-profile rollout matrix as required execution evidence.

This slice closes that correlation gap. It does not execute the matrix or the candidate and does not accept the gate.

## Machine execution contract

`crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-execution-contract.json` now requires three existing machine packets:

1. `pages_inline_edit_artifact_http_execution_v1` with status `artifact_http_execution_passed_browser_rollout_pending`;
2. `pages_inline_edit_browser_execution_v1` with status `browser_execution_passed_rollout_pending`;
3. `pages_builder_rollout_runtime_matrix_v1` with status `four_profile_runtime_matrix_passed_owner_review_pending`.

All three packets must match the exact checked-out Git HEAD. The browser packet must remain bound to the exact artifact/HTTP file hash and immutable API/server RepoDigest. The rollout matrix must remain bound to the exact browser packet hash, the same API RepoDigest, the browser predecessor API-origin hash and the browser predecessor standalone-admin-origin hash.

The candidate contract also requires the rollout-matrix source verifier as a canonical source guard, so source drift in the matrix contract/spec/ownership boundary prevents candidate production.

## Rollout matrix recheck inside the candidate runner

`scripts/evidence/pages-reference-consumer-gate-evidence.mjs` does not rerun Playwright. Instead, it treats the successful matrix packet as a required execution input and independently checks the retained bounded facts before any candidate can be emitted.

For every canonical profile — `all_on`, `publish_off`, `preview_off`, `builder_off` — the candidate runner rechecks:

- the exact persisted rollout flags;
- successful production settings-write evidence;
- server-owned rollout snapshot tenant/flag agreement;
- provider health remaining `unobserved`;
- Pages-owned list and document reads;
- expected admin provider/Preview/Properties/Publish UI state;
- authoritative Preview SSR pass or typed disabled outcome;
- typed `FLY_CAPABILITY_DENIED / publish / save` browser-intent denial for disabled publish;
- typed Properties denial for `builder_off`;
- non-mutating publish-dry behavior for `all_on`.

It also requires the original Pages module settings to be restored and verified by canonical semantic hash. A matrix packet with missing profiles, an unverified restore, mismatched provenance, unexpected promotion claims, or retained sensitive/raw evidence fails closed.

## Bounded runner

The candidate runner continues to:

- use `spawnSync` with `shell: false`;
- execute only contract-declared `node` source guards and bounded `cargo test` commands;
- validate exact Git HEAD before execution;
- reject missing, symlinked or oversized required source files;
- hash every required source file;
- remove stale candidate output before execution;
- retain only command id/argv, exit status and stdout/stderr byte length + SHA-256, never raw command output;
- write atomically inside repository `target/`;
- avoid persisting raw artifact/browser/matrix packets, rollout settings, credentials, sessions, grants, proofs or HTTP bodies.

The focused Cargo set remains bounded to metadata revision/dirty-Fly isolation, Page Builder sanitization/resource limits, Pages publish/rollback cache correlation and provider degraded-profile behavior.

## Candidate boundary

Successful component execution produces:

```text
format = pages_reference_consumer_gate_candidate_v1
status = component_execution_passed_owner_review_pending
```

The candidate records that the artifact/browser chain and rollout-matrix/browser chain are bound, all four matrix profiles passed, and original rollout settings were restored. It still records:

```text
provider_health = unobserved
owner_signoff = pending
rollback_decision = pending
gate_acceptance = pending
```

The candidate owner review remains pending.

The candidate does not accept `pages_reference_consumer_gate`.

It does not claim provider health.

It does not promote FFA/FBA.

It does not accept Forum Wave evidence or mutate canonical source. A later maintainer review must independently decide owner sign-off and rollback disposition before the source gate can change from `accepted = false`.

## Source evidence and guards

- `crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-evidence-harness-source.json` records the unexecuted matrix-backed candidate state.
- `crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-evidence-harness.mjs` locks the three-input chain, source guard allow-list, matrix outcome rechecks, privacy boundary and pending-approval semantics.
- `crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-runtime-matrix-harness.mjs` is now a required source guard of the Pages reference gate and candidate contract.
- `crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json` keeps `execution_gate = pending`, `accepted = false`, provider health `unobserved`, Forum Wave blocked and FFA/FBA unpromoted.

## Maintainer execution

After producing the exact-source artifact/HTTP, browser and rollout-matrix packets, the candidate runner requires their paths through:

```text
RUSTOK_PAGES_REFERENCE_GATE_ARTIFACT_HTTP_EVIDENCE
RUSTOK_PAGES_REFERENCE_GATE_BROWSER_EVIDENCE
RUSTOK_PAGES_REFERENCE_GATE_ROLLOUT_MATRIX_EVIDENCE
```

Optional candidate output override:

```text
RUSTOK_PAGES_REFERENCE_GATE_OUTPUT
```

Suggested source guards, intentionally not run by this implementation slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-runtime-matrix-harness.mjs
node crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-evidence-harness.mjs
```

No tests, verifiers, Cargo commands, Node commands, browser runs, HTTP requests, workflows or CI were run by this implementation slice.
