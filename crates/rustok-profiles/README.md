# rustok-profiles

## Purpose

`rustok-profiles` owns the universal public profile domain for RusToK.

## Responsibilities

- Provide a single profile boundary for any authenticated platform user.
- Keep public profile data separate from auth identity, commerce customers, and future seller accounts.
- Own profile storage (`profiles`, `profile_translations`), migrations, and the reusable profile service contract.
- Own profile-to-taxonomy relation storage via `profile_tags`.
- Provide batched profile summary lookup for downstream author/member presentation without per-user fan-out.
- Provide explicit backfill helpers for provisioning missing profiles from existing user/customer data.
- Expose a request-scoped GraphQL `ProfileSummaryLoader` for host applications that need DataLoader-based batching and caching.
- Expose module-owned GraphQL transport for self-service and public profile lookups, including targeted profile update mutations.
- Publish `profile.updated` through the transactional outbox after successful profile writes.
- Define the reusable profile DTOs and reader contract that groups, forum, blog, social, and commerce surfaces can consume.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Uses SeaORM-backed storage and module-local migrations for profile persistence.
- Depends on `rustok-taxonomy` for shared scope-aware tags while keeping `profile_tags`
  module-owned.
- Sits above the platform `users` identity model and references it by `user_id`.
- Must not collapse `customer`, `seller`, or staff/admin roles into one profile record.
- Is intended to become the canonical source for public author/member cards across host applications and module-owned UI packages.
- Already serves `rustok-blog` and `rustok-forum` through `ProfilesReader` with batched summary resolution.
- Uses `rustok-events` + `rustok-outbox` for downstream synchronization after profile mutations.

## Entry points

- `ProfilesModule`
- `ProfileService`
- `ProfilesReader`
- `ProfileSummaryLoader`
- `ProfileSummaryLoaderKey`
- `graphql::*`
- `dto::*`
- `entities::*`
- `migrations::*`

See also `docs/README.md`.
