# Pages / Page Builder provider-health owner acceptance actualization — 2026-08-09

Status: `owner-acceptance-packet-source-ready / maintainer-execution-pending / server-owner-health-binding-blocked / production-consumer-health-binding-blocked / observed-health-acceptance-pending`.

## Cursor

This packet continues the provider-health chain after:

- deployment metrics/freshness source;
- exact deployment identity and expected-target inventory;
- deployment-bound Prometheus evaluator;
- typed Pages observed-health GraphQL/admin transport.

The remaining gap before server binding was an explicit owner decision over retained deployment evidence. A typed transport alone must not make an observed-health claim.

## Owner acceptance packet

The source contract is:

```text
crates/rustok-pages/contracts/evidence/pages-builder-provider-health-owner-acceptance-source.json
```

The maintainer runner is:

```text
scripts/evidence/accept-pages-builder-provider-health-deployment.mjs
```

It accepts only the retained evaluator output:

```text
format: page_builder_provider_health_deployment_evaluation_v1
status: deployment_health_evaluated_pages_binding_pending
```

The evaluation file must be a regular non-symlink file under repository `target/`, must report the exact checkout `HEAD`, and must retain source-file hashes that still match the checkout.

## Fail-closed evaluation admission

Before any decision can be recorded, the runner rechecks:

- canonical 40-character source commit equals checkout `HEAD`;
- immutable deployment image identity is a canonical `REPOSITORY@sha256:<digest>`;
- expected target count equals verified backend target count and remains within the admitted 1..64 inventory;
- retained query window is still inside the evaluator's 300..86400 second bounds;
- retained freshness window is still at least 60 seconds and no larger than the query window;
- retained identity age still covers the full query window and does not exceed the evaluator's maximum admitted age;
- `evaluated_at` and `identity_captured_at` are canonical ISO-8601 UTC values and agree with retained identity age;
- target mapping is complete;
- each retained target has exact current-source admission, no unexpected source in the SLO window, and Preview/Publish freshness ages no larger than the retained freshness window;
- Preview and Publish sample populations are both at least 20;
- cumulative histogram `+Inf` populations agree with terminal completion populations;
- provider observations are bounded and finite;
- evaluator thresholds remain 1500ms Preview, 3000ms Publish, 1% sanitize failure and 1% runtime error;
- state and degradation reasons are recomputed from the canonical policy;
- the retained SLO pass/fail fields are recomputed;
- the evaluator packet still claims no Pages observed health, Pages gate acceptance, Forum Wave, FFA or FBA promotion.

An owner cannot use the acceptance runner to bless a mismatched, partial, stale-source-shaped, freshness-invalid or already-promoted packet.

## Decision semantics

The runner requires a bounded operator identifier:

```text
--owner-id <A-Z/a-z/0-9/._- identifier, max 64 chars>
```

The identifier is an **operator assertion** recorded for accountability. This source does not claim a cryptographic signature or independent identity proof.

Two decisions are available:

```text
accept_for_pages_binding
reject
```

Acceptance additionally requires the explicit rollback action:

```text
restore_unobserved_provider_health
```

Example future maintainer execution:

```text
node scripts/evidence/accept-pages-builder-provider-health-deployment.mjs \
  --evaluation target/page-builder-provider-health-deployment-evaluation.json \
  --owner-id <operator-id> \
  --decision accept_for_pages_binding \
  --rollback-action restore_unobserved_provider_health
```

A rejection writes a retained rejection packet and carries no rollback-action value.

## Health state is not forced to ready

Owner acceptance validates provenance and policy evaluation; it does not rewrite provider state.

A canonical evaluator packet may report:

- `ready`;
- `degraded`;
- `unavailable`.

This is deliberate. Binding degraded/unavailable health is useful because Pages capability controls can then narrow or disable unsafe operations. Requiring `ready` would hide exactly the health states the transport exists to expose.

## Retained packet

Default output:

```text
target/pages-builder-provider-health-owner-acceptance.json
```

Accepted packets use:

```text
format: pages_builder_provider_health_owner_acceptance_v1
status: owner_accepted_server_binding_pending
```

Rejected packets use:

```text
status: owner_rejected_observed_health_binding
```

The packet retains:

- owner id and decision;
- explicit rollback action for acceptance;
- exact deployment id, image RepoDigest and source commit;
- admitted query/freshness windows and identity age;
- SHA-256 of the evaluator packet, never its raw path;
- source-hash verification result;
- Preview/Publish sample counts;
- canonical provider-health snapshot and SLO evaluation;
- source-file hashes for the acceptance implementation.

No free-text reason is retained.

## This does not perform server binding

An accepted owner packet can authorize a **future** server-owned binding slice, but it does not itself change GraphQL or any Page Builder consumer.

Future server binding must still revalidate:

- exact live source commit;
- exact reviewed deployment image RepoDigest;
- accepted packet status and decision;
- the retained provider snapshot;
- the configured acceptance artifact itself.

Any binding failure must restore the unobserved provider-health state rather than fabricate health.

## Pages remains `unobserved`

This source slice deliberately leaves all current production paths unchanged:

- Pages GraphQL still returns `provider_health_observed: false` and `provider_health: None`;
- authoritative SSR still builds `PageBuilderAdminProviderStatus::unobserved`;
- Pages workspace consumes rollout flags only;
- standalone browser-intent capability narrowing consumes rollout flags only;
- `pages_reference_consumer_gate` remains unaccepted;
- Forum Wave remains blocked;
- FFA/FBA remain unpromoted.

## Source guard

The fail-closed source guard is:

```text
crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-owner-acceptance.mjs
```

It locks the evaluator packet admission, owner decision vocabulary, explicit rollback action, source/deployment binding fields, query/freshness/identity-age admission, canonical health/SLO re-evaluation, source hash checks and continued production anti-promotion.

## Next cursor

```text
bounded process-local provider observation [source-ready]
-> deployment metrics + freshness [source-ready]
-> exact deployment identity + expected-target inventory [source-ready]
-> deployment health evaluator [source-ready]
-> typed observed-health transport [source-ready]
-> owner acceptance packet [source-ready / maintainer-execution-pending]
-> retained identity + evaluator + accepted owner packet [maintainer execution pending]
-> server provider-health binding [blocked on accepted owner packet]
-> UI / SSR / browser-intent health binding [blocked on server binding]
-> observed-health acceptance [pending]
```

## Validation boundary

Per maintainer instruction, tests were not run. No Node verifier, Cargo command, formatter, GraphQL/HTTP request, browser run, identity capture, Prometheus query, evaluator execution, owner acceptance execution, workflow or CI was executed by this slice.

Suggested maintainer source checks, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-owner-acceptance.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-transport.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-evaluator.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
```
