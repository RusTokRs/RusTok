# Pages / Page Builder tenant rollout settings runtime actualization — 2026-08-08

Status: `platform-settings-read-seam-source-ready / pages-server-owner-graphql-source-ready / all-consumer-bindings-source-ready / promotion-safe-settings-cas-transport-source-ready / four-profile-runtime-evidence-pending / gate-acceptance-pending / ffa-fba-promotion-execution-pending`.

Current correction base: `main@77cb554c35cead2f24765b971ef3431d114ef3eb`.

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

## Promotion-safe settings write transport

The canonical unconditional `updateModuleSettings` command remains the ordinary tenant settings editor contract, including its Core-row upsert semantics. Reviewed rollout automation uses a separate conditional command because it has different write semantics rather than a compatibility version of the same command.

`compareAndSwapModuleSettings` is now mounted into the assembled server GraphQL mutation root through `ModuleSettingsCasMutation`. It:

- requires the routed authenticated tenant and `modules:manage`;
- accepts the exact reviewed `expectedEnabled` bit and full `expectedSettings` JSON together with the proposed settings JSON;
- normalizes both expected and proposed settings through the active module settings schema;
- delegates to `ModuleRolloutPromotionSettingsService` and the lifecycle-owner/store CAS introduced by the promotion-safety chain;
- returns `MODULE_SETTINGS_SNAPSHOT_CONFLICT` with `requires_rereview=true` when either enablement or settings changed since review;
- never treats a caller-supplied approval boolean, reviewer id, or evidence path as review authority.

This GraphQL command is an execution transport only. The Forum FFA/FBA promotion-review packet remains a maintainer evidence artifact whose asserted owner identity is not a cryptographic server credential. A future maintainer execution runner must validate the accepted promotion-review packet and its exact source/deployment lineage **before** invoking this mutation. The mutation itself does not approve review evidence and its existence does not promote FFA/FBA readiness.

## Current boundary

The platform seam, Pages server-owner GraphQL snapshot, all real consumer bindings, and the promotion-safe compare-and-swap transport are source-ready. The four declared profiles are technically exercisable through UI provider status, authoritative SSR dispatch and browser-intent preflight.

This is **not** runtime acceptance. No four-profile execution packet has been retained. Provider health remains `unobserved`; `pages_reference_consumer_gate` remains `accepted=false`; Forum Wave and FFA/FBA promotion execution remain blocked on their explicit evidence/decision chain.

## Next exact cursor

Retain a bounded four-profile runtime matrix for `all_on`, `publish_off`, `preview_off`, and `builder_off` on one exact source/deployment. It must use production module-settings authority, restore the original Pages settings even after failure, verify UI/SSR/browser-intent agreement, prove Pages-owned list/document reads remain available, and avoid fabricating provider health.

After accepted provider-health, Pages gate, Forum evidence/Wave, freshness/lineage, observed-Wave owner acceptance and an approved FFA/FBA promotion-review packet exist on the required exact lineage, the maintainer execution runner may use `compareAndSwapModuleSettings` for any reviewed tenant-settings mutation. A CAS conflict requires a fresh read and fresh review; it must never be retried by overwriting the newer state.

Readiness-board promotion to FFA `parity_verified` or FBA `transport_verified` remains a separate evidence-backed governance change and must not be inferred from source-ready CAS transport alone.

Maintainer execution remains separate.

No tests, Node verifiers, Cargo commands, formatting, database scenarios, HTTP requests, browser runs, workflows, CI, or `git diff --check` were executed by this implementation slice.
