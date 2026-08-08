# Pages / Page Builder rollout feature preflight — 2026-08-08

Status: `source-ready / maintainer-execution-pending / candidate-input-pending / gate-unaccepted`.

Base: `main@a08337a73bba49ee57dc932d7e0128ac545e2071`.

## Why this packet exists

The existing four-profile runtime matrix proves real Pages UI state, Preview SSR behavior, standalone browser-intent preflight and Pages-owned reads. During candidate integration review, one contract mismatch became explicit: standalone browser intents intentionally return `FLY_CAPABILITY_DENIED`, while the canonical Page Builder degraded-mode catalog requires `feature-disabled / FEATURE_DISABLED` for disabled provider capabilities.

Those are different contracts and must not be treated as interchangeable evidence.

This packet adds a non-mutating server-owned preflight for the canonical provider error catalog and a bounded four-profile evidence harness. It does not replace the runtime matrix; the two packets prove different layers and are both required before a reference-gate candidate can be produced.

## Server-owned non-mutating preflight

`crates/rustok-pages/src/graphql/builder_rollout.rs` now exposes `pageBuilderCapabilityPreflight`.

The query:

- resolves the routed `TenantContext` and `AuthContext` on the Pages GraphQL server;
- rejects a tenant mismatch;
- uses the exact Preview/Tree→`pages:read`, Properties→`pages:update`, Publish→`pages:publish` mapping used by `PageBuilderCapabilityPermissions`;
- keeps that mapping source-locked against the server-only Page Builder authorizer instead of enabling the Page Builder `server` feature inside `rustok-pages`;
- requires the effective permission before rollout evaluation;
- reads the persisted Pages tenant-module settings through the existing server-owned rollout loader;
- evaluates the same `rustok_page_builder::rollout::ensure_capability` guard used by `CapabilityGuardedService`;
- returns `allowed=true` with no error contract for an enabled capability;
- returns `allowed=false`, `errorKind=feature-disabled`, `errorCode=FEATURE_DISABLED` for a disabled capability.

The preflight never invokes Preview rendering or Publish persistence. This is deliberate: `CapabilityGuardedService` already checks `ensure_capability` before write-policy validation and before its inner Publish service/store, so the query gives the canonical provider error contract without creating a document write path.

The source guard verifies both mappings together. Any future change in `PageBuilderCapabilityPermissions` that is not mirrored by the lightweight Pages preflight mapping fails the feature-preflight source contract instead of silently changing authorization semantics.

## Four-profile harness

The source packet consists of:

- `crates/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-execution-contract.json`
- `apps/next-admin/playwright.pages-builder-rollout-feature-preflight.config.ts`
- `apps/next-admin/tests/pages-builder-rollout-feature-preflight/feature-preflight.spec.ts`
- `crates/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-harness-source.json`
- `crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-feature-preflight-harness.mjs`

The execution chain is strictly:

```text
browser -> rollout matrix -> feature preflight
```

The browser and matrix predecessor packets must use the exact source commit. The matrix must bind the exact browser packet hash. API origin and immutable API/server RepoDigest must match the predecessor chain. The matrix must already prove that its own Pages settings were restored before the feature preflight starts.

## Production settings ownership

The feature-preflight harness does not use SQL or database credentials. It snapshots the complete current Pages settings through `tenantModules` and applies each profile through production `updateModuleSettings`.

Only these four values are changed:

```text
builder.enabled
builder.preview.enabled
builder.properties.enabled
builder.publish.enabled
```

Other Pages settings are preserved. The original settings are restored in `finally` through the same production mutation, re-read, and required to match by canonical semantic hash before output can be retained.

## Required operator authority

The reviewed API session must have:

- `modules:manage` for controlled settings changes;
- effective `pages:read` for Preview preflight;
- effective `pages:update` for Properties preflight;
- effective `pages:publish` for Publish preflight.

This matters because the actual Page Builder handler authorizes a capability before the rollout guard. The harness therefore cannot manufacture a `FEATURE_DISABLED` result by using an operator that would be forbidden earlier by RBAC.

## Expected profiles

The profile expectations are:

| Profile | Preview | Properties | Publish |
| --- | --- | --- | --- |
| `all_on` | allowed | allowed | allowed |
| `publish_off` | allowed | allowed | `feature-disabled / FEATURE_DISABLED` |
| `preview_off` | `feature-disabled / FEATURE_DISABLED` | allowed | `feature-disabled / FEATURE_DISABLED` |
| `builder_off` | `feature-disabled / FEATURE_DISABLED` | `feature-disabled / FEATURE_DISABLED` | `feature-disabled / FEATURE_DISABLED` |

The `all_on` Publish check is a true dry preflight: it confirms the authoritative permission + rollout path would allow Publish but never calls Publish persistence.

The existing runtime matrix separately keeps the standalone browser-intent `FLY_CAPABILITY_DENIED` checks. Those prove direct browser POST bypass resistance and remain useful; they are no longer confused with the provider `FEATURE_DISABLED` catalog.

## Retained evidence

The output is:

```text
target/pages-builder-rollout-feature-preflight.json
```

with:

```text
format: pages_builder_rollout_feature_preflight_v1
status: four_profile_feature_preflight_passed_candidate_pending
```

It retains source hashes, predecessor/storage-state hashes and sizes, API origin hash, immutable RepoDigest, original settings semantic hash, response status/body hashes and bounded capability outcome facts.

It does not retain tenant identity, credentials, storage-state contents, raw settings, GraphQL bodies or database URLs. Trace, screenshots and video are disabled.

## Promotion boundary

A successful packet is only an input to the Pages reference-consumer candidate. It does not accept `pages_reference_consumer_gate`, does not claim provider SLO health, does not accept Forum Wave and does not promote FFA/FBA.

## Maintainer execution

After exact-source browser and rollout-matrix evidence exists:

```bash
cd apps/next-admin
npx --no-install playwright test --config playwright.pages-builder-rollout-feature-preflight.config.ts
```

Source guard:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-feature-preflight-harness.mjs
```

No tests, Node verifiers, Cargo commands, formatting, GraphQL/HTTP requests, Playwright/browser runs, workflows, CI, builds or migrations were executed by this implementation slice.
