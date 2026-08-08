# Pages / Page Builder trusted rollout server snapshot actualization — 2026-08-08

Status: `trusted-server-snapshot-source-ready / ui-facade-binding-pending / ssr-dispatch-binding-pending / four-profile-runtime-evidence-blocked`.

Base rechecked: `main@0ae8f7f4382a4218107c43a4e1e6f1e5b957a84e`.

## Why this slice exists

The previous slice added the neutral `rustok_api::tenant_module_settings` read seam. It deliberately did not decide tenant authority or expose a Pages-specific transport.

The Pages reference consumer still needs a trusted way to turn the routed tenant's persisted module settings into `BuilderCapabilityFlags` without accepting rollout state from browser input. That trusted snapshot must be reusable by the admin facade and the authoritative SSR Page Builder capability dispatch before the four gate profiles can be exercised honestly.

## Trusted server snapshot

The trusted server snapshot is source-ready in `crates/rustok-pages/admin/src/builder_rollout_settings.rs`.

`pages_builder_rollout_flags` is a native server-function endpoint. On the server it:

1. resolves the host `HostRuntimeContext`;
2. extracts `AuthContext` and routed `TenantContext`;
3. rejects an auth/routed-tenant mismatch;
4. requires effective `Pages:Read` authority;
5. reads the exact enabled `pages` row through `rustok_api::tenant_module_settings`;
6. normalizes only the declared nested builder flags;
7. validates the resulting `BuilderCapabilityFlags` combination before returning it.

The raw tenant-module settings document is never returned. The client-visible result is only the normalized Page Builder capability flag structure.

Omitted builder keys preserve the existing backward-compatible all-on defaults. Invalid persisted combinations fail closed instead of being repaired silently. Source tests retain all four declared profiles plus default and invalid-combination behavior.

The browser never supplies rollout flags to this endpoint; it can only request the server-owned snapshot.

## Authority boundary

The low-level `rustok_api::tenant_module_settings` helper remains authority-neutral. This Pages adapter supplies the tenant id from `TenantContext` and explicitly checks that the authenticated tenant matches the routed tenant before reading settings.

This follows the same transport boundary used elsewhere by native admin adapters: tenant routing establishes scope, authenticated authority is bound to that scope, and only then is tenant-owned state read.

## Current boundary

The trusted server snapshot is source-ready.

UI facade binding remains pending: `PagesBuilderFacade::provider_status()` still reads the hardcoded default provider flags.

SSR capability dispatch binding remains pending: the Page Builder handler dispatch still composes handlers from the hardcoded default provider flags.

Therefore the four-profile runtime matrix is still blocked and `pages_reference_consumer_gate` remains unaccepted. Provider health remains `unobserved`; Forum Wave and FFA/FBA remain blocked.

## Next exact source cursor

Wire the server-owned snapshot into both remaining consumer seams without creating a browser-controlled authority path:

- load `pages_builder_rollout_flags()` as part of the Pages workspace data needed to construct `PagesBuilderFacade` and pass the returned flags into provider status;
- make authoritative SSR Page Builder capability dispatch independently read the same routed-tenant settings snapshot on every request rather than trusting the UI copy;
- remove the hardcoded `BuilderCapabilityFlags::default()` source once both paths use equivalent normalization;
- retain source tests proving all four profiles map to the exact expected admin/provider and SSR handler capability outcomes.

Only after that binding is complete should a four-profile runtime evidence harness be admitted to the Pages reference-consumer candidate packet.

## Source evidence

- `crates/rustok-pages/contracts/evidence/pages-builder-rollout-server-snapshot-source.json`
- `crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-server-snapshot.mjs`
- `crates/rustok-pages/admin/src/builder_rollout_settings.rs`

No tests, Node verifiers, Cargo commands, formatting, database scenarios, HTTP requests, browser runs, workflows, or CI were executed by this implementation slice.
