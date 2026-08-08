# Pages / Page Builder reference-consumer gate evidence harness actualization — 2026-08-08

Status: `candidate-harness-source-ready / matrix-and-feature-preflight-required / exact-source-input-chain-required / owner-review-pending / gate-acceptance-pending`.

Base rechecked: `main@a08337a73bba49ee57dc932d7e0128ac545e2071`.

## Purpose

The Pages reference candidate must not infer the canonical Page Builder provider error catalog from a different browser-security contract.

The runtime matrix proves real Pages UI/SSR behavior, Pages-owned reads and direct standalone-browser bypass resistance. Its direct browser intent denials intentionally use `FLY_CAPABILITY_DENIED`. The canonical Page Builder degraded-mode contract, however, requires `feature-disabled / FEATURE_DISABLED` for disabled provider capabilities.

This slice makes those two evidence layers explicit and requires both. It does not execute any harness and does not accept `pages_reference_consumer_gate`.

## Four required machine packets

`crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-execution-contract.json` now requires four existing machine packets:

1. `pages_inline_edit_artifact_http_execution_v1` / `artifact_http_execution_passed_browser_rollout_pending`;
2. `pages_inline_edit_browser_execution_v1` / `browser_execution_passed_rollout_pending`;
3. `pages_builder_rollout_runtime_matrix_v1` / `four_profile_runtime_matrix_passed_owner_review_pending`;
4. `pages_builder_rollout_feature_preflight_v1` / `four_profile_feature_preflight_passed_candidate_pending`.

All four must match the exact checked-out Git HEAD.

The browser packet must bind the exact artifact/HTTP file hash and immutable API/server RepoDigest. The rollout matrix must bind the exact browser packet hash, the same API RepoDigest, browser API-origin hash and standalone-admin-origin hash. The canonical feature-preflight packet must in turn bind the exact browser packet hash and exact rollout-matrix packet hash, plus the same API origin and immutable RepoDigest.

Both the matrix and feature-preflight packets must prove that their temporary Pages settings changes were restored before candidate production.

## Why two rollout execution layers are required

The two rollout packets have intentionally different responsibilities.

The runtime matrix proves:

- persisted profile flags through production `updateModuleSettings`;
- real Pages admin provider state;
- real Preview UI and authoritative Preview SSR behavior;
- Pages-owned list/document reads under degraded profiles;
- direct standalone `/builder/intents` bypass resistance;
- `FLY_CAPABILITY_DENIED` for disabled browser Publish/Properties intents;
- no fabricated provider health;
- settings restoration.

The feature-preflight packet proves the canonical provider catalog without mutation:

- server-resolved tenant and auth authority;
- the same `PageBuilderCapabilityPermissions` mapping used by the Page Builder authorizer;
- the shared `rustok_page_builder::rollout::ensure_capability` guard;
- `all_on` Preview/Properties/Publish dry preflight allowed;
- disabled Preview/Properties/Publish preflight returning `feature-disabled / FEATURE_DISABLED` exactly where the canonical profiles require it;
- no Preview rendering or Publish persistence;
- settings restoration.

`FLY_CAPABILITY_DENIED` is therefore never treated as `FEATURE_DISABLED` evidence. Both are retained as distinct guarantees.

## Candidate runner recheck

`scripts/evidence/pages-reference-consumer-gate-evidence.mjs` does not rerun Playwright. It consumes the already-produced packets and independently rechecks their bounded retained facts.

For the runtime matrix it rechecks all four profile flags, settings writes, server-owned rollout snapshot agreement, Pages reads, admin UI state, Preview SSR behavior, browser-intent denials, privacy, provenance and restore state.

For the canonical feature preflight it rechecks all four profile flags and the exact Preview/Properties/Publish expectations. Disabled results must be:

```text
allowed = false
error_kind = feature-disabled
error_code = FEATURE_DISABLED
```

Allowed results must contain no error kind/code. The feature-preflight packet must bind the exact supplied browser and matrix packet hashes, exact source commit, API origin and RepoDigest.

Only after these packet checks does the runner execute the contract-declared source guards and bounded focused Cargo tests.

## Bounded runner

The candidate runner continues to:

- use `spawnSync` with `shell: false`;
- execute only contract-declared `node` source guards and bounded `cargo test` commands;
- validate exact Git HEAD before execution;
- reject missing, symlinked or oversized required files;
- hash every required source file;
- remove stale candidate output before execution;
- retain command id/argv, exit status and stdout/stderr byte length + SHA-256, never raw output;
- retain input packet byte lengths and SHA-256 only, never packet contents;
- write atomically inside repository `target/`;
- avoid retaining raw rollout settings, credentials, sessions, grants, proofs, HTML or HTTP/GraphQL bodies.

Both rollout source verifiers are canonical gate guards:

```text
verify-pages-builder-rollout-runtime-matrix-harness.mjs
verify-pages-builder-rollout-feature-preflight-harness.mjs
```

## Candidate boundary

Successful component execution produces:

```text
format = pages_reference_consumer_gate_candidate_v1
status = component_execution_passed_owner_review_pending
```

The candidate records successful correlation of the artifact/browser chain, matrix/browser chain and feature-preflight/browser+matrix chain. It also records that matrix profiles passed, both rollout settings cycles were restored, and the canonical feature-disabled catalog passed.

It still records:

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

It does not accept Forum Wave or mutate canonical source. A maintainer must still decide owner sign-off and rollback disposition before gate acceptance can change.

## Source evidence and guards

- `crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-evidence-harness-source.json` records the unexecuted four-packet candidate state.
- `crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-evidence-harness.mjs` locks the four-input chain, both rollout guards, canonical error separation, privacy boundary and pending-approval semantics.
- `crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json` keeps `execution_gate = pending`, `accepted = false`, provider health `unobserved`, Forum Wave blocked and FFA/FBA unpromoted.

## Maintainer execution

After producing the exact-source packets, the candidate runner requires:

```text
RUSTOK_PAGES_REFERENCE_GATE_ARTIFACT_HTTP_EVIDENCE
RUSTOK_PAGES_REFERENCE_GATE_BROWSER_EVIDENCE
RUSTOK_PAGES_REFERENCE_GATE_ROLLOUT_MATRIX_EVIDENCE
RUSTOK_PAGES_REFERENCE_GATE_ROLLOUT_FEATURE_PREFLIGHT_EVIDENCE
```

Optional output override:

```text
RUSTOK_PAGES_REFERENCE_GATE_OUTPUT
```

Suggested source guards, intentionally not run by this implementation slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-runtime-matrix-harness.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-feature-preflight-harness.mjs
node crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-evidence-harness.mjs
```

No tests, verifiers, Cargo commands, Node commands, browser runs, HTTP/GraphQL requests, workflows, CI, builds, formatting or migrations were run by this implementation slice.
