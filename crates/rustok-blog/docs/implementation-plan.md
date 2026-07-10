# rustok-blog implementation plan

## Current state

`rustok-blog` owns localized posts, categories, blog-specific tag relations,
channel-aware publication visibility, GraphQL/HTTP adapters, and admin/storefront
packages. It consumes `rustok-comments` through `CommentsThreadPort` and uses
shared taxonomy without sharing blog storage. Native `#[server]` and GraphQL
remain parallel transports; the owner packages have core/transport/UI splits.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `in_progress` (`core_transport_ui`).
- The comments consumer contract is `CommentsThreadPort` /
  `comments.thread.v1`. Its declared degraded behavior remains source-locked,
  not live-runtime proven.
- Evidence: `crates/rustok-blog/contracts/blog-fba-registry.json`,
  `crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`,
  `crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`,
  `crates/rustok-blog/contracts/evidence/blog-comments-consumer-runtime-order-smoke.json`,
  `scripts/verify/verify-blog-fba.mjs`,
  `scripts/verify/verify-blog-admin-boundary.mjs`, and
  `scripts/verify/verify-blog-storefront-boundary.mjs`.

## Next results

1. **Protect public and write paths under load.** Apply the platform
   `RateLimiter` to public REST/GraphQL reads and authenticated write/moderation
   operations, with tenant/actor/IP keys and observable rejection behavior.
   Done when integration tests establish limits without weakening publication,
   channel, or RBAC checks.
2. **Verify the blog search projection.** Prove every published, updated,
   unpublished, archived, and deleted post event maps to the intended
   `rustok-index` document lifecycle without moving index logic into blog. Done
   when an event-to-index integration test and recovery behavior are recorded.
3. **Execute owner-boundary evidence end to end.** Run the comments consumer
   contract against an available runtime and complete the next admin/storefront
   host parity slice, preserving native `#[server]` plus GraphQL paths. Done
   when comments fallback/error mapping and equivalent authenticated/public UI
   outcomes are observed outside source-only checks.

## Verification

- `npm run verify:blog:admin-boundary`
- `npm run verify:blog:storefront-boundary`
- `npm run verify:blog:fba`
- `npm run verify:consumer:fba-runtime-order`
- `cargo xtask module validate blog`
- Targeted lifecycle, channel visibility, comments, indexing, and rate-limit
  integration tests.

## References

- [Crate README](../README.md)
- [Blog documentation](./README.md)
- [Comments consumer registry](../contracts/blog-fba-registry.json)
