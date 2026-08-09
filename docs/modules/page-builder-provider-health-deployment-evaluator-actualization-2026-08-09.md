# Page Builder provider-health deployment evaluator actualization — 2026-08-09

Status: `deployment-health-backend-evaluator-source-ready / exact-target-source-admission-source-ready / freshness-and-sample-floor-source-ready / pages-health-binding-blocked / maintainer-execution-pending`.

## Cursor

This packet continues:

- `page-builder-provider-health-runtime-observation-actualization-2026-08-09.md`;
- `page-builder-provider-health-deployment-metrics-actualization-2026-08-09.md`;
- `page-builder-provider-health-deployment-identity-actualization-2026-08-09.md`.

The predecessor slices established bounded process-local observations, platform Prometheus metrics, exact runtime source identity and a complete expected-target inventory contract. The remaining source gap was the deployment evaluator that admits only those exact targets, rejects mixed/stale source windows, computes the deployment SLO observations and retains a deployment-bound provider-health snapshot.

This slice closes that **source** gap. It does not execute the evaluator and does not promote Pages health.

## Evaluator input boundary

The runner is:

```text
scripts/evidence/evaluate-page-builder-provider-health-deployment.mjs
```

It requires:

```text
--identity FILE
--backend-map FILE
--prometheus-url URL
--window-seconds N
--freshness-seconds N
```

The identity packet must be the retained output from the exact-target deployment-identity capture and must:

- have format `page_builder_provider_health_deployment_identity_v1`;
- have status `deployment_identity_verified_health_evaluation_pending`;
- carry the exact current checkout source SHA;
- retain `inventory_complete = true`;
- have equal expected/verified target counts;
- retain the immutable image RepoDigest as the previously admitted maintainer-reviewed external deployment fact.

No source-only path can fabricate this input.

## Exact backend target mapping

The evaluator deliberately does not assume that Prometheus uses `instance`, `pod`, `host`, or any other specific target label.

Maintainer input supplies one complete mapping packet:

```json
{
  "schema_version": 1,
  "deployment_id": "prod-eu",
  "inventory_complete": true,
  "target_label": "instance",
  "common_matchers": {
    "job": "rustok-server"
  },
  "targets": [
    {
      "target_id": "server-a",
      "target_label_value": "10.0.0.10:5150"
    }
  ]
}
```

The target id set must equal the identity packet exactly. Target label values must be unique. Matchers are exact equality only; regex matchers are not accepted. Reserved metric-contract labels (`__name__`, `source_commit`, `operation`, `outcome`, `le`) cannot be supplied as topology matchers.

Raw matcher values are used for queries but are not retained in the output. The output retains target ids, matcher names and selector SHA-256 values.

## Exact-source admission across the whole SLO window

Current build identity alone is not sufficient because a rate window can otherwise straddle an older deployment that reused the same Prometheus target labels.

The evaluator therefore requires all of the following for every expected target:

1. current `rustok_page_builder_provider_build_info{source_commit="<identity-sha>"}` exists exactly once and equals `1`;
2. the admitted source build-info has samples inside the full query window;
3. `count_over_time(...source_commit!="<identity-sha>"[window])` returns no positive sample count;
4. each expected target resolves to a unique current backend series fingerprint;
5. partial target success is rejected.

The identity capture itself must predate the **entire** query window. This prevents the evaluator from treating pre-attestation metric history as admitted deployment health.

The evaluator also bounds identity age to 24 hours. A stale historical identity packet is not deployment authority indefinitely.

## Backend clock, query window and freshness

Prometheus `time()` is the evaluator clock source, avoiding local runner clock skew when deciding target freshness.

The admitted query window is maintainer-selected but bounded to:

```text
300s <= window <= 86400s
```

Freshness is also explicit and bounded:

```text
60s <= freshness <= window
```

For **every expected target**, both of these series must exist and be fresh:

```text
rustok_page_builder_provider_last_observation_unix_seconds{operation="preview"}
rustok_page_builder_provider_last_observation_unix_seconds{operation="publish"}
```

A missing target, missing operation, stale operation timestamp, or timestamp more than five seconds ahead of backend `time()` fails closed.

## Reset-aware deployment aggregation

The evaluator uses `increase(...)` over the admitted window for counters and histogram buckets. It never subtracts raw cumulative values.

For every admitted target it reads:

```text
increase(rustok_page_builder_provider_operation_completed_total[window])
increase(rustok_page_builder_provider_operation_duration_seconds_bucket[window])
```

The target-local results are then summed by operation/outcome and by cumulative histogram boundary.

The deployment sample floor is the same source policy as the process-local observation contract:

```text
preview terminal completions >= 20
publish terminal completions >= 20
```

Less than either floor fails closed; absence is not interpreted as healthy.

The deployment p95 is calculated from the **summed cumulative bucket increases**, not by averaging per-target p95 values. This preserves histogram semantics across multiple provider instances.

Failure-rate denominators remain aligned with the Rust provider-health policy:

- sanitize failure rate = `publish/sanitize_failed` / all Publish terminal completions;
- runtime error rate = Preview + Publish `runtime_failed` / all Preview + Publish terminal completions.

Unknown operation/outcome labels and non-finite or negative backend values fail closed.

## Provider-health policy parity

The evaluator source-locks the same pilot thresholds as `crates/rustok-page-builder/src/health.rs`:

```text
preview_p95_ms <= 1500
publish_p95_ms <= 3000
sanitize_failure_rate <= 0.01
runtime_error_rate <= 0.01
```

It emits the same degradation reasons:

- `provider_unhealthy` for Preview latency or runtime error breach;
- `sanitize_backpressure` for sanitize failure breach;
- `publish_backlog` for Publish latency breach.

Runtime error rate above two times the pilot maximum produces `unavailable`; other threshold failures produce `degraded`; no threshold failure produces `ready`.

The retained output contains both the deployment-bound snapshot and explicit SLO pass/fail evaluation.

## Retained evidence and privacy

Default output:

```text
target/page-builder-provider-health-deployment-evaluation.json
```

Retained fields include:

- deployment id, immutable image RepoDigest and exact source SHA;
- identity capture time and age;
- expected/verified backend target counts;
- query/freshness windows;
- target ids, selector hashes, source-window admission and freshness ages;
- Preview/Publish sample counts;
- computed SLO observations;
- provider-health snapshot and SLO evaluation;
- hashes of source files used by the evaluator;
- credential environment names only.

The output does **not** retain:

- raw Prometheus URL;
- raw PromQL;
- raw Prometheus responses;
- raw target matcher values;
- authorization/cookie/common-header values;
- tenant/page/revision/correlation identifiers.

## Pages remains `unobserved`

This slice deliberately does not connect evaluator output to:

- `provider_health_observed` in Pages GraphQL;
- `PageBuilderAdminProviderStatus::observed(...)` in Pages admin;
- `pages_reference_consumer_gate` acceptance;
- Forum Wave acceptance;
- FFA/FBA promotion.

A source-ready evaluator is not runtime evidence. Pages may consume observed health only after a retained evaluator packet is executed for the admitted deployment and the owner acceptance/binding step is made explicit.

## Source guard

Machine source contract:

```text
crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-evaluator-source.json
```

Fail-closed source guard:

```text
crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-evaluator.mjs
```

The guard locks:

- exact identity packet requirements;
- complete 1:1 backend target mapping;
- source-window anti-mixing checks;
- backend-clock freshness semantics;
- reset-aware aggregation;
- 20 + 20 sample floors;
- Rust health threshold/state parity;
- continued Pages anti-promotion.

## Next cursor

```text
bounded process-local observation [source-ready]
-> deployment metrics + freshness [source-ready]
-> deployment identity + expected-target inventory contract [source-ready]
-> deployment health backend evaluator [source-ready]
-> retained identity/evaluator runtime evidence [maintainer execution pending]
-> Pages provider-status binding + owner acceptance [blocked]
-> observed-health acceptance [pending]
```

## Validation boundary

Per maintainer instruction, tests were not run. No Cargo commands, Node verifiers, formatting, build, metrics scrape, Prometheus query, deployment identity capture, evaluator execution, GraphQL/HTTP request, browser run, workflow or CI was executed by this slice.

Suggested maintainer source checks, intentionally not run:

```bash
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-evaluator.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-identity.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-metrics.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
```
