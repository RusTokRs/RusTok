# Pages / Page Builder provider-health consumer binding actualization — 2026-08-09

Status: `consumer-provider-health-binding-source-ready / workspace-observed-health-source-ready / authoritative-ssr-health-guard-source-ready / browser-intent-health-preflight-source-ready / runtime-activation-pending / observed-health-acceptance-pending`.

## Scope

This continuation closes the source-only consumer gap after typed provider-health transport, owner acceptance and the fail-closed Pages server binding became source-ready.

The server-owned `pageBuilderRolloutSnapshot` is now the single transport source for both configured rollout flags and optional validated provider health. Pages consumers no longer discard the health half of that snapshot. The same canonical `PageBuilderAdminProviderStatus` now drives the workspace provider controls, authoritative Preview/Publish runtime guards and standalone browser-intent capability preflight.

This remains source readiness only. Pages remains `unobserved` without a live accepted packet. No deployment identity capture, deployment evaluator, owner acceptance, accepted packet installation or observed consumer runtime behavior was executed in this slice.

## Canonical narrowing contract

`PageBuilderAdminProviderStatus` already owned the Page Builder degraded-control policy used by the admin UI. This slice adds `effective_runtime_flags()` so the server runtime uses exactly the same provider state rather than reimplementing health rules in Pages.

Configured rollout flags remain authoritative. Provider health can only narrow them:

```text
Unobserved -> configured rollout flags unchanged
Ready      -> configured rollout flags unchanged
Degraded   -> configured rollout flags, publish=false
Unavailable-> builder=false, preview=false, properties=false, publish=false
```

Invalid configured rollout flags already resolve to `Unavailable`, so runtime composition also fails closed to builder-off. Health cannot enable Preview, Properties or Publish if rollout or tenant/RBAC policy denied them.

The existing provider policy remains intentionally conservative: observed `Degraded` disables Publish while draft editing may remain available; observed `Unavailable` forces read-only/unavailable behavior. This is the same policy already exposed by the canonical Page Builder admin degraded controls.

## Workspace binding

The Pages workspace previously fetched the server-owned rollout snapshot and immediately discarded everything except `.flags`. It now retains the validated provider status:

```text
fetch_pages_builder_rollout_snapshot(...)
-> PagesBuilderRolloutSnapshot::provider_status()
-> PagesBuilderFacade::with_provider_status(...)
-> PageBuilderAdminFacade::provider_status()
-> canonical Page Builder admin capability/provider controls
```

When no observed health is transported, `provider_status()` still constructs the same explicit `Unobserved` state over the configured flags. Therefore the ordinary no-packet path is unchanged and never becomes implicitly healthy.

When a fresh accepted server authority supplies health, the Page Builder admin UI can display the observed state and declared degradation reasons and can apply its existing degraded/read-only controls from the same validated snapshot.

## Authoritative SSR binding

The Preview/Publish server-function path already reread `fetch_pages_builder_rollout_snapshot` for each capability request and verified the routed tenant. It formerly passed `trusted_rollout.flags` directly into `compose_fly_page_builder_handlers`.

It now derives:

```text
trusted_rollout
-> provider_status()
-> effective_runtime_flags()
-> compose_fly_page_builder_handlers(...)
-> canonical CapabilityGuardedService
```

Consequences:

- missing health preserves configured rollout behavior;
- `Ready` preserves configured rollout behavior;
- observed `Degraded` forces `publish_enabled=false`, so a Publish request reaches the existing canonical Page Builder capability guard and returns the established `feature-disabled / FEATURE_DISABLED` contract rather than a Pages-specific health error;
- observed `Unavailable` forces the entire builder off, so Preview and Publish are denied by the same canonical capability guard;
- the server still re-fetches the snapshot per request, so stale UI state cannot authorize a capability after the server authority has expired or been revoked.

No health state bypasses tenant or role authorization.

## Standalone browser-intent binding

The standalone admin host continues to verify the authenticated user and evaluate role capabilities first. It now applies:

```text
pages_editor_capabilities_for_snapshot(role_capabilities, &rollout)
```

instead of the rollout-only helper.

The helper delegates to `snapshot.provider_status().limit_capabilities(...)`, so provider health can only narrow the already evaluated role capability set. Degraded health disables Publish intent. Unavailable health reduces the builder intent profile to read-only. Missing health preserves the prior rollout-only result.

This preflight remains separate from the authoritative SSR re-read. Browser/UI capability state is advisory UX and intent filtering; the server capability composition remains the final authority.

## Runtime evidence boundary

The source is now wired end-to-end:

```text
accepted deployment health authority
-> typed GraphQL provider health
-> fail-closed admin transport revalidation
-> PagesBuilderRolloutSnapshot
-> workspace provider status
-> SSR effective runtime flags
-> standalone browser-intent capability narrowing
```

That does not mean observed health has been activated or accepted in production. Runtime activation remains maintainer-owned.

A real observed-health execution still requires the existing exact chain:

1. live exact-target deployment identity capture;
2. retained deployment evaluator packet for that exact deployment;
3. explicit owner acceptance while the retained health freshness deadline is still valid;
4. installation/configuration of that accepted packet for the live Pages server binding;
5. observed GraphQL transport and consumer behavior evidence proving degraded/unavailable controls against that exact deployment;
6. explicit observed-health acceptance decision.

Removing, rejecting, invalidating or expiring the accepted packet restores the server GraphQL snapshot to unobserved; the three consumers then naturally return to the configured rollout-only behavior on their next server-owned snapshot read.

## Non-promotion

This slice does not accept the Pages reference-consumer gate, does not unblock or accept Forum Wave, and does not claim FFA/FBA promotion. Existing rollout execution evidence remains pending.

No process-local observation or raw Prometheus data is accepted by these consumers. They consume only the already validated optional health transported by Pages GraphQL.

## Next cursor

```text
bounded process-local Preview/Publish observation [source-ready]
-> deployment metrics + freshness [source-ready]
-> exact source/deployment identity + target inventory [source-ready]
-> deployment health evaluator [source-ready]
-> typed observed-health transport [source-ready]
-> owner acceptance packet + retained health deadline [source-ready]
-> server provider-health binding + hot revoke [source-ready]
-> UI / SSR / browser-intent provider-health binding [source-ready]
-> live identity + evaluator + accepted owner packet + observed consumer behavior [maintainer execution pending]
-> observed-health acceptance decision [pending]
```

## Validation boundary

Tests were not run. Node verifiers, Cargo commands, formatting, builds, GraphQL/HTTP requests, deployment identity capture, Prometheus queries, evaluator execution, owner-acceptance execution, accepted-packet installation, browser runs, workflows and CI were intentionally not executed by this implementation slice.

Suggested maintainer source checks, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-consumer-binding.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-server-binding.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-transport.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-binding.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-admin-provider-status.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
```
