# Pages / Page Builder tenant rollout settings runtime actualization — 2026-08-08

Status: `platform-settings-read-seam-source-ready / pages-consumer-binding-source-ready / four-profile-runtime-evidence-pending / gate-acceptance-pending`

Current binding rechecked at `main@488803a831e725ef0fbeaa8540f09458ee461b85` and continued on `agent/pages-builder-rollout-ssr-binding-20260808`.

## Why this boundary matters

The Pages reference-consumer gate requires observed behavior for `all_on`, `publish_off`, `preview_off`, and `builder_off`. Earlier source state could only expose the hardcoded all-on profile through the real Pages consumer, so focused provider tests were not enough to prove persisted tenant rollout behavior.

That source blocker is now closed.

## Canonical persisted settings source

`rustok_api::tenant_module_settings` remains the read-only platform seam for the exact enabled `tenant_modules` row. It is backend-aware for SQLite, PostgreSQL and MySQL, fails closed on malformed stored JSON, and does not own tenant authority or mutate settings.

Pages supplies that authority in `builder_rollout_settings.rs`: the server extracts `AuthContext` and routed `TenantContext`, requires their tenant ids to match, requires effective `Pages:Read`, fixes the module slug to `pages`, and normalizes only:

```text
builder.enabled
builder.preview.enabled
builder.properties.enabled
builder.publish.enabled
```

Missing keys retain the backward-compatible all-on defaults. Present non-boolean values and invalid flag combinations fail closed.

## Pages consumer binding is source-ready

The real Pages admin consumer now uses the trusted settings path on both sides of the Page Builder boundary.

### Admin provider status

The workspace loads `pages_builder_rollout_flags()` before mounting the selected Page Builder workspace. The returned server-owned `BuilderCapabilityFlags` are passed into `PagesBuilderFacade::with_provider_flags(...)`; `provider_status()` exposes them as `PageBuilderAdminProviderStatus::unobserved(...)`.

This narrows the UI through the existing provider-status state machine without fabricating observed health. If the trusted rollout snapshot cannot be loaded or validated, the workspace returns an error instead of silently mounting an all-on builder.

### Authoritative SSR capability dispatch

Preview and publish do not trust the UI copy. Every SSR `dispatch_pages_page_builder_capability(...)` independently calls `load_trusted_pages_builder_rollout_snapshot()` and passes those freshly normalized flags to `compose_fly_page_builder_handlers(...)`.

The request snapshot tenant slug must also equal the routed trusted tenant slug before capability dispatch. Browser-intent persistence reaches the same facade/SSR dispatch and therefore cannot bypass the server-owned rollout state.

The old `pages_builder_capability_flags() -> BuilderCapabilityFlags::default()` consumer binding is removed.

## Current boundary

The platform read seam and Pages UI/SSR consumer binding are source-ready. Therefore the four declared profiles are now technically exercisable through the real reference consumer.

This is **not** runtime acceptance. No four-profile execution packet has been retained in this slice. Provider health remains `unobserved`; `pages_reference_consumer_gate` remains `accepted = false`; Forum Wave remains blocked; FFA/FBA are not promoted.

## Next exact cursor

Retain a bounded four-profile Pages runtime matrix that drives persisted tenant settings for `all_on`, `publish_off`, `preview_off`, and `builder_off` on one exact source/deployment and proves:

- the expected preview/properties/publish-dry result for each profile;
- Pages-owned list/document reads remain available in every profile;
- the admin provider status and authoritative SSR dispatch agree on the effective profile;
- no observed provider-health claim is fabricated.

Maintainer execution of that matrix remains separate. Only accepted execution evidence should advance the gate toward owner sign-off and rollback disposition.

## Source evidence

- `crates/rustok-pages/contracts/evidence/pages-tenant-rollout-settings-runtime-source.json`
- `crates/rustok-pages/contracts/evidence/pages-builder-rollout-server-snapshot-source.json`
- `crates/rustok-pages/scripts/verify/verify-pages-tenant-rollout-settings-runtime.mjs`
- `crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-server-snapshot.mjs`

No tests, Node verifiers, Cargo commands, formatting, database scenarios, HTTP requests, browser runs, workflows, or CI were executed by this implementation slice.
