# Pages / Page Builder tenant rollout settings runtime actualization — 2026-08-08

Status: `platform-settings-read-seam-source-ready / pages-server-owner-graphql-source-ready / all-consumer-bindings-source-ready / promotion-safe-settings-cas-transport-source-ready / ffa-fba-promotion-execution-source-ready / four-profile-runtime-evidence-pending / gate-acceptance-pending / ffa-fba-promotion-live-execution-pending / readiness-governance-pending`.

Current correction base: `main@85c5f608882a523c8583c832c679d59b91e6ba98`.

## Canonical persisted settings source

`rustok_api::tenant_module_settings` remains the read-only platform seam for the exact enabled `tenant_modules` row. It is backend-aware for SQLite, PostgreSQL and MySQL, fails closed on malformed stored JSON, and does not mutate settings.

Tenant authority belongs to the Pages **server owner**, not `rustok-pages-admin`. GraphQL `pageBuilderRolloutSnapshot` resolves server auth/routed tenant, requires tenant equality and `Pages:Read`, reads the enabled Pages settings row, then normalizes the shared nested shape through `BuilderCapabilityFlags::from_module_settings`:

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

The selected Pages workspace loads the typed snapshot before mounting Page Builder and injects the flags into `PagesBuilderFacade::with_provider_flags(...)`. Provider health remains `unobserved` unless a separate accepted deployment-health binding is installed.

Preview and publish independently refetch the server-owned snapshot on authoritative SSR dispatch and compose Page Builder handlers from those flags.

The standalone browser-intent route also fetches the server-owned snapshot and narrows role capabilities before intent preflight/draft dispatch. This means `builder_off`, properties-disabled and publish-disabled profiles cannot be bypassed with handcrafted browser intents.

Browser-supplied rollout flags are not accepted.

## Promotion-safe settings write transport

The canonical unconditional `updateModuleSettings` command remains the ordinary tenant settings editor contract, including its Core-row upsert semantics. Reviewed rollout automation uses a separate conditional command because it has different write semantics rather than a compatibility version of the same command.

`compareAndSwapModuleSettings` is mounted into the assembled server GraphQL mutation root through `ModuleSettingsCasMutation`. It:

- requires the routed authenticated tenant and `modules:manage`;
- accepts the exact reviewed `expectedEnabled` bit and full `expectedSettings` JSON together with the proposed settings JSON;
- normalizes both expected and proposed settings through the active module settings schema;
- delegates to `ModuleRolloutPromotionSettingsService` and the lifecycle-owner/store CAS introduced by the promotion-safety chain;
- returns `MODULE_SETTINGS_SNAPSHOT_CONFLICT` with `requires_rereview=true` when either enablement or settings changed since review;
- never treats a caller-supplied approval boolean, reviewer id, or evidence path as review authority.

This GraphQL command is an execution transport only. The Forum FFA/FBA promotion-review packet remains a maintainer evidence artifact whose asserted owner identity is not a cryptographic server credential. The mutation itself does not approve review evidence and its existence does not promote FFA/FBA readiness.

## Promotion execution harness

`crates/rustok-forum/contracts/evidence/forum-page-builder-ffa-fba-promotion-execution-source.json` and `scripts/evidence/execute-forum-page-builder-ffa-fba-promotion.mjs` now define the separate maintainer execution step after an approved promotion review.

The runner revalidates the approved review packet against exact checkout `HEAD`, the immutable deployment RepoDigest, the retained observed-Wave freshness/admission-lineage facts and the still-live Wave lease before making a target request. Target origin, tenant and auth come from bounded environment inputs and are never retained raw.

The operator requires both `modules:manage` for the settings CAS and `pages:read` for the authoritative `pageBuilderRolloutSnapshot` postcondition.

The runner:

1. reads the current enabled Pages row through `tenantModules`;
2. clones the complete current settings document;
3. changes only the four Page Builder rollout booleans to the reviewed `all_on` profile and preserves every other setting;
4. calls `compareAndSwapModuleSettings` with the exact current enabled/settings snapshot;
5. verifies the returned settings through a fresh `tenantModules` read and verifies all four flags through `pageBuilderRolloutSnapshot`;
6. retains only bounded response and semantic hashes, never raw settings or credentials.

A `MODULE_SETTINGS_SNAPSHOT_CONFLICT` is terminal for that review and requires a new read and new review. It is never converted into an unconditional overwrite.

If the mutation is confirmed but the postcondition fails, the runner attempts one CAS rollback whose expected settings are the confirmed applied snapshot and whose restore target is the exact original snapshot. Confirmed rollback still fails the promotion execution and records `control_plane_change_postcondition_failed_rolled_back`. Rollback conflict or ambiguity records manual reconciliation.

If the initial mutation outcome is ambiguous, the runner deliberately does **not** attempt rollback because it cannot safely know whether the write committed; it records `control_plane_change_requires_manual_reconciliation`.

A target that is already semantically `all_on` is not accepted as new execution evidence.

## Current boundary

The platform read seam, Pages server-owner GraphQL snapshot, real consumer bindings, promotion-safe CAS transport and promotion-execution harness source are ready. The four rollout profiles remain technically exercisable through UI provider status, authoritative SSR dispatch and browser-intent preflight.

This is **not** runtime acceptance. No new live promotion execution is claimed. Provider-health, Pages gate, Forum Wave and review packets still must be produced on the required exact lineage before the execution runner is eligible to mutate a tenant.

A successful runner receipt has status `control_plane_change_executed_readiness_promotion_pending`. It proves only the reviewed tenant/control-plane settings transition and its postcondition. The runner leaves `ffa_promoted=false` and `fba_promoted=false` and does not change repository source.

## Next exact cursor

Retain the bounded four-profile runtime matrix and complete the accepted provider-health, Pages gate, Forum execution/admission, observed Wave, freshness/lineage and observed-Wave owner-decision chain on one valid exact deployment.

While that retained Wave lease is still live, run the explicit FFA/FBA promotion review. Only after status `owner_approved_ffa_fba_promotion_review_execution_pending` may a maintainer run `execute-forum-page-builder-ffa-fba-promotion.mjs` against the exact reviewed deployment.

A successful execution receipt is then input to a **separate** evidence-backed readiness governance review. FFA `parity_verified` and FBA `transport_verified` must not be inferred from source-ready CAS transport or from the tenant settings write alone.

Maintainer live execution remains separate.

No tests, Node verifiers, Cargo commands, formatting, database scenarios, GraphQL/HTTP requests, browser runs, workflows, CI, or `git diff --check` were executed by this implementation slice.
