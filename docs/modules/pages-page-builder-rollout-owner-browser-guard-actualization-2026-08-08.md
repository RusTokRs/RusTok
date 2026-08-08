# Pages / Page Builder rollout owner and browser-intent guard actualization — 2026-08-08

Status: `source-review-complete / runtime-evidence-not-run / gate-unaccepted`.

Base: `main@9221664d677f3f8775ef4ade66ffe14a4b316c54`.

## Finding

Source review after the initial rollout binding found two related ownership gaps before runtime execution:

1. `rustok-pages-admin` attempted to resolve `HostRuntimeContext` for tenant-module settings, while the standalone admin host does not own or provide the tenant database runtime.
2. `/api/admin/pages/{page_id}/builder/intents` only applied role/contribution capabilities before draft dispatch, so a handcrafted browser intent could bypass persisted Page Builder rollout narrowing.

Neither gap had runtime evidence accepted, so the source design is corrected here before the reference-consumer gate can advance.

## Correction

Pages server now owns the persisted rollout read through GraphQL `pageBuilderRolloutSnapshot`. The resolver is tenant-bound, requires effective `Pages:Read`, reads only the enabled Pages module settings row, normalizes the shared builder flags, and never returns raw settings. Database errors are logged server-side and not reflected to the client.

`BuilderCapabilityFlags::from_module_settings` is now the shared normalization contract for the nested module-settings shape and preserves all four canonical profiles plus backwards-compatible omitted-key defaults.

`rustok-pages-admin` is a stateless transport consumer. Its UI workspace and authoritative SSR Preview/Publish path both fetch the server-owned snapshot and verify the routed tenant identity.

The standalone browser-intent route fetches the same snapshot, intersects it with role capabilities, and runs capability preflight before any draft-store dispatch. Browser-supplied rollout flags are never accepted.

## Source review notes

- `Permission::PAGES_READ` exists and effective permission checks include `Pages:Manage` through the existing helper.
- `AuthContext` and `TenantContext` contain the tenant identity required by the server owner.
- `rustok-pages` already depends on `rustok-page-builder`, so shared normalization introduces no new package edge.
- `rustok-pages-admin` no longer needs `leptos_axum` or `rustok-api/server` solely for rollout settings.
- `GraphqlHttpError::Graphql(String)` matches the new transport validation path.
- provider health remains explicitly unobserved.
- no settings mutation or parallel settings store was added.

## Boundary

This correction does not produce four-profile runtime evidence and does not accept `pages_reference_consumer_gate`. Forum Wave remains blocked and FFA/FBA are not promoted.

Next cursor: retain the bounded four-profile runtime matrix using production module-settings writes with mandatory restoration, then let the maintainer execute it on one exact source/deployment.

No tests, Node verifiers, Cargo commands, formatting, database scenarios, HTTP requests, browser runs, workflows, CI, or `git diff --check` were executed.
