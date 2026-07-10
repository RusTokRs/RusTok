---
id: doc://docs/modules/page-builder-implementation-plan.md
kind: development_plan
language: en
status: active
---
# Page Builder Implementation Plan

## Current verified state

`rustok-page-builder` owns the GrapesJS capability boundary and its FBA provider contracts. `rustok-pages` is the reference consumer; feature profiles, fallback semantics, evidence packets, and rollout state remain provider/consumer concerns rather than Rich Text concerns.

## Next priorities

1. Replace synthetic Page Builder Wave evidence with a real tenant control-plane dry run and correlation packet. When Flutter joins a Wave 1 rollout, attach Flutter device/runtime evidence without duplicating provider FBA metadata. Completion: the provider and `pages` consumer verifiers accept the packet without waiver.
2. Complete persistence and rendering adapters behind the existing capability contract, then expose owner-owned GraphQL and native server-function endpoints. Completion: tenant-scoped preview, save, publish, and fallback behavior have targeted runtime evidence.
3. Start the `forum` consumer only after the `pages` reference integration has a verified fallback and rollout profile. Completion: forum widgets consume the public builder contract without importing provider internals.

## FFA/FBA boundary

The provider and each consumer retain separate ownership. FBA evidence, central readiness status, and local consumer plans must remain synchronized; host code only composes owner-owned surfaces.

## Fallback matrix

| Profile | Builder path | Read/list paths |
| --- | --- | --- |
| `all_on` | available | `stable` |
| `publish_off` | publish returns `typed_feature_disabled_error` | `stable` |
| `preview_off` | preview returns `typed_feature_disabled_error` | `stable` |
| `builder_off` | `readonly_fallback`; capabilities return `typed_feature_disabled_error` | `stable` |

## Correlation evidence

Wave evidence must correlate `builder write -> pages publish -> storefront read`; the packet records the capability result, pages publish outcome, and storefront read result for the same tenant and correlation identifier.

## Dependencies and verification

- Dependency: `rustok-pages` is the first reference consumer; `rustok-forum` depends on the verified provider/consumer contract.
- Targeted verification: `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs pages`.
