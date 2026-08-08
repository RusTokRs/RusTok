# Pages / Page Builder tenant rollout settings runtime actualization — 2026-08-08

Status: `platform-settings-read-seam-source-ready / pages-consumer-binding-pending / four-profile-runtime-evidence-blocked / gate-acceptance-pending`

Base rechecked: `main@e91926363cfcdf6a91836358add1afdf94f65ffa`.

## Why this slice exists

The Pages reference-consumer gate requires observed behavior for four provider profiles: `all_on`, `publish_off`, `preview_off`, and `builder_off`. PR #3326 retained a bounded candidate harness and focused Page Builder provider-profile tests, but a source recheck found that the Pages reference consumer itself cannot yet be driven through those persisted tenant profiles.

`crates/rustok-pages/admin/src/builder.rs` still owns this source function:

```text
pages_builder_capability_flags() -> BuilderCapabilityFlags
```

and it currently returns the hardcoded `BuilderCapabilityFlags::default()` value. The same hardcoded all-on snapshot is exposed through `PagesBuilderFacade::provider_status()` and passed to SSR `compose_fly_page_builder_handlers(...)`.

That is sufficient to keep both sides internally consistent for the default profile, but it is not sufficient to claim tenant-scoped execution of `publish_off`, `preview_off`, or `builder_off`. A candidate packet produced without fixing this boundary would only exercise provider-profile logic in focused tests, not the actual persisted Pages tenant rollout state required by the gate.

Therefore owner review is not the next source step yet.

## Existing owner settings contract

Pages already interprets the persisted nested module settings shape in its owner service helpers:

```text
builder.enabled
builder.preview.enabled
builder.properties.enabled
builder.publish.enabled
```

The canonical `crates/rustok-pages/rustok-module.toml` declares the same four toggle profiles and explicitly requires Pages-owned read paths to remain available when Page Builder capabilities are disabled.

The missing part is transport/composition: Pages admin and its SSR Page Builder capability dispatch do not currently receive the exact enabled `tenant_modules.settings` snapshot.

## Neutral platform read seam

This slice adds `rustok_api::tenant_module_settings` next to the existing `rustok_api::is_tenant_module_enabled` runtime helper.

The helper is intentionally read-only and provider-neutral:

- it requires an exact tenant id and module slug;
- it returns settings only from an exact **enabled** `tenant_modules` row;
- disabled or missing rows return `None`;
- malformed stored JSON fails closed as a database error;
- SQLite, PostgreSQL, and MySQL use backend-specific read forms;
- it does not import tenant persistence entities;
- it does not mutate settings;
- it does not create a second settings store or Pages-owned control plane.

This keeps tenant-module lifecycle/settings ownership outside Pages while giving internal server adapters one neutral way to consume the persisted snapshot.

## Current boundary

The platform read seam is source-ready.

Pages consumer binding remains pending.

The four-profile runtime matrix remains blocked until Pages stops deriving both its admin provider status and SSR handler composition from the hardcoded all-on function.

`pages_reference_consumer_gate` remains `accepted = false`; provider health remains `unobserved`; Forum Wave and FFA/FBA remain unaccepted/unpromoted.

## Next exact source cursor

Bind one normalized `BuilderCapabilityFlags` snapshot derived from the exact enabled Pages tenant-module settings into both sides of the reference-consumer boundary:

1. the `PagesBuilderFacade::provider_status()` snapshot used to narrow the admin surface;
2. the authoritative SSR `compose_fly_page_builder_handlers(...)` capability composition.

The server side must independently resolve the trusted routed tenant and persisted Pages module settings rather than accepting rollout flags supplied by the browser. UI and SSR must use equivalent normalization/default rules so they cannot disagree about preview/properties/publish availability.

After that binding is source-ready, retain a four-profile runtime evidence harness proving the exact gate outcomes and Pages-owned read guarantees. Only then should the candidate evidence advance to owner sign-off and rollback disposition.

## Source evidence

- `crates/rustok-pages/contracts/evidence/pages-tenant-rollout-settings-runtime-source.json`
- `crates/rustok-pages/scripts/verify/verify-pages-tenant-rollout-settings-runtime.mjs`
- `crates/rustok-api/src/runtime.rs`

No tests, Node verifiers, Cargo commands, formatting, database scenarios, HTTP requests, browser runs, workflows, or CI were executed by this implementation slice.
