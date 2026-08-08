# Pages / Page Builder rollout server snapshot actualization — 2026-08-08

Status: `pages-server-owner-graphql-source-ready / stateless-admin-transport-source-ready / ui-ssr-browser-intent-bindings-source-ready / four-profile-runtime-evidence-pending`.

Current correction rechecked at `main@9221664d677f3f8775ef4ade66ffe14a4b316c54` and continued on `agent/pages-builder-browser-rollout-guard-20260808`.

## Ownership correction

The earlier source path placed `HostRuntimeContext` lookup inside `rustok-pages-admin`. A standalone admin host does not own the tenant database runtime; its request context contains auth/session and UI security state. Therefore that source design was not a valid standalone runtime ownership boundary and is superseded here before runtime acceptance.

Pages server now owns rollout persistence access through GraphQL `pageBuilderRolloutSnapshot` in `crates/rustok-pages/src/graphql/builder_rollout.rs`.

The resolver:

1. requires the Pages module to be enabled;
2. resolves server `AuthContext` and routed `TenantContext`;
3. requires authenticated tenant id to equal routed tenant id;
4. requires effective `Pages:Read`;
5. reads the exact enabled `pages` settings row through `rustok_api::tenant_module_settings`;
6. normalizes the nested builder flags with shared `BuilderCapabilityFlags::from_module_settings`;
7. returns only tenant identity plus typed rollout flags;
8. explicitly returns `providerHealthObserved=false`.

Raw module settings never cross into the admin client.

## Stateless admin transport

`rustok-pages-admin` consumes the GraphQL snapshot through the same token/tenant transport boundary as other Pages operations. It no longer requires `HostRuntimeContext`, `leptos_axum` extraction, or `rustok-api/server` solely for rollout settings.

The transport rejects an unexpected observed-health claim, validates the returned flags, and verifies the routed tenant slug matches the tenant requested by the admin surface.

## Consumer bindings

The selected Pages workspace loads the server-owned flags before Page Builder mounts and injects them into `PagesBuilderFacade::with_provider_flags(...)`. Provider health stays unobserved.

Every authoritative Preview/Publish SSR capability dispatch independently fetches the server-owned snapshot again and composes Page Builder handlers from those freshly returned flags.

The standalone `/api/admin/pages/{page_id}/builder/intents` route also fetches the same server-owned snapshot and intersects it with role capabilities **before** browser-intent preflight/draft dispatch. Therefore `builder_off` and properties/publish-disabled profiles cannot be bypassed with a handcrafted browser intent.

Browser-provided rollout flags are never accepted as authority.

## Current boundary

Source ownership and all three consumer bindings (UI provider status, authoritative SSR capability dispatch, standalone browser-intent preflight) are ready for runtime evidence. This does not claim execution.

`pages_reference_consumer_gate` remains `accepted=false`; provider health remains `unobserved`; no four-profile runtime packet has been retained; Forum Wave and FFA/FBA remain blocked.

## Next exact cursor

Retain a bounded four-profile runtime matrix using production module-settings authority, exact-source/deployment identity, mandatory restoration of original Pages settings, browser-intent denial checks, and Pages-owned list/document read guarantees. Maintainer execution remains separate.

No tests, Node verifiers, Cargo commands, formatting, database scenarios, HTTP requests, browser runs, workflows, CI, or `git diff --check` were executed by this implementation slice.
