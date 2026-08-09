# Page Builder provider-health deployment metrics actualization — 2026-08-09

Status: `deployment-aggregatable-metrics-source-ready / freshness-signal-source-ready / exact-deployment-identity-open / pages-health-binding-blocked / execution-pending`.

## Cursor

This packet continues `page-builder-provider-health-runtime-observation-actualization-2026-08-09.md`.

The predecessor slice made the canonical Fly service retain a bounded process-local Preview/Publish observation window, but intentionally kept Pages health `unobserved` because a restartable in-process window is not deployment authority.

This slice closes the next bounded source gap: the same terminal Preview/Publish observations are now exported through the platform Prometheus registry in a shape that can be aggregated across scrape targets and checked for freshness without adding a second observability stack.

## Platform-owned metric source

`rustok-telemetry` now owns three fixed-cardinality Page Builder provider series:

```text
rustok_page_builder_provider_operation_duration_seconds{operation="preview|publish"}
rustok_page_builder_provider_operation_completed_total{operation="preview|publish",outcome="..."}
rustok_page_builder_provider_last_observation_unix_seconds{operation="preview|publish"}
```

Allowed terminal outcomes are exactly:

```text
succeeded
sanitize_failed
runtime_failed
other_failed
```

The duration histogram explicitly contains the current pilot latency thresholds (`1.5s` Preview and `3.0s` Publish), so deployment aggregation does not need a second histogram policy.

The default Page Builder runtime telemetry records the platform metric and the predecessor process-local window from the same matched terminal call. `LoadProject` remains excluded from the provider Preview/Publish SLO contract.

## Cardinality and ownership

Application metrics deliberately do not include tenant, page, revision, correlation, instance or deployment labels.

Target/instance/deployment identity belongs to scrape/discovery infrastructure. This avoids turning user/content identifiers into Prometheus cardinality while still allowing a metrics backend to aggregate or preserve per-target series through its own labels.

`rustok-page-builder` depends on `rustok-telemetry` only under its existing `server` feature. Non-server consumers keep the provider crate free of the telemetry dependency.

## Deployment aggregation contract

The exported series support reset-aware deployment calculations such as:

```text
Preview p95:
histogram_quantile(0.95,
  sum by (le) (
    rate(rustok_page_builder_provider_operation_duration_seconds_bucket{operation="preview"}[<window>])
  )
)

Publish p95:
histogram_quantile(0.95,
  sum by (le) (
    rate(rustok_page_builder_provider_operation_duration_seconds_bucket{operation="publish"}[<window>])
  )
)

Sanitize failure rate:
sum(increase(rustok_page_builder_provider_operation_completed_total{operation="publish",outcome="sanitize_failed"}[<window>]))
/
sum(increase(rustok_page_builder_provider_operation_completed_total{operation="publish"}[<window>]))

Runtime error rate:
sum(increase(rustok_page_builder_provider_operation_completed_total{outcome="runtime_failed"}[<window>]))
/
sum(increase(rustok_page_builder_provider_operation_completed_total[<window>]))
```

Counter/histogram resets must be handled with range functions (`rate`/`increase`) rather than raw cumulative subtraction.

This packet does not choose a production query window or claim a live backend query result. Those values become authoritative only when the exact deployment/source observation packet is bound.

## Freshness contract

`rustok_page_builder_provider_last_observation_unix_seconds` exposes the latest terminal observation independently for Preview and Publish on each scrape target.

A future deployment evaluator must:

1. preserve scrape target identity instead of collapsing freshness first;
2. require both Preview and Publish freshness for every expected active target admitted into the deployment observation set;
3. fail closed when an expected target is missing or stale for the evaluator's admitted freshness bound;
4. only then aggregate latency/error series for provider-health evaluation.

The repository does not yet own an exact expected-target inventory, deployment digest binding or freshness-age admission value for Pages provider health. Therefore this slice provides the aggregation/freshness **signal contract**, not the final observed-health authority.

## Deliberately unpromoted Pages health

Pages remains `unobserved`.

This slice does not connect Prometheus metrics to:

- `provider_health_observed` in Pages GraphQL;
- `PageBuilderAdminProviderStatus::observed(...)` in Pages admin;
- `pages_reference_consumer_gate` acceptance;
- Forum Wave acceptance;
- FFA/FBA promotion.

The next source cursor is exact source/deployment observation identity plus expected-target inventory. Only that authority may define the admitted query/freshness window and produce a deployment-bound `ProviderHealthSnapshot` for Pages.

## Source evidence

Production source:

- `crates/rustok-telemetry/src/page_builder_provider_metrics.rs`;
- `crates/rustok-telemetry/src/lib.rs`;
- `crates/rustok-page-builder/src/runtime_telemetry.rs`;
- `crates/rustok-page-builder/Cargo.toml`.

Machine evidence:

- `crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-metrics-source.json`.

Fail-closed source guard:

- `crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-metrics.mjs`.

The guard also retains the anti-promotion boundary: Pages GraphQL/admin must still be unobserved until exact deployment observation authority exists.

## Next cursor

```text
bounded process-local observation [source-ready]
-> deployment-aggregatable metrics + freshness signal [source-ready]
-> exact source/deployment identity + expected-target inventory [open]
-> deployment health evaluator / Pages provider-status transport [blocked]
-> retained runtime evidence [maintainer execution]
-> observed-health acceptance [pending]
```

## Validation boundary

Per maintainer instruction, tests were not run. No Cargo commands, Node verifiers, formatting, Prometheus scrapes, backend queries, GraphQL/HTTP requests, browser runs, workflows, CI or production observations were executed.

Suggested maintainer commands, intentionally not run:

```bash
cargo test -p rustok-telemetry page_builder_provider
cargo test -p rustok-page-builder runtime_telemetry
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-metrics.mjs
cargo check -p rustok-page-builder --all-targets
```
