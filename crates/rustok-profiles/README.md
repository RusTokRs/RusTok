# rustok-profiles

## Purpose

`rustok-profiles` owns the universal public profile domain for RusToK.

## Responsibilities

- Provide a single profile boundary for any authenticated platform user.
- Keep public profile data separate from auth identity, commerce customers, and future seller accounts.
- Own profile storage (`profiles`, `profile_translations`), migrations, and the reusable profile service contract.
- Own profile-to-taxonomy relation storage via `profile_tags`.
- Resolve localized profile copy and attached Taxonomy tag names with the same profile-owned preference order: requested locale, that profile's preferred locale, then tenant default locale; Taxonomy retains platform, deterministic first-available, and canonical-key terminal fallback for vocabulary names.
- Provide batched profile summary lookup for downstream author/member presentation without per-user fan-out; each profile keeps its own preferred-locale step during one batched tag vocabulary read.
- Provide a tenant-scoped base-row privacy read adapter whose decisions do not depend on localized copy, tags, or media joins.
- Resolve active `followers_only` access through bounded Social Graph owner reads before presentation summaries are loaded.
- Validate avatar/banner references through the Media owner read port before self-service profile writes.
- Consume Media-selected direct or immutable capability image descriptors without inventing storage or proxy URLs.
- Publish `ProfileMediaPublicImageProvider` as the transport-neutral runtime seam for embedded or extracted Media presentation providers.
- Provide explicit backfill helpers for provisioning missing profiles from existing user/customer data.
- Expose `ProfileMutationService` as the preferred production mutation boundary; its constructor requires a database connection and transactional event bus, and every mutation delegates to an owner-write/outbox transaction.
- Keep the older mutation methods on `ProfileService` only as deprecated compatibility shims. New callers must use the corresponding event-aware `ProfileMutationService` methods; repository production call sites are rejected by the Forum Search mutation boundary verifier.
- Expose `redact_profile_for_account_deactivation_in_tx` for host-owned account deletion orchestration. It hides tenant-scoped public presentation inside the caller transaction before durable `UserDeleted` publication; a missing profile is already a valid redacted state.
- Expose a request-scoped GraphQL `ProfileSummaryLoader` for host applications that need DataLoader-based batching and caching.
- Expose module-owned GraphQL transport for self-service and public profile lookups, including targeted profile update mutations.
- Publish a module-owned Leptos storefront profile page with explicit native/GraphQL transport selection and follow/unfollow controls.
- Publish `ProfileUpdated` through the transactional outbox as part of active production profile-write transactions.
- Emit stable owner-operation telemetry for self-service writes, event publication, and CLI backfill without logging profile copy, source email, generated handles, locale values, Media references, URLs, or provider/storage details.
- Define reusable profile DTOs and reader contracts that groups, forum, blog, social, and commerce surfaces can consume.

## Interactions

- Depends on `rustok-core` for module contracts, permission vocabulary, and typed runtime extensions.
- Uses SeaORM-backed storage and module-local migrations for profile persistence.
- Depends on `rustok-taxonomy` for shared scope-aware tags while keeping `profile_tags` module-owned. Profiles consumes Taxonomy vocabulary through its owner read/service boundaries and does not read Taxonomy persistence entities directly.
- Depends on Media owner ports for tenant-scoped asset lookup and public descriptor selection; Profiles accepts only owner-uploaded image assets and revalidates tenant/uploader/MIME before exposing the Media-selected descriptor.
- Server composition prefers a deployment-preseeded `ProfileMediaPublicImageProvider`, then an existing module extension, then the embedded owner service. It publishes the same selected wrapper to GraphQL and native server-function host contexts.
- Profiles never imports the Media gRPC adapter, endpoint configuration, object keys, or capability route construction.
- Depends on the `rustok-social-graph` owner port for directional, tenant-scoped, bounded follower checks; profiles do not read social relation tables directly.
- Sits above the platform `users` identity model and references it by `user_id`.
- Must not collapse `customer`, `seller`, or staff/admin roles into one profile record.
- Is the canonical source for public author/member cards across host applications and module-owned UI packages.
- Serves `rustok-blog` and `rustok-forum` through `ProfilesReader` with batched summary resolution.
- Serves notification recipient policy through `ProfilePrivacyReadPort` and the minimal `ProfilePrivacyService` owner adapter.
- Uses `rustok-events` + `rustok-outbox` for downstream synchronization after profile mutations and account deactivation redaction.
- Publishes operational records through the stable `rustok_profiles::operations` tracing target. Per-user writes carry tenant/user correlation; CLI backfill carries tenant scope, dry-run/event flags, aggregate counters, stage, outcome, duration, stable error code, and retryability only.
- Mounts `rustok-profiles-storefront` through the module manifest; `apps/storefront` only composes the package and does not own profile UX.

## Entry points

- `ProfilesModule`
- `ProfileService` for reads, normalization, planning, and deprecated mutation compatibility only
- `ProfileMutationService` for production profile writes
- `redact_profile_for_account_deactivation_in_tx` for caller-owned account deactivation transactions
- `ProfilesReader`
- `ProfilePrivacyService`
- `ProfilePrivacyReadPort`
- `ProfileMediaPublicImageProvider`
- `ProfileMediaSlot`
- `ProfileImagePresentation`
- `profile_image_presentation`
- `validate_profile_media_asset`
- `ProfileSummaryLoader`
- `ProfileSummaryLoaderKey`
- `PROFILE_OPERATION_TARGET`
- `PROFILE_BACKFILL_OPERATION`
- `ProfileOperation`
- `ProfileOperationTimer`
- `ProfileBackfillTimer`
- `graphql::*`
- `dto::*`
- `entities::*`
- `migrations::*`
- `storefront/` package exporting `ProfilesView`

See also `docs/README.md` and `storefront/README.md`.
