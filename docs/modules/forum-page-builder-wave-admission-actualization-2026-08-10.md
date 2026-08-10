# Forum / Page Builder Wave admission actualization — 2026-08-10

Status: `forum-wave-admission-source-ready / accepted-pages-gate-required / exact-forum-evidence-lineage-required / maintainer-execution-pending / observed-control-plane-wave-pending`.

Base rechecked: `main@535c6d4a3aca4412c27c69453e08c6942c281ac9`.

## Purpose

The Pages reference-consumer gate now has a source-ready explicit owner decision packet, but Forum Wave 1 still had only a string blocker:

```text
blocked_by = pages_reference_consumer_gate
```

Nothing in Forum source consumed `pages_reference_consumer_gate_acceptance_v1`, and nothing correlated that accepted gate to the existing Forum browser, runtime-authorization and deployed server-function evidence packets.

This slice closes that source gap without executing the Pages gate, any Forum evidence harness or the observed control-plane Wave.

## Admission source

New contract:

```text
crates/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-source.json
```

New bounded runner:

```text
scripts/evidence/admit-forum-page-builder-wave.mjs
```

The runner requires all four future machine packets:

1. `pages_reference_consumer_gate_acceptance_v1 / owner_accepted_pages_reference_consumer_gate`;
2. `forum_page_builder_browser_execution_v1 / browser_execution_passed_runtime_evidence_pending`;
3. `forum_page_builder_runtime_authorization_execution_v1 / runtime_authorization_execution_passed_wave_pending`;
4. `forum_page_builder_server_fn_deployment_attestation_v1 / server_fn_deployment_attestation_passed_wave_pending`.

All four must carry the same exact checkout source commit.

The Pages gate, Forum browser packet and Forum server-function attestation must also carry the same immutable RepoDigest. The runtime-authorization packet is source-bound but does not independently claim deployment identity, so the admission runner does not fabricate a deployment binding for it.

## Pages gate admission

The Pages input must retain:

```text
decision = accept_pages_reference_consumer_gate
rollback_decision = retain_reference_consumer_candidate
gate.accepted = true
owner_signoff_satisfied = true
rollback_decision_satisfied = true
```

Its source hashes must match `pages-reference-consumer-gate-acceptance-source.json` and the current checkout.

The gate packet must still say that it did not mutate canonical source, execute rollback, accept Forum Wave or promote FFA/FBA.

An accepted Pages gate is therefore a prerequisite for Forum Wave admission, not Forum Wave acceptance itself.

## Forum browser recheck

The admission runner revalidates the browser packet against the browser execution contract and current checkout.

It requires the exact profile set:

```text
full
preview_off
properties_off
forum_disabled
no_read
```

Every profile must have `passed = true` and zero critical failures. The retained facts must prove:

- full profile: Forum block admission, owner validation rejection, owner normalization, Fly undo/redo, owner preview readiness and Pages save;
- preview-off: authoring/properties remain available while owner preview is not admitted;
- properties-off: Forum block/property authoring is not admitted;
- Forum-disabled: Forum contribution panels are absent;
- no-read: Forum contribution/property admission is denied.

Storage-state contents, profile URLs and secrets remain unretained; only bounded hashes/byte lengths are admitted.

## Runtime authorization recheck

The runtime packet source hashes must match its execution contract and checkout.

Every retained command record must match the execution contract by exact command id, program and argv, with exit status zero. Raw stdout/stderr remain unretained; only bounded byte lengths and SHA-256 values are admitted.

The runtime packet must continue to say that browser execution, deployment attestation, provider SLO health and observed Wave are not claimed by that packet alone.

## Deployed server-function recheck

The server-function packet must prove that the live response reported the exact checkout source commit and must carry the same immutable RepoDigest as the Pages gate and Forum browser packet.

The exact scenario set remains contract-owned. The authorized scenario must retain the contract-required HTTP status; credential values and raw response bodies remain unretained.

The `origin_to_repo_digest_binding` remains an explicit maintainer-reviewed external fact. This slice does not upgrade it into a cryptographic claim.

## Admission output

Successful maintainer execution can produce:

```text
format = forum_page_builder_wave_admission_v1
status = forum_wave_inputs_admitted_observed_control_plane_pending
```

The output retains only bounded input packet byte lengths/SHA-256 values, current source hashes, source commit, deployment id and immutable RepoDigest.

It records that the accepted Pages gate and Forum browser/runtime/server-function evidence are correlated for the same exact source/deployment boundary.

It does **not** materialize the final Wave packet.

## Observed Wave boundary

The existing Forum Wave still requires:

```text
control_plane.audit_trail
fallback.profiles
observability.metrics
observability.traces
rollback.decision
approvals
waivers
```

Those live sections remain absent from source-ready evidence.

The observed control-plane Wave remains pending after admission. Wave admission does not run rollout mutation, create live metrics/traces, record approvals, make a rollback keep decision, accept Forum Wave or promote FFA/FBA.

The canonical `forum-wave1-rollout-evidence.json` remains:

```text
mode = source_ready
provenance = synthetic_fixture
execution_status = not_run_by_implementation_agent
observed_run.status = not_run
```

It now additionally declares the required accepted Pages gate packet and the required Wave admission packet.

## Guard

New source verifier:

```text
scripts/verify/verify-forum-page-builder-wave-admission.mjs
```

It locks the four-packet lineage, same exact checkout source commit, same immutable RepoDigest where deployment identity exists, exact browser/runtime/server-function source contracts, non-retention boundaries and the continued `observed_run = not_run` state.

`verify-forum-wave-plan-sync.mjs` is also actualized so the Forum programme ledger cannot regress to a string-only Pages blocker.

## Validation boundary

Tests were not run. No Node verifier, Cargo command, formatter, build, GraphQL/HTTP request, Playwright/browser run, Forum runtime-authorization command, server-function attestation request, Pages gate decision, Wave admission runner, observed control-plane Wave, workflow or CI run was performed by this slice.
