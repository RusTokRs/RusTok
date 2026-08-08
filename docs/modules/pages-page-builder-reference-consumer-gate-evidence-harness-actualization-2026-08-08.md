# Pages / Page Builder reference-consumer gate evidence harness actualization — 2026-08-08

Status: `candidate-harness-source-ready / exact-source-input-chain-required / owner-review-pending / gate-acceptance-pending`

Base rechecked: `main@ae40411b5d2c6aae978f98860c773c1e8a08e57b`.

## Purpose

The canonical Pages / Page Builder cursor now points to execution and acceptance of `pages_reference_consumer_gate`. Existing source already provides detailed artifact/HTTP and browser execution packets plus focused metadata, sanitizer/resource-limit, cache-correlation and provider-status test seams. The remaining source gap was a bounded way to correlate those results into one exact-source candidate without turning an implementation agent into the rollout or approval authority.

This slice adds that correlation boundary.

## Machine execution contract

`crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-execution-contract.json` defines one maintainer-run candidate packet.

It requires two existing machine packets:

1. `pages_inline_edit_artifact_http_execution_v1` with status `artifact_http_execution_passed_browser_rollout_pending`;
2. `pages_inline_edit_browser_execution_v1` with status `browser_execution_passed_rollout_pending`.

Both packets must match the exact checked-out Git HEAD. The browser packet must use the same immutable deployment RepoDigest as the artifact/HTTP packet and must bind the exact artifact/HTTP input file hash. The artifact/HTTP packet continues to carry the anonymous authoring-exclusion evidence, while the browser packet carries launch, save, replacement revision/hash, reload, replay, stale-write and expiry behavior.

## Bounded runner

`scripts/evidence/pages-reference-consumer-gate-evidence.mjs`:

- uses `spawnSync` with `shell: false`;
- executes only contract-declared `node` source guards and `cargo test` commands;
- validates the exact Git HEAD before execution;
- rejects missing, symlinked or oversized required source files;
- hashes every required source file;
- removes stale candidate output before execution;
- retains only command id/argv, exit status and stdout/stderr byte length + SHA-256, never raw command output;
- writes atomically inside repository `target/`;
- does not persist raw artifact/browser packets or credential/session/grant/proof values.

The focused Cargo set is deliberately bounded to:

- stale Pages metadata revision rejection before patch transport;
- metadata-only save preserving dirty Fly state;
- Page Builder publish sanitization tests;
- Page Builder global static-publish resource-limit tests;
- Pages publish/rollback cache-correlation regression;
- Page Builder admin provider degraded-profile tests.

The runner also executes the source guards already required by the Pages reference-consumer gate.

## Candidate boundary

Successful component execution produces:

```text
format = pages_reference_consumer_gate_candidate_v1
status = component_execution_passed_owner_review_pending
```

The candidate owner review remains pending.

The candidate explicitly records:

```text
provider_health = unobserved
owner_signoff = pending
rollback_decision = pending
gate_acceptance = pending
```

It does not accept `pages_reference_consumer_gate`.

It does not claim provider health.

It does not promote FFA/FBA.

It does not accept Forum Wave evidence or mutate canonical source.

A later maintainer review must independently decide owner sign-off and rollback disposition before the source gate can be changed from `accepted = false`.

## Source evidence and guard

- `crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-evidence-harness-source.json` records the unexecuted source state.
- `crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-evidence-harness.mjs` locks the contract, runner, exact input-chain requirements, command allow-list, privacy boundary and pending-approval semantics.
- `crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json` registers the candidate harness while retaining `execution_gate = pending`, `accepted = false` and provider health `unobserved`.

## Maintainer execution

Suggested source guard, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-evidence-harness.mjs
```

After producing the prerequisite artifact/HTTP and browser evidence packets, the candidate runner can be invoked by supplying their paths through:

```text
RUSTOK_PAGES_REFERENCE_GATE_ARTIFACT_HTTP_EVIDENCE
RUSTOK_PAGES_REFERENCE_GATE_BROWSER_EVIDENCE
```

Optional candidate output override:

```text
RUSTOK_PAGES_REFERENCE_GATE_OUTPUT
```

No tests, verifiers, Cargo commands, Node commands, browser runs, HTTP requests, workflows or CI were run by this implementation slice.
