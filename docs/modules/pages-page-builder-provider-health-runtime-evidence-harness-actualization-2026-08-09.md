# Pages / Page Builder provider-health runtime evidence harness actualization — 2026-08-09

Status: `provider-health-runtime-evidence-harness-source-ready / exact-evidence-chain-source-ready / non-mutating-runtime-observation-source-ready / maintainer-execution-pending / observed-health-owner-acceptance-pending`.

## Purpose

The provider-health source path is now wired through deployment metrics/identity/evaluation, owner acceptance, Pages server binding, workspace/SSR/browser-intent consumers and the non-mutating GraphQL capability preflight. The remaining gap was a bounded runtime packet that can prove those source-ready layers refer to the same live deployment and are actually observed by Pages consumers.

This slice prepares that harness only. It does not execute the deployment identity capture, Prometheus evaluator, owner acceptance, packet installation, GraphQL/HTTP requests or browser run.

## Exact predecessor chain

The harness requires three already-produced maintainer inputs:

```text
page_builder_provider_health_deployment_identity_v1
-> page_builder_provider_health_deployment_evaluation_v1
-> pages_builder_provider_health_owner_acceptance_v1 (accepted)
```

It fails closed unless:

- all three source commits equal checkout `HEAD`;
- deployment id and immutable RepoDigest match across the chain;
- evaluator `identity_captured_at` equals the supplied identity packet `captured_at`;
- owner acceptance `evaluation_sha256` equals the exact supplied evaluator packet;
- accepted evaluator snapshot and SLO evaluation equal the supplied evaluator packet;
- binding is authorized, still records `server_binding_performed=false`, and retains the exact source/digest rollback contract;
- `health_valid_until` is canonical and has not expired before execution begins.

The harness never discovers or installs a provider-health packet. The operator must deploy the exact source/deployment and configure Pages server binding to the accepted packet before executing the harness.

## Rollout isolation

Runtime provider-health evidence requires configured rollout `all_on`.

The harness reads that state from `pageBuilderRolloutSnapshot`; it does not call `updateModuleSettings` and does not restore or mutate rollout settings. This makes any capability narrowing attributable to the observed provider status rather than an overlapping rollout profile.

## Observed surfaces

The runtime packet observes one live accepted health state (`ready`, `degraded` or `unavailable`) across:

1. `pageBuilderRolloutSnapshot` with `providerHealthObserved=true` and a typed health payload matching owner acceptance;
2. non-mutating `pageBuilderCapabilityPreflight` for Preview / Properties / Publish;
3. workspace provider state and capability controls via canonical `data-fly-provider-*` / capability attributes;
4. authoritative SSR Preview when the UI permits Preview;
5. standalone browser-intent capability denial when health narrows Publish or Properties.

The expected all-on behavior is:

```text
Ready       -> Preview allowed, Properties allowed, Publish allowed
Degraded    -> Preview allowed, Properties allowed, Publish FEATURE_DISABLED
Unavailable -> Preview FEATURE_DISABLED, Properties FEATURE_DISABLED, Publish FEATURE_DISABLED
```

The GraphQL capability contract remains canonical `feature-disabled / FEATURE_DISABLED`. Standalone browser intent remains the separate `FLY_CAPABILITY_DENIED` security/preflight contract.

## Non-mutating browser-intent probe

A normal expected-denial `save` probe is unsafe: if the accepted health lease expires or is revoked between observations, Publish could become allowed and the request might persist a document.

The harness therefore sends its capability-required browser intent with an intentionally mismatched envelope page id. Pages browser-intent code runs `validate_browser_capability_access` before dispatch:

```text
health still narrows capability -> FLY_CAPABILITY_DENIED before dispatch
health unexpectedly revoked/expired -> capability may pass, then PageMismatch stops dispatch before document mutation
```

For `Degraded`, the probe is `save` / Publish. For `Unavailable`, it additionally probes `rename_page` / Properties. This preserves observed browser-intent coverage without a mutating fallback.

## SSR safety

Preview is non-mutating. Under Ready/Degraded the harness clicks the canonical server-preview control and retains only response status, byte length and SHA-256. Under Unavailable it verifies the preview control is disabled and does not manufacture a server-function request body merely to force a denial.

No Publish request is ever executed by this harness.

## Retained packet

Successful maintainer execution writes:

```text
target/pages-builder-provider-health-runtime-evidence.json
```

with format/status:

```text
pages_builder_provider_health_runtime_evidence_v1
observed_runtime_evidence_owner_review_pending
```

It retains exact source/deployment identity, hashes of the three predecessor packets and required source files, accepted health/SLO facts, hashed origins/tenant/page identities and bounded status/body hashes plus capability outcomes.

It does not retain tenant/page ids, credentials, storage state contents, raw GraphQL/server-function bodies, raw evidence paths, screenshots, videos or traces.

## Acceptance boundary

Producing a runtime packet does **not** accept provider health. The next step remains an explicit owner review/acceptance decision over the retained runtime packet.

This harness also does not alter:

- `pages_reference_consumer_gate accepted=false`;
- Forum Wave pending on the Pages gate;
- FFA/FBA non-promotion.

## Source evidence

```text
crates/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-execution-contract.json
crates/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-harness-source.json
apps/next-admin/playwright.pages-builder-provider-health-runtime.config.ts
apps/next-admin/tests/pages-builder-provider-health-runtime/runtime.spec.ts
crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-runtime-harness.mjs
```

## Validation boundary

No tests, Node verifiers, Cargo commands, formatting, builds, GraphQL/HTTP requests, Playwright/browser runs, identity captures, Prometheus queries, evaluator executions, owner acceptance executions, packet installations, workflows or CI were run by this slice.

Suggested maintainer source check, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-runtime-harness.mjs
```

Suggested runtime command after the exact evidence chain and binding are prepared, intentionally not run:

```bash
cd apps/next-admin
npx --no-install playwright test --config playwright.pages-builder-provider-health-runtime.config.ts
```
