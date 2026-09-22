# Pages / Page Builder rollout plan actualization — 2026-08-08

Status: `source-parity-current / rollout-owner-source-ready / four-profile-matrix-source-ready / canonical-feature-preflight-source-ready / reference-candidate-source-ready / execution-acceptance-pending`.

This packet is the current rollout/evidence overlay for the larger Pages/Page Builder plans. Where older 2026-08-08 plan text still describes a hardcoded `pages_builder_capability_flags()` helper, rollout wiring as pending, the matrix as merely executable, or the reference candidate as accepting only artifact/browser evidence, this packet supersedes that rollout-specific wording.

## Rechecked merged source

The current source sequence is:

- PR #3333 — Pages UI provider status and authoritative Preview/Publish SSR composition consume a persisted tenant rollout snapshot instead of hardcoded all-on flags;
- PR #3337 — rollout settings authority is owned by Pages GraphQL, standalone admin remains stateless, and direct `/builder/intents` requests are narrowed by the same server-owned persisted flags before draft mutation;
- PR #3345 — the four canonical profiles (`all_on`, `publish_off`, `preview_off`, `builder_off`) have a bounded exact-source runtime-matrix harness using production `tenantModules` / `updateModuleSettings`, real admin UI, Preview SSR, standalone browser-intent preflight, Pages-owned reads and mandatory settings restoration;
- PR #3353 — Pages exposes a tenant-bound non-mutating canonical capability preflight and the reference candidate requires artifact/HTTP, browser, rollout-matrix and canonical feature-preflight packets as one exact-source chain.

No tests or runtime evidence are claimed by these source statements.

## Current ownership and error contracts

Persisted rollout settings remain owned by the Pages server. Admin consumers never supply rollout flags as request authority.

The two degraded-path error contracts are deliberately distinct:

- standalone browser-intent bypass prevention returns `FLY_CAPABILITY_DENIED`;
- canonical Page Builder provider rollout preflight returns `feature-disabled / FEATURE_DISABLED`.

`FLY_CAPABILITY_DENIED` is not accepted as evidence for the provider `FEATURE_DISABLED` catalog.

The canonical non-mutating capability preflight uses the same Pages permission mapping source-locked against `PageBuilderCapabilityPermissions`:

```text
Preview / Tree -> pages:read
Properties     -> pages:update
Publish        -> pages:publish
```

It evaluates `rustok_page_builder::rollout::ensure_capability` after RBAC and before any Preview renderer or Publish persistence path.

## Four-profile execution chain

The accepted execution order is now source-defined as:

```text
artifact/HTTP
-> browser
-> rollout runtime matrix
-> canonical rollout feature preflight
-> reference-consumer candidate
-> maintainer owner sign-off + rollback decision
```

The runtime matrix proves real UI/SSR/read/bypass behavior and restores original tenant settings. The feature-preflight packet separately proves the canonical provider error catalog and performs its own settings snapshot/restore cycle. The reference candidate independently rechecks both packets and their predecessor hashes, source commit, API deployment RepoDigest and origin bindings before executing its bounded source guards/focused tests.

## Canonical profile outcomes

| Profile | Preview | Properties | Publish dry | Pages reads |
| --- | --- | --- | --- | --- |
| `all_on` | pass | pass | pass | pass |
| `publish_off` | pass | pass | `feature-disabled / FEATURE_DISABLED` | pass |
| `preview_off` | `feature-disabled / FEATURE_DISABLED` | pass | `feature-disabled / FEATURE_DISABLED` | pass |
| `builder_off` | `feature-disabled / FEATURE_DISABLED` | read-only/hidden plus browser bypass denial | `feature-disabled / FEATURE_DISABLED` | pass |

The standalone browser-intent matrix additionally retains typed `FLY_CAPABILITY_DENIED` evidence for disabled mutating intents.

## Current source boundary

Source architecture for this Pages reference-consumer rollout slice is complete. The repository now has source for:

- server-owned persisted rollout state;
- UI and authoritative SSR binding;
- standalone browser-intent narrowing;
- four-profile real-consumer runtime matrix;
- canonical non-mutating `FEATURE_DISABLED` preflight;
- exact predecessor/source/deployment correlation into the reference candidate;
- fail-closed source guards and bounded retained evidence formats.

Provider health remains `unobserved`; no live SLO source exists in this slice.

`pages_reference_consumer_gate` remains `accepted = false` and `execution_gate = pending`.

Forum Wave remains blocked by `pages_reference_consumer_gate`.

FFA/FBA promotion remains unclaimed.

## Next cursor

There is no additional source-only rollout architecture task identified by this reconciliation. The next required work is maintainer execution on one exact source/deployment chain:

1. produce accepted artifact/HTTP evidence;
2. produce accepted browser evidence;
3. execute and retain the four-profile rollout matrix;
4. execute and retain the canonical feature-preflight packet;
5. execute the reference-candidate runner;
6. review the candidate and record owner sign-off plus explicit rollback decision;
7. only then change gate acceptance and proceed to Forum browser/runtime/deployment evidence and observed tenant Wave.

The implementation agent must not mark any of those execution steps complete from source inspection alone.

## Validation boundary

No tests, Node verifiers, Cargo commands, formatting, GraphQL/HTTP requests, Playwright/browser runs, workflows, CI, builds, migrations or `git diff --check` were executed by this plan actualization.
