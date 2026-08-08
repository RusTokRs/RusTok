# Pages / Page Builder trusted rollout server snapshot actualization — 2026-08-08

Status: `trusted-server-snapshot-source-ready / ui-facade-binding-source-ready / ssr-dispatch-binding-source-ready / four-profile-runtime-evidence-pending`.

Current binding rechecked at `main@488803a831e725ef0fbeaa8540f09458ee461b85` and continued on `agent/pages-builder-rollout-ssr-binding-20260808`.

## Trusted server snapshot

`crates/rustok-pages/admin/src/builder_rollout_settings.rs` owns the Pages-specific trusted normalization boundary.

The server:

1. resolves `HostRuntimeContext`;
2. extracts `AuthContext` and routed `TenantContext`;
3. rejects an auth/routed-tenant mismatch;
4. requires effective `Pages:Read` authority;
5. reads the exact enabled `pages` row through `rustok_api::tenant_module_settings`;
6. normalizes only the declared builder flags;
7. rejects malformed setting types and invalid flag combinations;
8. returns a trusted snapshot containing normalized flags plus the routed tenant slug.

The client-visible server function still returns only `BuilderCapabilityFlags`; raw tenant-module settings and tenant authority are not exposed.

## UI facade binding

The Pages workspace loads the server-owned rollout flags before mounting Page Builder. It supplies those flags to `PagesBuilderFacade::with_provider_flags(...)` and the facade reports them through `PageBuilderAdminProviderStatus::unobserved(...)`.

No provider-health observation is invented. Failure to load or validate the trusted rollout state fails the selected workspace instead of falling back to a hardcoded all-on provider status.

## Authoritative SSR dispatch binding

Every Preview/Publish capability dispatch independently calls `load_trusted_pages_builder_rollout_snapshot()` on the server. Before Page Builder composition, the request snapshot tenant slug must match the routed trusted tenant slug.

`compose_fly_page_builder_handlers(...)` receives the freshly reread trusted flags, not the UI copy. Browser-intent persistence reaches this same dispatch path, so browser-controlled rollout flags are never an authority source.

The prior hardcoded `pages_builder_capability_flags() -> BuilderCapabilityFlags::default()` binding has been removed from `builder.rs`.

## Current boundary

Both UI facade and authoritative SSR rollout bindings are source-ready. The four declared profiles can now be exercised through the real Pages reference consumer.

Execution is still pending: no four-profile runtime packet was produced, no provider SLO health was observed, and `pages_reference_consumer_gate` remains `accepted=false`. This is the four-profile runtime-evidence-pending boundary. Forum Wave and FFA/FBA remain blocked.

## Next exact cursor

Retain a bounded source harness for the four persisted profiles and Pages-owned read guarantees, then let the maintainer execute it on one exact source revision and immutable deployment. Owner sign-off and rollback disposition remain after accepted runtime evidence.

## Source evidence

- `crates/rustok-pages/contracts/evidence/pages-builder-rollout-server-snapshot-source.json`
- `crates/rustok-pages/contracts/evidence/pages-tenant-rollout-settings-runtime-source.json`
- `crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-server-snapshot.mjs`
- `crates/rustok-pages/scripts/verify/verify-pages-tenant-rollout-settings-runtime.mjs`

No tests, Node verifiers, Cargo commands, formatting, database scenarios, HTTP requests, browser runs, workflows, or CI were executed by this implementation slice.
