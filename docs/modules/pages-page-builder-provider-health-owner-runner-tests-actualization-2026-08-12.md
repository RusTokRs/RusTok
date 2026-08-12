# Pages / Page Builder provider-health owner-runner tests actualization — 2026-08-12

Status: `provider-health-observed-acceptance-runner-tested / synthetic-fail-closed-coverage-source-ready / focused-provider-health-ci-source-ready / live-provider-health-execution-pending / pages-gate-unaccepted`.

Base rechecked: `main@1c5eadab8d77695358bd5df304ec97cd4211d104`.

## Why this continuation exists

The provider-health source chain was already complete before this slice:

```text
exact deployment identity
  -> deployment metrics/evaluator
  -> binding owner acceptance
  -> Pages fail-closed binding and consumer narrowing
  -> deployed runtime evidence harness
  -> retrospective observed-health owner acceptance
  -> Pages reference-consumer gate review
```

The remaining live cursor is maintainer execution, not another provider-health architecture change. However, the production retrospective owner-decision runner at `scripts/evidence/accept-pages-builder-provider-health-runtime.mjs` previously had only static source verification. The source guard checked important strings and boundaries, but the runner itself was not exercised against constructed exact-lineage packets in CI.

This slice closes that verification gap without fabricating deployment evidence.

## Executable runner coverage

`scripts/evidence/accept-pages-builder-provider-health-runtime.test.mjs` constructs bounded synthetic packets under repository `target/` and invokes the production owner-acceptance runner as a child process.

The fixture does not copy the runner's decision functions. It derives checkout `HEAD`, reads the real identity/evaluator/binding/runtime contracts, recomputes every required source SHA-256 from the current checkout, writes predecessor packets, binds their actual byte lengths/SHA-256 values into the runtime packet, and then executes the production CLI.

The seven synthetic fail-closed cases are:

1. valid `accept_observed_runtime_evidence` produces `owner_accepted_observed_runtime_evidence_gate_review_pending`, while `pages_reference_consumer_gate_accepted = false` and `current_provider_health_asserted = false` remain explicit;
2. valid `reject` produces `owner_rejected_observed_runtime_evidence` and is not eligible for gate review;
3. a retained runtime source-hash tamper is rejected;
4. runtime evidence generated after its admitted historical health deadline plus skew is rejected;
5. a runtime claim that the Pages reference-consumer gate was already accepted is rejected;
6. a privacy/non-retention flag that claims raw evidence paths were retained is rejected;
7. a binding owner-acceptance RepoDigest that differs from the runtime exact deployment identity is rejected even when the runtime-retained binding packet hash is refreshed to match that tampered packet.

The contract for this source slice is:

`crates/rustok-pages/contracts/evidence/pages-builder-provider-health-observed-acceptance-runner-test-source.json`.

The anti-drift guard is:

`scripts/verify/verify-pages-builder-provider-health-owner-runner-tests.mjs`.

## Focused CI

`.github/workflows/pages-page-builder-provider-health.yml` is a read-only focused gate. It executes:

```text
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-identity.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-evaluator.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-owner-acceptance.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-runtime-harness.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-observed-acceptance.mjs
node scripts/verify/verify-pages-builder-provider-health-owner-runner-tests.mjs
node scripts/evidence/accept-pages-builder-provider-health-runtime.test.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
```

The synthetic runner suite and source guards are intended to run in ordinary CI without operator credentials or live infrastructure.

## Boundary of the evidence

This PR does **not** execute any live provider-health chain. In particular it does not:

- capture deployment identity from live targets;
- query Prometheus;
- execute the deployment evaluator against live observations;
- activate or change a Pages provider-health binding;
- issue GraphQL or HTTP probes;
- execute the runtime Playwright harness;
- create owner acceptance for a live deployment;
- assert current provider health;
- accept the Pages reference-consumer gate;
- admit or accept Forum Wave;
- promote FFA or FBA.

The synthetic Ready snapshot is only a valid input fixture for exercising the production acceptance CLI. It is not retained as evidence about any deployed Page Builder provider.

## Cursor after this slice

`provider-health-observed-acceptance-runner-tested` means the retrospective owner-decision runner now has executable fail-closed coverage in addition to static source guards.

The live parity cursor is unchanged and remains maintainer-owned:

1. execute exact deployment identity capture against the reviewed target inventory and immutable RepoDigest;
2. execute the exact deployment evaluator against the admitted backend target map;
3. take the binding owner decision and configure the exact accepted packet without extending freshness;
4. execute the deployed Pages provider-health runtime packet while the admitted historical lease is valid;
5. run the retrospective observed-health owner decision over those exact retained packets;
6. only then execute the rollout-only Pages reference candidate and take the separate Pages gate owner + rollback decision.

Live provider-health execution remains pending. Pages reference-consumer gate remains unaccepted.
