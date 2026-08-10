# Pages / Page Builder reference-consumer gate acceptance actualization — 2026-08-10

Status: `reference-consumer-gate-acceptance-source-ready / dual-evidence-lineage-source-ready / owner-signoff-source-ready / rollback-disposition-source-ready / forum-wave-admission-source-ready / maintainer-execution-pending / forum-wave-blocked`.

Base rechecked: `main@2dcffcd7c20c9deee58e6912d7b18ea761e149c5`.

## Purpose

The reference-consumer candidate and observed provider-health evidence intentionally prove different things.

The existing `pages_reference_consumer_gate_candidate_v1` proves the four configured rollout profiles, canonical `FEATURE_DISABLED` preflight, Pages-owned reads, browser-intent bypass resistance, focused tests and source guards while provider health remains deliberately `unobserved`.

The newer `pages_builder_provider_health_observed_acceptance_v1` proves that an exact deployed Pages/Page Builder chain actually observed Ready, Degraded or Unavailable provider health and that the consumers narrowed capabilities according to the canonical provider runtime policy. That packet is retrospective and does not assert current provider health.

Gate acceptance must require both branches. It must not replace the rollout-only candidate with observed-health evidence or reinterpret `FLY_CAPABILITY_DENIED` as `FEATURE_DISABLED`.

## Acceptance source

`crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json` defines the source-only owner-decision boundary.

`scripts/evidence/accept-pages-reference-consumer-gate.mjs` requires:

- `pages_reference_consumer_gate_candidate_v1 / component_execution_passed_owner_review_pending`;
- `pages_builder_provider_health_observed_acceptance_v1 / owner_accepted_observed_runtime_evidence_gate_review_pending`;
- the same exact source commit and immutable RepoDigest across both packets and checkout `HEAD`;
- candidate `source_sha256` matching the candidate execution contract and current checkout;
- observed-health acceptance `source_files` matching its source contract and current checkout;
- the exact command ids/programs/argv declared by the candidate execution contract, with every retained source-guard and focused-test status equal to zero;
- the exact four bounded candidate input hash records (`artifact_http`, `browser`, `rollout_matrix`, `rollout_feature_preflight`);
- all rollout matrix and canonical feature-preflight booleans retained as passed;
- candidate `provider_health = unobserved` and owner/gate decisions still pending;
- observed-health decision `accept_observed_runtime_evidence`;
- a bounded observed-health deployment id and the same immutable RepoDigest as the candidate;
- observed-health packet `eligible_for_pages_gate_review = true` while predecessor Pages gate/sign-off/rollback flags remain false;
- no current provider-health assertion, no health lease extension and unchanged live provider binding;
- the committed source gate remains fail closed (`accepted=false`, execution pending, rollout evidence health `unobserved`, Forum Wave still blocked) before any decision packet is written.

The runner performs no HTTP, GraphQL, browser, Prometheus, evaluator, Cargo or test execution. It only revalidates bounded retained evidence and checkout files.

## Owner decision and rollback disposition

The owner decision is explicit:

```text
accept_pages_reference_consumer_gate
reject
```

The rollback disposition is also explicit:

```text
retain_reference_consumer_candidate
rollback_reference_consumer_candidate
```

An accepted gate requires `retain_reference_consumer_candidate`. A rejected gate requires `rollback_reference_consumer_candidate`.

This packet records the disposition only. It does not execute rollback, redeploy anything, mutate rollout settings or change provider-health binding.

## Output boundary

A real maintainer execution can produce:

```text
format = pages_reference_consumer_gate_acceptance_v1
status = owner_accepted_pages_reference_consumer_gate
```

or:

```text
status = owner_rejected_pages_reference_consumer_gate
```

The accepted packet is machine evidence for downstream admission. It does not automatically accept Forum Wave and does not promote FFA/FBA.

The committed source gate remains:

```text
pages_reference_consumer_gate_source.accepted = false
execution_gate = pending
provider_health = unobserved
```

That `provider_health = unobserved` remains correct for the rollout-candidate branch. Observed provider health is a separate exact-source acceptance input; source inspection must not fabricate it.

## Provider-health semantics

Gate acceptance accepts historical observed-health evidence for the same exact source/deployment. It does not assert current provider health and does not extend `health_valid_until`.

Ready, Degraded or Unavailable historical evidence may be valid if the retained consumer outcomes matched the canonical policy. Gate acceptance is therefore evidence acceptance, not a claim that the provider is currently Ready.

## Forum / promotion boundary

Source gate remains fail closed:

- `pages_reference_consumer_gate_source.accepted = false`;
- rollback action is not executed by the decision runner;
- FFA/FBA promotion remains unclaimed;
- canonical source is not mutated automatically.

The next downstream source boundary is now explicit:

```text
accepted Pages gate packet
-> forum_page_builder_wave_admission_v1
-> observed Forum control-plane Wave
```

`crates/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-source.json` requires the accepted gate packet together with exact-source Forum browser, runtime-authorization and server-function evidence. Forum Wave admission is source-ready but maintainer execution remains pending, so the observed Wave is still blocked on admitted exact-source inputs.

Only maintainer-produced accepted gate evidence can enter that admission runner, and admission itself still does not accept Forum Wave.

## Validation boundary

Source verifier:

```text
crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-acceptance.mjs
```

It locks the dual packet lineage, exact source/RepoDigest binding, exact candidate command records, fail-closed source gate, historical-health semantics, explicit owner/rollback decisions, the downstream Forum admission cursor and non-promotion boundaries.

Tests were not run. No Node verifier, Cargo command, formatter, build, GraphQL/HTTP request, browser/Playwright run, Prometheus/evaluator execution, provider-health owner decision, reference candidate execution, Pages gate decision, Forum Wave admission, observed Forum Wave, workflow or CI run was performed by this slice.
