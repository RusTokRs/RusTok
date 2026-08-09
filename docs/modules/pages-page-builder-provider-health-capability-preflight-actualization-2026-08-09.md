# Pages / Page Builder provider-health capability preflight actualization — 2026-08-09

Status: `provider-health-capability-preflight-source-ready / shared-runtime-policy-source-ready / non-mutating-health-preflight-source-ready / rollout-only-preflight-health-boundary-source-ready / runtime-evidence-harness-source-ready / runtime-execution-pending / observed-health-acceptance-pending`.

## Why this continuation exists

PR #3417 closed provider-health source binding for the Pages workspace, authoritative Preview/Publish SSR and standalone browser-intent path. The remaining preflight inconsistency was removed by making `pageBuilderCapabilityPreflight` evaluate the same fresh provider-health authority and shared runtime policy as authoritative SSR.

## Shared Page Builder runtime policy

The health/rollout narrowing rule is owned by:

```text
rustok_page_builder::rollout::effective_provider_runtime_flags
```

It can only narrow configured rollout:

```text
invalid rollout or builder disabled -> BuilderOff
observed Unavailable                -> BuilderOff
observed Degraded                   -> preserve rollout, force publish=false
rollout-degraded state              -> preserve disabled rollout fields, force publish=false
observed Ready                      -> configured rollout unchanged
Unobserved                          -> configured rollout unchanged
```

`PageBuilderAdminProviderStatus::effective_runtime_flags()` delegates to this shared policy, and Pages authoritative SSR consumes the same provider-status path.

## Health-aware non-mutating preflight

`pageBuilderCapabilityPreflight` evaluates routed tenant/module admission, canonical Page Builder permissions, server-owned configured rollout, fresh optional `PagesGraphqlRuntimeData` health, shared runtime narrowing and canonical `ensure_capability` in that order.

The operation remains non-mutating and returns the canonical provider/rollout denial:

```text
errorKind = feature-disabled
errorCode = FEATURE_DISABLED
```

Permission denial remains separate as `FORBIDDEN`.

With configured all-on rollout:

- `Ready`: Preview / Properties / Publish allowed;
- `Degraded`: Preview / Properties allowed, Publish `FEATURE_DISABLED`;
- `Unavailable`: Preview / Properties / Publish `FEATURE_DISABLED`.

The rollout snapshot continues to expose configured flags separately from optional health rather than rewriting them to effective flags.

## Existing rollout feature-preflight harness

The four-profile rollout harness remains rollout-only. It observes `pageBuilderRolloutSnapshot` before and after each profile and fails closed unless `providerHealthObserved=false` with no health payload. It retains only status/body hashes and explicit unobserved booleans, not the provider-health payload.

## Runtime evidence continuation

The previously open observed-health runtime harness is now source-ready:

```text
docs/modules/pages-page-builder-provider-health-runtime-evidence-harness-actualization-2026-08-09.md
```

It can bind exact deployment identity + retained evaluator + accepted owner packet to observed GraphQL health/preflight, workspace provider controls, safe authoritative SSR Preview and standalone browser-intent outcomes into one `observed_runtime_evidence_owner_review_pending` packet.

The harness requires configured all-on rollout and does not mutate rollout settings or execute Publish. Browser-intent denial uses a deliberately mismatched envelope page id so an unexpected health expiry/revoke falls through to PageMismatch before document mutation.

Runtime execution remains maintainer-owned.

## Non-promotion

No identity/evaluator/owner-acceptance execution, accepted packet installation, observed runtime behavior, Pages reference-consumer gate acceptance, Forum Wave acceptance or FFA/FBA promotion is claimed by this source work. Pages remains `unobserved` in retained execution evidence until the maintainer executes and retains the exact chain.

## Source evidence

```text
crates/rustok-pages/contracts/evidence/pages-builder-provider-health-capability-preflight-source.json
crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-capability-preflight.mjs
crates/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-harness-source.json
```

## Validation boundary

Tests were not run. Node verifiers, Cargo commands, formatting, builds, GraphQL/HTTP requests, Playwright/browser runs, deployment identity capture, Prometheus queries, evaluator execution, owner acceptance, workflows and CI were intentionally not executed.

Suggested maintainer source checks, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-capability-preflight.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-runtime-harness.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
```
