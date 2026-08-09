# Pages / Page Builder provider-health capability preflight actualization — 2026-08-09

Status: `provider-health-capability-preflight-source-ready / shared-runtime-policy-source-ready / non-mutating-health-preflight-source-ready / rollout-only-preflight-health-boundary-source-ready / runtime-execution-pending / observed-health-acceptance-pending`.

## Why this continuation exists

PR #3417 closed provider-health source binding for the Pages workspace, authoritative Preview/Publish SSR and standalone browser-intent path. One source inconsistency remained: the non-mutating GraphQL `pageBuilderCapabilityPreflight` still evaluated only configured rollout flags.

That could make evidence disagree with the real server guard after a fresh accepted provider-health packet was installed. With all rollout flags enabled and observed `Degraded` health, authoritative SSR correctly disables Publish, while the old preflight could still report Publish as allowed.

This slice removes that drift without executing any runtime evidence.

## Shared Page Builder runtime policy

The health/rollout narrowing rule now lives in Page Builder core as:

```text
rustok_page_builder::rollout::effective_provider_runtime_flags
```

The function takes configured `BuilderCapabilityFlags` plus optional `ProviderHealthSnapshot` and can only narrow configured capabilities:

```text
invalid rollout or builder disabled -> BuilderOff
observed Unavailable                -> BuilderOff
observed Degraded                   -> preserve rollout, force publish=false
rollout-degraded state              -> preserve disabled rollout fields, force publish=false
observed Ready                      -> configured rollout unchanged
Unobserved                          -> configured rollout unchanged
```

This preserves the policy already used by the Page Builder admin provider status, including partial rollout states such as Properties disabled while Preview remains available. Health cannot re-enable a rollout-disabled capability.

`PageBuilderAdminProviderStatus::effective_runtime_flags()` remains the stable admin-facing seam but now delegates directly to this shared core policy. Pages authoritative SSR continues to call the snapshot/admin seam, so existing consumer code does not gain a second policy implementation.

## Health-aware non-mutating preflight

`pageBuilderCapabilityPreflight` now evaluates in this order:

1. Pages module and routed tenant admission;
2. canonical Page Builder permission mapping;
3. server-owned configured rollout read;
4. fresh optional provider health from `PagesGraphqlRuntimeData`;
5. shared `effective_provider_runtime_flags` narrowing;
6. canonical `ensure_capability` result.

The preflight remains non-mutating. It does not render Preview, save Publish data, read the retained acceptance packet directly or inspect provider-health environment configuration.

Disabled capabilities still return the existing Page Builder contract:

```text
errorKind = feature-disabled
errorCode = FEATURE_DISABLED
```

Permission denial remains a separate `FORBIDDEN` boundary and is checked before a capability result is returned.

## Expected observed behavior

With all configured rollout flags enabled:

- `Ready`: Preview, Properties and Publish remain allowed;
- `Degraded`: Preview and Properties remain allowed, Publish is `FEATURE_DISABLED`;
- `Unavailable`: Preview, Properties and Publish are `FEATURE_DISABLED`.

With rollout restrictions already present, health may only keep or further narrow those restrictions.

The rollout snapshot continues to expose the configured flags and provider health separately. It does **not** replace its configured flag fields with health-limited effective flags. This keeps the transport auditable and lets clients independently validate the same provider status.

## Existing rollout feature-preflight harness

The existing four-profile rollout feature-preflight harness remains a rollout-only acceptance input and continues to claim provider health as `unobserved`.

Because the production GraphQL preflight is now health-aware, that claim is no longer accepted as a static assumption. The harness now reads `pageBuilderRolloutSnapshot` immediately before and after each profile capability preflight and fails closed unless `providerHealthObserved=false` and the health payload is absent on both observations. The retained profile record contains only HTTP status, bounded response-body size/hash and explicit unobserved/payload-absent booleans; the provider-health payload itself is not retained.

This prevents observed `Ready` health from being silently indistinguishable from rollout-only all-on behavior in a packet that claims provider health was unobserved. Observed-health execution still belongs to a separate provider-health evidence harness. The existing rollout matrix/preflight packets must not be reinterpreted as observed SLO evidence.

## Runtime evidence boundary

This source slice does not install or execute a provider-health acceptance packet. It does not call GraphQL, HTTP, browser, Prometheus or the deployment evaluator.

The next source-only cursor is a bounded observed-health runtime evidence harness that can bind:

```text
exact deployment identity
+ retained deployment evaluator
+ accepted owner packet
+ observed GraphQL health/preflight
+ workspace provider controls
+ authoritative SSR outcome
+ standalone browser-intent outcome
```

into one owner-review-pending packet without accepting the result automatically.

Runtime execution remains maintainer-owned.

## Non-promotion

This slice does not claim:

- deployment identity capture execution;
- deployment evaluator execution;
- owner acceptance execution;
- accepted packet installation;
- observed GraphQL/preflight behavior;
- observed workspace/SSR/browser-intent behavior;
- Pages reference-consumer gate acceptance;
- Forum Wave acceptance;
- FFA/FBA promotion.

Pages remains `unobserved` in retained execution evidence until the exact live chain is executed and retained.

## Source evidence

```text
crates/rustok-pages/contracts/evidence/pages-builder-provider-health-capability-preflight-source.json
crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-capability-preflight.mjs
```

## Validation boundary

Tests were not run. Node verifiers, Cargo commands, formatting, builds, GraphQL/HTTP requests, Playwright/browser runs, deployment identity capture, Prometheus queries, evaluator execution, owner acceptance, workflows and CI were intentionally not executed.

Suggested maintainer source checks, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-capability-preflight.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-consumer-binding.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-admin-provider-status.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-feature-preflight-harness.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
```
