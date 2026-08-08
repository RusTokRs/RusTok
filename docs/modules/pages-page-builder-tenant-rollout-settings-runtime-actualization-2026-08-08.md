# Pages / Page Builder tenant rollout settings runtime actualization — 2026-08-08

Status: `platform-settings-read-seam-source-ready / pages-server-owner-graphql-source-ready / all-consumer-bindings-source-ready / four-profile-runtime-evidence-pending / gate-acceptance-pending`.

Current correction base: `main@9221664d677f3f8775ef4ade66ffe14a4b316c54`.

## Canonical persisted settings source

`rustok_api::tenant_module_settings` remains the read-only platform seam for the exact enabled `tenant_modules` row. It is backend-aware for SQLite, PostgreSQL and MySQL, fails closed on malformed stored JSON, and does not mutate settings.

Tenant authority now belongs to the Pages **server owner**, not `rustok-pages-admin`. GraphQL `pageBuilderRolloutSnapshot` resolves server auth/routed tenant, requires tenant equality and `Pages:Read`, reads the enabled Pages settings row, then normalizes the shared nested shape through `BuilderCapabilityFlags::from_module_settings`:

```text
builder.enabled
builder.preview.enabled
builder.properties.enabled
builder.publish.enabled
```

Missing keys retain the backward-compatible all-on default. Present non-boolean values and invalid flag combinations fail closed.

## Stateless admin consumption

`rustok-pages-admin` fetches only the typed server-owned snapshot through its ordinary GraphQL token/tenant transport. It does not require direct tenant persistence access or a standalone-admin `HostRuntimeContext`.

The transport rejects an unexpected observed-health claim, validates returned flags, and rejects a routed tenant slug that differs from the admin-requested tenant.

## Pages consumer bindings

The selected Pages workspace loads the typed snapshot before mounting Page Builder and injects the flags into `PagesBuilderFacade::with_provider_flags(...)`. Provider health remains `unobserved`.

Preview and publish independently refetch the server-owned snapshot on authoritative SSR dispatch and compose Page Builder handlers from those flags.

The standalone browser-intent route also fetches the server-owned snapshot and narrows role capabilities before intent preflight/draft dispatch. This means `builder_off`, properties-disabled and publish-disabled profiles cannot be bypassed with handcrafted browser intents.

Browser-supplied rollout flags are not accepted.

## Current boundary

The platform seam, Pages server-owner GraphQL snapshot, and all real consumer bindings are source-ready. The four declared profiles are technically exercisable through UI provider status, authoritative SSR dispatch and browser-intent preflight.

This is **not** runtime acceptance. No four-profile execution packet has been retained. Provider health remains `unobserved`; `pages_reference_consumer_gate` remains `accepted=false`; Forum Wave and FFA/FBA remain blocked.

## Next exact cursor

Retain a bounded four-profile runtime matrix for `all_on`, `publish_off`, `preview_off`, and `builder_off` on one exact source/deployment. It must use production module-settings authority, restore the original Pages settings even after failure, verify UI/SSR/browser-intent agreement, prove Pages-owned list/document reads remain available, and avoid fabricating provider health.

Maintainer execution remains separate.

No tests, Node verifiers, Cargo commands, formatting, database scenarios, HTTP requests, browser runs, workflows, CI, or `git diff --check` were executed by this implementation slice.
