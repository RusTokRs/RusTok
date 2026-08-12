# Pages / Page Builder reference-consumer gate owner-runner tests actualization — 2026-08-12

Status: `reference-consumer-gate-owner-runner-tested / synthetic-fail-closed-coverage-source-ready / focused-provider-health-gate-ci-source-ready / live-gate-execution-pending / forum-wave-unaccepted`.

Base rechecked: `main@44e949f693dd199f12e62b891a409e4606f3abb8`.

## Why this continuation exists

The Pages reference-consumer gate already has a source-ready owner-decision runner:

```text
scripts/evidence/accept-pages-reference-consumer-gate.mjs
```

Its live inputs remain intentionally maintainer-owned:

```text
pages_reference_consumer_gate_candidate_v1
+ pages_builder_provider_health_observed_acceptance_v1
-> explicit owner gate decision + explicit rollback disposition
```

The production runner was guarded statically but did not have executable synthetic coverage comparable to the observed-health owner-acceptance runner. This slice closes that testability gap without fabricating a live candidate, live provider-health evidence or a real owner decision.

## Executable owner-runner coverage

`scripts/evidence/accept-pages-reference-consumer-gate.test.mjs` builds bounded synthetic predecessor packets under repository `target/` and invokes the production gate owner-decision CLI as a child process.

The fixture binds itself to checkout `HEAD`, recomputes the candidate execution contract and observed-health acceptance source hashes from the current checkout, copies the exact allowlisted source-guard/focused-test command identities and argv from the candidate contract, and keeps the candidate provider-health value `unobserved`.

The nine synthetic fail-closed cases are:

1. valid `accept_pages_reference_consumer_gate` + `retain_reference_consumer_candidate` emits `owner_accepted_pages_reference_consumer_gate` without mutating canonical source or accepting Forum Wave;
2. valid `reject` + `rollback_reference_consumer_candidate` emits `owner_rejected_pages_reference_consumer_gate` while recording that rollback was not executed;
3. accepted gate paired with the rollback disposition is rejected;
4. retained reference-candidate source hash tamper is rejected;
5. allowlisted candidate command argv drift is rejected;
6. candidate provider-health promotion from `unobserved` is rejected;
7. observed-health immutable RepoDigest mismatch against the candidate is rejected;
8. observed-health current-provider-health overclaim is rejected;
9. observed-health predecessor with gate-review eligibility revoked is rejected.

The production decision logic is not imported or reimplemented by the test.

## Focused CI

The existing `.github/workflows/pages-page-builder-provider-health.yml` is extended instead of creating another workflow. It remains `contents: read`, keeps one concurrency group per PR/ref with `cancel-in-progress: true`, and now runs the Pages gate acceptance source guard, this new anti-drift guard and the production-runner synthetic suite alongside the existing provider-health chain and plan parity.

This consolidation is intentional: gate-owner coverage depends on the observed-health predecessor contract, so keeping it in the same focused workflow avoids a second overlapping Pages/Page Builder evidence workflow.

## Evidence boundary

This is executable **synthetic runner evidence only**. It does not:

- execute artifact/HTTP, browser, rollout matrix or feature-preflight candidate evidence;
- execute deployment identity, Prometheus evaluation or provider-health runtime observation;
- create a real observed-health owner acceptance;
- make a real Pages gate owner decision;
- execute rollback;
- mutate `pages-reference-consumer-gate-source.json`;
- assert current provider health;
- accept Forum Wave;
- promote FFA or FBA.

The synthetic accepted output proves only that the production CLI accepts a structurally valid exact-checkout fixture and fails closed on the covered boundary violations. It is not retained as production gate evidence.

## Cursor after this slice

`reference-consumer-gate-owner-runner-tested` adds executable fail-closed CI coverage to the already source-ready decision runner. The live parity cursor is unchanged:

1. execute the rollout-only reference candidate on the exact reviewed source/deployment;
2. execute and owner-accept the exact provider-health evidence chain;
3. run the Pages gate owner decision over those exact retained packets and record the rollback disposition;
4. only an accepted live gate packet may become the Pages input for Forum Wave admission;
5. execute Forum browser/runtime/server-function evidence and admit the exact-source Forum Wave;
6. execute the observed control-plane Wave and subsequent owner review.

Live candidate and observed-health execution remain pending. The Pages source gate remains fail closed. Forum Wave remains unaccepted.
