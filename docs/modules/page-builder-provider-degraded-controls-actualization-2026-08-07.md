# Page Builder Provider Degraded Controls Actualization

Date: 2026-08-07  
Status: `current-source-overlay / admin-provider-status-source-ready / degraded-control-source-ready / process-local-runtime-observation-source-ready / deployment-observed-health-open`.

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

The Pages reference consumer exposes the same server-owned rollout flags through its provider status and authoritative Page Builder composition.

Pages admin health remains explicitly `unobserved`. The existing Pages host capability policy derived from verified role and contribution-assembly diagnostics remains separate and continues to be intersected before provider-status narrowing.

## Admin UX and control path

The capability panel now distinguishes:

1. provider control state — `ready`, `degraded`, `unavailable`, or `unobserved`;
2. observed provider health — or `unobserved` when no authoritative live snapshot exists;
3. host provider policy — the existing tenant/RBAC/contribution evaluation;
4. concrete rollout flags;
5. declared observed degradation reasons when present.

Effective capability rows continue to read from the state machine after host policy **and** provider-status narrowing.

Server preview is not only visually disabled. The click path rechecks the same provider status before dispatching a preview request, preserving the provider rollout boundary even if the button callback is invoked programmatically.

No fallback editor is mounted for unavailable state.

## 2026-08-09 runtime observation continuation

`docs/modules/page-builder-provider-health-runtime-observation-actualization-2026-08-09.md` now closes the first source-only part of the former live-observation gap.

Default Fly composition records bounded process-local terminal observations for canonical Preview rendering and Publish project-save calls through the existing Page Builder runtime-telemetry seam. The local window is capped, requires a minimum Preview/Publish sample floor, and evaluates only through the existing pilot `ProviderHealthSnapshot` thresholds.

This does **not** make Pages health observed. The local window is restartable, lacks deployment-wide aggregation/freshness and exact source/deployment identity, and does not cover validation/inspection that occurs before the existing telemetry seam. Pages therefore continues to expose `provider_health_observed = false` and to construct `PageBuilderAdminProviderStatus::unobserved(...)`.

The next provider-health source cursor is deployment aggregation/freshness plus exact deployment identity. Pages provider-status binding remains blocked on that authority.

## Deliberately unchanged boundaries

These slices do not add or change:

- database schema or migrations;
- GraphQL, HTTP or OpenAPI APIs;
- Pages persistence, reviewed publish, rollback or repair contracts;
- public storefront rendering or cache policy;
- tenant rollout persistence;
- a second editor or degraded fallback editor;
- Pages reference-consumer gate acceptance;
- Forum Wave acceptance;
- FFA/FBA promotion.

Observed deployment health remains open. The admin seam and new process-local observer prevent missing deployment evidence from being mislabeled as healthy in the meantime.

## Machine evidence

Admin degraded-control source:

```text
crates/rustok-page-builder/contracts/evidence/page-builder-admin-provider-status-source.json
crates/rustok-page-builder/scripts/verify/verify-page-builder-admin-provider-status.mjs
```

Process-local runtime observation source:

```text
crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-runtime-observation-source.json
crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-runtime-observation.mjs
```

## Validation boundary

Execution remains pending. No Rust tests, Node verifiers, Cargo checks, formatting, browser scenarios, workflows, CI or runtime evidence were run by this implementation slice. FFA/FBA promotion remains blocked on accepted execution and authoritative observed-provider evidence.
