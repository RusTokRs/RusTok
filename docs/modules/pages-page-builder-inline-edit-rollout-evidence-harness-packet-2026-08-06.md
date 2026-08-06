# Pages / Page Builder Inline Edit Rollout Evidence Harness

Date: 2026-08-06  
Status: `source-ready / maintainer-execution-pending`

## Scope

This packet defines the final source-only evidence boundary for tenant rollout of authenticated Pages inline editing. It starts only after a passing browser packet exists for the exact source commit and immutable deployment RepoDigest.

The harness does not change deployment or configuration, query monitoring systems, promote FFA or FBA, or perform rollback. Maintainers execute those actions through their existing operational owners and provide one bounded observation document to the assembler.

No rollout execution is claimed by this source packet.

## Source files

```text
crates/rustok-pages/contracts/evidence/pages-inline-edit-rollout-execution-contract.json
crates/rustok-pages/contracts/evidence/pages-inline-edit-rollout-evidence-harness-source.json
scripts/evidence/assemble-pages-inline-edit-rollout-evidence.mjs
crates/rustok-pages/scripts/verify/verify-pages-inline-edit-rollout-evidence-harness.mjs
```

## Required predecessor

Both phases require:

```text
format: pages_inline_edit_browser_execution_v1
status: browser_execution_passed_rollout_pending
```

The browser packet must bind the current source commit and an immutable deployment RepoDigest. Its rollout, FFA and FBA boundaries must still be open.

FBA additionally requires a previous rollout packet with:

```text
format: pages_inline_edit_rollout_execution_v1
phase: ffa
status: ffa_observation_passed_fba_pending
```

The FFA observation window must finish before the FBA observation window starts.

## External observation format

Maintainers provide a JSON object with:

```text
format: pages_inline_edit_rollout_observation_v1
status: maintainer_observation_recorded
phase: ffa | fba
source_commit: <40-character SHA>
deployment_image_digest: <immutable RepoDigest>
```

The observation is an attestation input collected from existing deployment, configuration and monitoring owners. The assembler validates internal consistency and predecessor identity; it does not independently attest an external orchestrator or monitoring backend.

## Environment and cohort

The input contains a bounded environment name and configuration profile. The output retains only their SHA-256 identities.

Tenant identities must already be represented as lowercase SHA-256 values. Raw tenant UUIDs and tenant names are rejected by contract and are not copied to output.

The cohort must use exactly:

```text
pages.builder.inline_edit.enabled
```

It must contain:

- at least one enabled tenant identity;
- at least one disabled control tenant identity;
- no duplicate identities;
- no overlap between enabled and disabled control cohorts.

The harness observes the existing configuration decision. It does not introduce a runtime flag owner or change how configuration reaches the application.

## Admission facts

Every observation must affirm that inline editing still requires:

```text
pages_module_enabled_required
direct_user_required
authenticated_session_required
pages_update_required
```

These are bounded rollout review facts. Runtime authorization and Pages owner checks remain authoritative.

## Observation window

Every phase requires exact UTC start and end timestamps and a positive whole-second duration matching those timestamps.

The harness sets no invented product-specific minimum duration. The rollout owner remains responsible for choosing and reviewing the observation window before recording the input.

## Monitoring series

The observation must include exactly four reviewed series:

```text
save_conflicts
authorization_denials
grant_verification_failures
client_load_failures
```

Each series contains a non-negative observed count and a non-negative reviewed threshold. Assembly fails closed when any observed count exceeds its threshold.

Only counts, thresholds and pass facts are retained. Raw monitoring logs, alert payloads, correlation identifiers and request details are not retained.

## Rollback identity

Both phases require:

- a SHA-256 rollout owner identity;
- an immutable rollback image RepoDigest;
- a rollback image different from the active deployment image.

FFA records the rollback target but must not claim the later rehearsal.

FBA requires:

```text
rehearsal.executed: true
rehearsal.passed: true
```

The assembler does not execute rollback and does not retain a raw owner name.

## Review facts

Both phases require affirmative review of:

- browser evidence;
- configuration snapshot;
- monitoring observation;
- rollout owner approval.

FFA must leave `ffa_packet_reviewed` false because that packet is being produced.

FBA requires `ffa_packet_reviewed` true and consumes the exact previous FFA packet as an input.

## FFA output

Example command:

```bash
node scripts/evidence/assemble-pages-inline-edit-rollout-evidence.mjs \
  --phase ffa \
  --browser target/pages-inline-edit-browser-evidence.json \
  --observation /secure/input/pages-inline-edit-rollout-ffa-observation.json \
  --output target/pages-inline-edit-rollout-ffa-evidence.json
```

Required output identity:

```text
format: pages_inline_edit_rollout_execution_v1
phase: ffa
status: ffa_observation_passed_fba_pending
```

A passing FFA packet records that the reviewed cohort rollout and observation were performed externally. It does not complete FBA.

## FBA output

Example command:

```bash
node scripts/evidence/assemble-pages-inline-edit-rollout-evidence.mjs \
  --phase fba \
  --browser target/pages-inline-edit-browser-evidence.json \
  --ffa target/pages-inline-edit-rollout-ffa-evidence.json \
  --observation /secure/input/pages-inline-edit-rollout-fba-observation.json \
  --output target/pages-inline-edit-rollout-fba-evidence.json
```

Required output identity:

```text
format: pages_inline_edit_rollout_execution_v1
phase: fba
status: fba_rollout_evidence_complete
```

FBA closes the rollout evidence chain only when the predecessor FFA packet, later observation window, reviewed thresholds and successful rollback rehearsal all pass.

## Retained evidence

The assembler retains:

- current source commit;
- source-file SHA-256 hashes;
- immutable active and rollback image RepoDigests;
- hashes and byte sizes of input packets;
- SHA-256 environment, profile, owner and tenant identities;
- observation timestamps and duration;
- monitoring counts, thresholds and pass facts;
- admission, approval and rollout boundary booleans.

It does not retain:

- raw tenant IDs or names;
- raw environment, profile or owner values;
- Authorization or Cookie values;
- tokens, sessions, grants, proofs or signing keys;
- database URLs, deployment credentials or configuration secrets;
- raw monitoring logs or alert payloads;
- raw browser HTML or request/response bodies.

## Source guard

```bash
node crates/rustok-pages/scripts/verify/verify-pages-inline-edit-rollout-evidence-harness.mjs
```

The guard verifies the contract chain, assembler mutation boundary, FFA/FBA separation, cohort and monitoring requirements, privacy markers and active execution-plan cursor.

## Promotion boundary

The evidence files describe actions already performed by maintainers. Generating either packet does not itself mutate tenant configuration, deploy an image, promote a cohort or execute rollback.

`fba_rollout_evidence_complete` is the terminal evidence status for this Pages inline-edit continuation cursor. Actual production ownership and any later operational changes remain outside the source harness.
