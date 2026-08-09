# Page Builder provider-health deployment identity actualization — 2026-08-09

Status: `deployment-identity-contract-source-ready / expected-target-inventory-contract-source-ready / live-identity-capture-execution-pending / deployment-health-evaluator-open / pages-binding-blocked`.

## Why this slice exists

The previous provider-health slices made two pieces source-ready:

1. bounded process-local Preview/Publish observations;
2. deployment-aggregatable Prometheus duration, outcome and freshness series.

Those metrics still could not be authoritative for Pages because a backend series without an exact deployment boundary can silently mix old/new targets, omit a replica, or aggregate an unexpected target. The missing source contract was therefore identity and completeness, not another SLO formula.

This slice defines that boundary without claiming runtime execution.

## Reused release identity

RusToK already has a canonical release source-identity path:

```text
GitHub release commit (`github.sha`)
-> Docker build arg `OCI_REVISION`
-> OCI label `org.opencontainers.image.revision`
-> runtime env `RUSTOK_SOURCE_COMMIT`
```

The same runtime environment is already consumed by the Forum Page Builder deployment-attestation flow. Provider health now exposes it through one fixed-cardinality build-info series:

```text
rustok_page_builder_provider_build_info{source_commit="<40-char-git-sha>"} 1
```

The series is emitted only when `RUSTOK_SOURCE_COMMIT` is a canonical Git SHA. Missing or malformed identity leaves the build-info series absent; the deployment capture harness therefore fails closed.

No tenant, page, revision, correlation, target, host or deployment id is added as an application metric label.

## Immutable deployment digest boundary

The pushed image RepoDigest does not exist until after the image is published, so the running process cannot derive or cryptographically prove its own RepoDigest from `RUSTOK_SOURCE_COMMIT` alone.

The deployment identity packet therefore follows the repository's existing Forum attestation boundary:

- every expected live target must report the exact checkout source commit;
- the immutable deployment image digest is supplied explicitly as `REPOSITORY@sha256:<digest>`;
- the origin/target-to-RepoDigest association is recorded as a `maintainer_reviewed_external_fact`;
- a cryptographic origin-to-RepoDigest binding is **not** claimed.

This keeps the source/deployment statement exact without inventing provenance the runtime does not possess.

## Expected-target inventory contract

`capture-page-builder-provider-health-deployment-identity.mjs` requires a deployment-specific inventory file with this shape:

```json
{
  "schema_version": 1,
  "deployment_id": "prod-eu-wave-1",
  "inventory_complete": true,
  "targets": [
    {
      "target_id": "server-a",
      "metrics_url": "https://example.invalid/metrics/"
    }
  ]
}
```

The example is structural only; no production inventory is committed by this slice.

The harness rejects:

- `inventory_complete != true`;
- zero targets or more than 64 targets;
- duplicate target ids;
- duplicate metrics URLs;
- unbounded/unsupported target or deployment identifiers;
- metrics URLs with embedded credentials, query strings or fragments;
- redirects;
- any expected target that does not answer `200`;
- any expected target missing the build-info series;
- more than one build-info series on a target;
- any source commit that differs from the exact checkout `HEAD`.

Partial success is forbidden: every expected target must verify before an identity packet is written.

## Retained identity packet

The default output is:

```text
target/page-builder-provider-health-deployment-identity.json
```

It retains:

- exact checkout source commit;
- maintainer-supplied deployment id;
- maintainer-supplied immutable deployment image RepoDigest;
- expected and verified target counts;
- target ids;
- SHA-256/byte length of each metrics URL, but not the raw URL;
- SHA-256/byte length of each metrics response, but not the raw body;
- exact per-target source-commit verification;
- hashes of the source files that define the identity contract;
- credential environment variable names only.

Authorization, cookies, common-header values, raw metrics URLs and raw metrics bodies are not retained.

## Deliberately unpromoted provider health

This slice does **not** evaluate SLO health.

The deployment identity packet is an admission input for the next evaluator slice. It does not by itself calculate Preview/Publish p95, failure rates, freshness acceptance or deployment health.

Therefore:

- Pages remains `unobserved`;
- `provider_health_observed = false` remains authoritative;
- Pages admin remains `PageBuilderAdminProviderStatus::unobserved(...)`;
- the Pages reference-consumer gate remains unaccepted;
- Forum Wave remains blocked on the Pages gate;
- FFA/FBA promotion remains unclaimed.

## Source evidence

Runtime identity metric:

- `crates/rustok-telemetry/src/page_builder_provider_metrics.rs`.

Canonical release identity chain:

- `apps/server/Dockerfile.release`;
- `.github/workflows/release.yml`.

Machine contract:

- `crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-identity-source.json`.

Capture harness:

- `scripts/evidence/capture-page-builder-provider-health-deployment-identity.mjs`.

Fail-closed source guard:

- `crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-identity.mjs`.

## Next cursor

Provider-health source sequence is now:

```text
bounded process-local Preview/Publish observation [source-ready]
-> deployment metrics + freshness signal [source-ready]
-> source/deployment identity + expected-target inventory contract [source-ready]
-> live exact-target identity capture [maintainer execution pending]
-> deployment health backend evaluator [open]
-> Pages provider-status transport/binding [blocked]
-> retained deployment/runtime evidence [maintainer execution]
-> observed-health acceptance decision [pending]
```

The next autonomous source slice is the deployment health evaluator: it must consume only identity-admitted expected targets, apply an explicit freshness window, use reset-aware backend aggregation, reject missing/stale/mismatched targets, and emit a deployment-bound `ProviderHealthSnapshot` without binding Pages until runtime evidence exists.

## Validation boundary

Per maintainer instruction, tests were not run. No Cargo commands, Node verifiers, formatting, builds, metrics scrapes, Prometheus backend queries, GraphQL/HTTP requests, browser runs, workflows, CI, deployment capture or production observations were executed.

Suggested maintainer commands, intentionally not run:

```bash
cargo test -p rustok-telemetry page_builder_provider
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-identity.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-metrics.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
```
