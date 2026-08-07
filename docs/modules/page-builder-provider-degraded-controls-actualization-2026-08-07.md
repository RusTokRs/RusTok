# Page Builder Provider Degraded Controls Actualization

Date: 2026-08-07  
Status: `current-source-overlay / admin-provider-status-source-ready / degraded-control-source-ready / observed-health-execution-open`

## Why this slice exists

The Page Builder backend already owns typed rollout and health contracts:

- `BuilderCapabilityFlags` and the `AllOn`, `PublishOff`, `PreviewOff`, `BuilderOff` fallback profiles;
- `ProviderHealthState::{Ready, Degraded, Unavailable}`;
- declared degradation reasons and SLO evaluation.

The admin surface already had an `EditorCapabilityPolicy` capable of turning a degraded provider into publish-disabled authoring and an unavailable provider into read-only authoring. The missing boundary was the connection between those provider contracts and the canonical admin facade/UI.

Before this slice, `PageBuilderAdminFacade` could expose capability requests and consumer properties only. `CapabilityPolicyPanel` also displayed `healthy` when no detailed evaluation was supplied, which could be mistaken for observed provider health even though no provider-health snapshot existed.

## Source-ready contract

`rustok-page-builder-admin` now owns `PageBuilderAdminProviderStatus`.

It contains:

```text
BuilderCapabilityFlags
+ Option<ProviderHealthSnapshot>
```

The optional health snapshot is intentional. `None` means **unobserved**; it never means healthy.

The canonical admin facade has an optional `provider_status()` seam. A consumer facade that supplies the status must report the same rollout flags used by its server capability composition.

Provider status may only narrow the already evaluated host tenant/RBAC editor capabilities:

- invalid rollout flags -> unavailable -> read-only;
- `builder_enabled=false` -> read-only;
- observed `Unavailable` -> read-only;
- observed `Degraded` -> publish disabled while draft editing may remain available;
- `publish_enabled=false` -> publish disabled;
- `properties_enabled=false` -> properties capability disabled;
- `preview_enabled=false` -> server-preview control disabled.

No provider status can grant a capability denied by tenant or permission policy.

## Pages reference consumer

`PagesBuilderFacade` now has one source function for the provider rollout flags used by both sides of the seam:

```text
pages_builder_capability_flags()
```

The facade exposes those flags through `provider_status()`, and the SSR capability composition passes the same flags to `compose_fly_page_builder_handlers`.

Pages does not currently have a live SLO snapshot source, so its health remains explicitly `unobserved`. The existing Pages host capability policy derived from verified role and contribution-assembly diagnostics remains separate and continues to be intersected before provider-status narrowing.

## Admin UX and control path

The capability panel now distinguishes:

1. provider control state — `ready`, `degraded`, `unavailable`, or `unobserved`;
2. observed provider health — or `unobserved` when no live snapshot exists;
3. host provider policy — the existing tenant/RBAC/contribution evaluation;
4. concrete rollout flags;
5. declared observed degradation reasons when present.

Effective capability rows continue to read from the state machine after host policy **and** provider-status narrowing.

Server preview is not only visually disabled. The click path rechecks the same provider status before dispatching a preview request, preserving the provider rollout boundary even if the button callback is invoked programmatically.

No fallback editor is mounted for unavailable state.

## Deliberately unchanged boundaries

This slice does not add or change:

- database schema or migrations;
- GraphQL, HTTP or OpenAPI APIs;
- Pages persistence, reviewed publish, rollback or repair contracts;
- public storefront rendering or cache policy;
- a live metrics/SLO collection pipeline;
- tenant rollout persistence;
- a second editor or degraded fallback editor.

Observed health remains a future composition/runtime evidence source. The new admin seam prevents that missing observation from being mislabeled as healthy in the meantime.

## Machine evidence

```text
crates/rustok-page-builder/contracts/evidence/page-builder-admin-provider-status-source.json
crates/rustok-page-builder/scripts/verify/verify-page-builder-admin-provider-status.mjs
```

## Validation boundary

Execution remains pending. No Rust tests, Node verifiers, Cargo checks, formatting, browser scenarios, workflows or CI were run by this implementation slice. FFA/FBA promotion remains blocked on accepted execution and observed-provider evidence.
