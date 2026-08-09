# Pages / Page Builder provider-health transport actualization — 2026-08-09

Status: `typed-observed-health-transport-source-ready / canonical-client-revalidation-source-ready / server-owner-health-binding-blocked / runtime-evidence-pending / pages-health-unobserved`.

## Cursor

This packet continues the provider-health source chain after:

- `page-builder-provider-health-runtime-observation-actualization-2026-08-09.md`;
- `page-builder-provider-health-deployment-metrics-actualization-2026-08-09.md`;
- `page-builder-provider-health-deployment-identity-actualization-2026-08-09.md`;
- `page-builder-provider-health-deployment-evaluator-actualization-2026-08-09.md`.

The predecessor slice made a deployment-bound evaluator source-ready, but Pages still had no typed transport for an owner-accepted `ProviderHealthSnapshot`. The existing GraphQL snapshot exposed only `providerHealthObserved`, and the admin adapter rejected any `true` value.

This slice closes that transport-only source gap. It does **not** execute deployment evidence, accept a health packet, or bind observed health into any production Pages consumer.

## Server-owned GraphQL shape

`pageBuilderRolloutSnapshot` now exposes the existing boolean plus an optional typed health payload:

```text
providerHealthObserved
providerHealth {
  state
  degradationReasons
  previewP95Ms
  publishP95Ms
  sanitizeFailureRate
  runtimeErrorRate
}
```

The payload is derived from the canonical `rustok_page_builder::health::ProviderHealthSnapshot` rather than defining a second health policy.

The **current live query path remains deliberately unobserved**:

```text
provider_health_observed: false
provider_health: None
```

A future server-owned binding may map an owner-accepted retained evaluator packet into this shape. This slice does not provide that authority and does not load evaluator artifacts from GraphQL code.

## Boolean/payload consistency

The stateless admin transport is fail-closed over the two fields:

| `providerHealthObserved` | `providerHealth` | Result |
|---|---|---|
| `false` | absent | accepted as unobserved |
| `false` | present | rejected |
| `true` | absent | rejected |
| `true` | present | validated as an observed candidate |

This prevents a boolean-only healthy claim and prevents a hidden health payload from being consumed while the server still reports unobserved state.

## Canonical client revalidation

Even when a future server binding returns `observed=true`, the admin transport does not trust the state/reason labels independently.

It validates:

- Preview and Publish p95 values are non-negative;
- sanitize/runtime failure rates are finite and inside `[0, 1]`;
- the observation values are fed back through canonical `ProviderHealthSnapshot::evaluate`;
- transported `state` must equal the canonical result;
- transported `degradationReasons` must equal the canonical ordered reason set.

Thresholds are therefore not accepted from the wire. They remain owned by `ProviderSloThresholds::PILOT` through canonical evaluation.

The transport never consumes Prometheus metrics, raw evaluator queries, target mappings or deployment credentials.

## Admin snapshot seam

`PagesBuilderRolloutSnapshot` now retains:

```text
flags: BuilderCapabilityFlags
provider_health: Option<ProviderHealthSnapshot>
```

and exposes a canonical `provider_status()` helper:

- health present -> `PageBuilderAdminProviderStatus::observed(...)`;
- health absent -> `PageBuilderAdminProviderStatus::unobserved(...)`.

A separate `pages_editor_capabilities_for_snapshot(...)` helper is source-ready for the eventual owner binding.

The existing `pages_editor_capabilities_for_rollout(...)` helper remains unchanged and unobserved.

## Deliberately unbound production consumers

This slice does not activate the new helper in current production paths.

The Pages workspace still:

- extracts `.flags` from the server snapshot;
- passes `provider_flags: BuilderCapabilityFlags`;
- composes the facade with `.with_provider_flags(provider_flags)`.

The authoritative SSR facade still resolves missing health through `PageBuilderAdminProviderStatus::unobserved`.

The standalone browser-intent preflight still calls `pages_editor_capabilities_for_rollout(..., &rollout.flags)`.

Therefore transport readiness cannot silently change editing, preview or publish behavior before evidence and acceptance exist.

## Why owner binding remains blocked

The required runtime sequence is still maintainer-owned:

1. execute exact deployment identity capture;
2. retain a deployment evaluator packet over the same admitted deployment;
3. review/accept the resulting source SHA, RepoDigest, target inventory, freshness window and SLO result;
4. only then bind the accepted snapshot into the server-owned Pages rollout query;
5. only after server binding may UI/SSR/browser-intent consumers switch from rollout-only to validated provider status.

Source inspection cannot substitute for steps 1–3.

## Source evidence

Machine contract:

```text
crates/rustok-pages/contracts/evidence/pages-builder-provider-health-transport-source.json
```

Fail-closed source guard:

```text
crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-transport.mjs
```

The guard locks the GraphQL shape, boolean/payload consistency, canonical re-evaluation, optional admin snapshot seam and continued absence of production health binding.

## Next cursor

```text
bounded runtime observation [source-ready]
-> deployment metrics/freshness [source-ready]
-> exact deployment identity + target inventory [source-ready]
-> deployment health evaluator [source-ready]
-> typed observed-health transport [source-ready]
-> retained identity + evaluator runtime evidence [maintainer execution pending]
-> owner acceptance + server health binding [blocked]
-> UI / SSR / browser-intent health binding [blocked]
-> observed-health acceptance [pending]
```

## Validation boundary

Per maintainer instruction, tests were not run. No Node verifier, Cargo command, formatting, build, GraphQL request, Prometheus query, deployment capture/evaluator execution, browser run, workflow or CI was executed by this slice.

Suggested maintainer source checks, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-transport.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-server-snapshot.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-binding.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-evaluator.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
```
