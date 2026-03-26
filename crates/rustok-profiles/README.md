# rustok-profiles

## Purpose

`rustok-profiles` owns the universal public profile domain for RusToK.

## Responsibilities

- Provide a single profile boundary for any authenticated platform user.
- Keep public profile data separate from auth identity, commerce customers, and future seller accounts.
- Own profile storage (`profiles`, `profile_translations`), migrations, and the reusable profile service contract.
- Expose module-owned GraphQL transport for self-service and public profile lookups.
- Define the reusable profile DTOs and reader contract that groups, forum, blog, social, and commerce surfaces can consume.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Uses SeaORM-backed storage and module-local migrations for profile persistence.
- Sits above the platform `users` identity model and references it by `user_id`.
- Must not collapse `customer`, `seller`, or staff/admin roles into one profile record.
- Is intended to become the canonical source for public author/member cards across host applications and module-owned UI packages.

## Entry points

- `ProfilesModule`
- `ProfileService`
- `ProfilesReader`
- `graphql::*`
- `dto::*`
- `entities::*`
- `migrations::*`

See also `docs/README.md`.
