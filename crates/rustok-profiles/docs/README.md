# `rustok-profiles` Documentation

`rustok-profiles` — domain module for the unified public user profile
in RusToK. It defines the profile boundary over the platform `users`,
without mixing auth identity, customer and future seller/merchant surfaces.

## Purpose

- publish the canonical profile runtime contract for public profile and author/member summary;
- keep storage, service layer and transport boundary of profiles inside a separate module;
- provide downstream modules with a single source for author/member presentation without a direct dependency on `users`.

## Scope

- profile aggregate: `profiles`, `profile_translations`, `profile_tags`;
- `ProfileService`, `ProfilesReader`, `ProfileSummary` and related DTO/enum contracts;
- public handle, display name, bio, avatar/banner references, locale and visibility policy;
- GraphQL read/write surfaces for public profile lookup and self-service edit path;
- event contract `profile.updated` and backfill path for existing users.

## Integration

- `users` remains the identity/security boundary and does not become the public profile source;
- `rustok-customer` remains a separate commerce-domain profile with optional linkage to `user_id`;
- `rustok-blog` and `rustok-forum` already use `ProfilesReader` for author presentation;
- `rustok-taxonomy` provides a shared dictionary for `profile_tags`, but ownership of the bindings remains with the profiles module.

## Verification

- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- targeted tests for handle policy, locale fallback, summary batching, GraphQL self-service path and profile backfill

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Platform documentation map](../../../docs/index.md)
