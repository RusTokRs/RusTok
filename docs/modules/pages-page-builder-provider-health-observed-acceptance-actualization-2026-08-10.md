# Pages / Page Builder observed-health owner acceptance actualization — 2026-08-10

Status: `provider-health-observed-acceptance-source-ready / retrospective-runtime-evidence-review-source-ready / exact-predecessor-chain-source-ready / health-lease-non-extension-source-ready / maintainer-execution-pending / pages-gate-acceptance-pending`.

## Why this continuation exists

PR #3424 made the observed provider-health runtime evidence harness source-ready. A successful maintainer run can now produce `pages_builder_provider_health_runtime_evidence_v1` with status `observed_runtime_evidence_owner_review_pending`, but that packet intentionally does not contain an owner decision.

This slice closes that final provider-health source gap with an explicit observed-health owner acceptance runner. It does not execute the runtime harness or make the decision for the maintainer.

## Acceptance meaning

The accepted decision is:

```text
accept_observed_runtime_evidence
```

This is a **retrospective evidence decision**. It means the owner accepts that the exact-deployment runtime evidence is internally consistent and suitable as an input to later Pages gate review.

It does **not** mean:

- the provider is currently `Ready`;
- the historical `health_valid_until` lease is still live;
- the lease is restarted or extended;
- Pages server binding is newly authorized or changed;
- the Pages reference-consumer gate is accepted;
- the reference-gate owner sign-off or rollback decision is satisfied.

`Degraded` or `Unavailable` runtime evidence may therefore be accepted when the observed consumers correctly enforce the canonical narrowing policy. Acceptance is about the evidence and behavior, not promotion of the health state.

## Exact predecessor chain

The runner requires four maintainer-supplied evidence packets:

```text
page_builder_provider_health_deployment_identity_v1
page_builder_provider_health_deployment_evaluation_v1
pages_builder_provider_health_owner_acceptance_v1
pages_builder_provider_health_runtime_evidence_v1
```

It fails closed unless:

- runtime `source_commit` equals checkout `HEAD`;
- source hashes retained by the runtime packet exactly match the runtime execution contract and checkout;
- runtime-retained identity/evaluation/binding-acceptance byte lengths and SHA-256 values match the supplied packets;
- source commit, deployment id and immutable RepoDigest match across all four packets;
- the binding owner packet is the accepted `accept_for_pages_binding` packet with rollback `restore_unobserved_provider_health`;
- the binding packet's evaluation SHA matches the supplied evaluation;
- runtime accepted health/SLO and `health_valid_until` match the binding packet;
- runtime `generated_at` is inside the historical health deadline plus the existing five-second clock-skew tolerance.

The current wall clock may be later than `health_valid_until`. Owner review is retrospective and must not manufacture a new lease.

## Runtime behavior revalidation

The acceptance runner revalidates the retained runtime behavior rather than trusting a single boolean:

- configured rollout was `all_on`;
- GraphQL observed provider health and state match the accepted snapshot;
- Preview/Properties/Publish preflight results match canonical Ready/Degraded/Unavailable behavior;
- narrowed capabilities use `feature-disabled / FEATURE_DISABLED`;
- workspace provider/capability controls match the state;
- authoritative SSR Preview is allowed for Ready/Degraded and blocked before dispatch for Unavailable;
- standalone browser-intent denials match Degraded/Unavailable expectations and retain the mismatched-page-id non-mutating fallback;
- provider health is still observed after the consumer probes;
- rollout settings and Publish persistence were not mutated;
- privacy/non-retention and anti-promotion flags remain fail-closed.

The runner performs no GraphQL, HTTP, browser, Prometheus, deployment or binding request itself.

## Output

Accepted output:

```text
format = pages_builder_provider_health_observed_acceptance_v1
status = owner_accepted_observed_runtime_evidence_gate_review_pending
```

Rejected output:

```text
status = owner_rejected_observed_runtime_evidence
```

The output retains exact source/deployment identity, hashes of all supplied evidence packets, the historical health deadline, accepted health/SLO snapshot and the bounded operator-asserted owner id. Raw input paths, HTTP bodies, browser bodies, credentials, cookies and storage-state contents are not retained.

The output explicitly records:

```text
live_binding_action = unchanged
health_lease_extended = false
current_provider_health_asserted = false
pages_reference_consumer_gate_accepted = false
```

## Pages gate boundary

An accepted packet is eligible for a future Pages reference-consumer gate review, but this slice does not accept the Pages reference-consumer gate. The existing gate remains `accepted = false` and retains `provider_health = unobserved` in source/retained execution state until maintainers execute the exact chain and separately review the gate.

Forum Wave therefore remains blocked, and FFA/FBA promotion remains unclaimed.

## Source evidence

```text
crates/rustok-pages/contracts/evidence/pages-builder-provider-health-observed-acceptance-source.json
scripts/evidence/accept-pages-builder-provider-health-runtime.mjs
crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-observed-acceptance.mjs
```

## Validation boundary

Tests were not run. Node verifiers, Cargo commands, formatting, builds, GraphQL/HTTP requests, Playwright/browser runs, deployment identity capture, Prometheus queries, evaluator execution, binding owner acceptance, observed runtime evidence and observed-health owner acceptance were intentionally not executed.

Suggested maintainer source checks, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-observed-acceptance.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-runtime-harness.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
```

Runtime execution and the owner decision remain maintainer-owned.
