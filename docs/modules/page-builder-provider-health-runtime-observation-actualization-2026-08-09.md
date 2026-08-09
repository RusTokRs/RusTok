# Page Builder provider-health runtime observation actualization — 2026-08-09

Status: `process-local-runtime-observation-source-ready / deployment-observed-health-open / pages-health-binding-blocked / execution-pending`.

## Recheck result

The Pages / Page Builder parity recheck confirmed that rollout ownership, degraded controls, the four-profile runtime-matrix harness, canonical `FEATURE_DISABLED` preflight and reference-consumer candidate are already source-ready. The remaining provider-health source gap was real rather than stale plan text:

- `ProviderHealthSnapshot` and the pilot SLO evaluator already existed;
- the canonical Fly service already emitted started/succeeded/failed runtime telemetry for preview rendering and project save;
- default composition still selected `NoopPageBuilderRuntimeTelemetry`;
- Pages GraphQL explicitly reported `provider_health_observed = false` and Pages admin constructed `PageBuilderAdminProviderStatus::unobserved(...)`.

This slice activates the existing provider runtime seam without promoting local process observations into deployment-wide health.

## Source-ready runtime observation boundary

Default `compose_fly_page_builder_handlers(...)` now installs `ProviderHealthRuntimeTelemetry` on the canonical Fly-backed service.

The retained SLO window is deliberately bounded and process-local:

- at most `256` completed Preview samples;
- at most `256` completed Publish samples;
- no snapshot before at least `20 Preview` and `20 Publish` samples are present;
- process restart clears the observation window and returns the provider to `unobserved` until the sample floor is rebuilt;
- unmatched or over-capacity pending calls are bounded separately and cannot create an unbounded correlation map.

The current operation mapping is exact:

```text
RenderPreview -> Preview
SaveProject   -> Publish
LoadProject   -> excluded from the Preview/Publish SLO window
```

The window feeds the existing `ProviderHealthSnapshot::evaluate(...)` and therefore reuses the declared pilot thresholds instead of introducing a second health policy.

## Measurement limits

This is a runtime observation source, not a completed observability product.

The existing Fly telemetry seam starts after structural inspection and request validation for these operations. Consequently:

- Preview p95 covers the telemetry-visible renderer call;
- Publish p95 covers the telemetry-visible project save call;
- runtime error rate covers terminal runtime failures visible on those two calls;
- sanitize failure rate can only include sanitize failures that reach the telemetry-visible Publish terminal path;
- pre-telemetry validation, inspection and release-gate failures are not folded into these rates.

Those boundaries are retained explicitly in machine evidence so a later aggregation slice cannot silently reinterpret this process-local sample as end-to-end deployment health.

## Deliberately unpromoted Pages health

Pages remains `unobserved` by design.

This slice does **not** connect `provider_health_runtime_snapshot()` to Pages GraphQL or admin status. `provider_health_observed = false` remains the authoritative Pages rollout response, and `PageBuilderAdminProviderStatus::unobserved(...)` remains the admin facade state.

A process-local, restartable window is insufficient authority for a tenant/deployment health decision. Before Pages can expose observed provider health, the next source slice must define a deployment-wide aggregation/freshness boundary with exact source/deployment identity and fail-closed stale/missing observation behavior.

The existing Pages reference-consumer gate therefore remains unaccepted, Forum Wave remains blocked by that gate, and FFA/FBA promotion remains unclaimed.

## Source evidence

Production source:

- `crates/rustok-page-builder/src/health.rs`;
- `crates/rustok-page-builder/src/runtime_telemetry.rs`;
- `crates/rustok-page-builder/src/composition.rs`;
- `crates/rustok-page-builder/src/adapters/fly_service.rs`.

Machine evidence:

- `crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-runtime-observation-source.json`.

Fail-closed source guard:

- `crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-runtime-observation.mjs`.

The guard also source-locks the non-promotion boundary: Pages GraphQL must still contain `provider_health_observed: false`, and Pages admin must still use `PageBuilderAdminProviderStatus::unobserved` until deployment observation authority exists.

## Next cursor

The provider-health source sequence is now:

```text
bounded process-local Preview/Publish observation
-> deployment aggregation + freshness contract
-> exact source/deployment identity binding
-> Pages provider-status transport/binding
-> retained deployment/runtime evidence
-> observed-health acceptance decision
```

The first item is source-ready in this slice. The remaining items stay open.

## Validation boundary

Per maintainer instruction, tests were not run. No Cargo commands, Node verifiers, formatting, builds, GraphQL/HTTP requests, browser runs, workflows, CI, runtime scenarios or production observations were executed.

Suggested maintainer commands, intentionally not run:

```bash
cargo test -p rustok-page-builder health
cargo test -p rustok-page-builder runtime_telemetry
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-runtime-observation.mjs
cargo check -p rustok-page-builder --all-targets
```
