# rustok-blog implementation plan

## Current state

`rustok-blog` owns localized posts, categories, blog-specific tag relations,
channel-aware publication visibility, GraphQL/HTTP adapters, and admin/storefront
packages. It consumes `rustok-comments` through `CommentsThreadPort` and uses
shared taxonomy without sharing blog storage. Native `#[server]` and GraphQL
remain parallel transports; the owner packages have core/transport/UI splits.

The host-level path limiter already protects every `/api/*` HTTP request,
including Blog REST routes and the `/api/graphql` transport. Blog now adds a
field-aware GraphQL policy on top of that transport boundary. The policy uses
the host-owned `SharedApiRateLimiter` through a Blog-owned trait, protects
`post`, `postBySlug`, `posts` and all Blog post mutations, scopes keys by tenant
plus actor/IP, preserves resolver authentication/RBAC errors for unauthorized
writes, and returns observable structured GraphQL errors when a limit or backend
failure is encountered.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Structural shape: `core_transport_ui`.
- Load-protection status: `implementation_ready`, runtime evidence pending.
- REST protection is host-owned; Blog does not instantiate a second limiter or
  duplicate the `/api/*` middleware counter.
- GraphQL protection is split into a Blog-owned policy/port and a host adapter
  over the configured memory/Redis API limiter.
- The comments consumer contract is `CommentsThreadPort` /
  `comments.thread.v1`. Its declared degraded behavior remains source-locked,
  not live-runtime proven.
- All Blog comment lifecycle operations invoke `CommentsThreadPort` with
  authenticated actor claims, locale, deadline, idempotency where required,
  and typed port-error mapping. Blog has no direct `CommentsService` calls.
- `BlogCommentProjectionHandler` consumes `comment.created` and
  `comment.deleted`, records a durable event-id delivery ledger, updates the
  Blog-owned reply count, and publishes `BlogPostUpdated` in one transaction.
  The replacement is governed by
  `DECISIONS/2026-07-16-comments-blog-event-projection.md`.
- Evidence: `crates/rustok-blog/contracts/blog-fba-registry.json`,
  `crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`,
  `crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`,
  `crates/rustok-blog/contracts/evidence/blog-comments-consumer-runtime-order-smoke.json`,
  `scripts/verify/verify-blog-fba.mjs`,
  `scripts/verify/verify-blog-admin-boundary.mjs`, and
  `scripts/verify/verify-blog-storefront-boundary.mjs`.

## Completed in the current slice

1. Reconciled the plan with the actual host composition: Blog REST was already
   covered by the global `/api/*` limiter, so no module-local duplicate limiter
   was introduced.
2. Added a Blog-owned GraphQL rate-limit contract and document policy covering
   public reads and authenticated post mutations, including fragment/batched
   document classification.
3. Added the server adapter from `SharedApiRateLimiter` to the Blog contract and
   injected the policy into schema composition only when `mod-blog` is compiled.
4. Added tenant/actor/IP key tests, field-classification tests, permission-gate
   tests, generic rate-limit metrics, and structured `BLOG_RATE_LIMITED` /
   `BLOG_RATE_LIMIT_BACKEND_UNAVAILABLE` errors.

## Next results

1. **Close rate-limit runtime evidence.** Add memory and Redis integration tests
   for allowed/exceeded/backend-unavailable outcomes, verify GraphQL error
   extensions and HTTP `Retry-After`, and record publication/channel/RBAC
   non-regression evidence. Reuse the host's trusted client-IP resolution rather
   than allowing module code to become a second proxy-trust authority.
2. **Align Blog mutation permissions.** `updatePost` currently asks for
   `blog_posts:publish` while REST update and the domain responsibility require
   `blog_posts:update`. Correct the resolver and lock equivalent REST/GraphQL
   authorization behavior in tests.
3. **Verify the blog search projection.** Prove every published, updated,
   unpublished, archived, and deleted post event maps to the intended
   `rustok-index` document lifecycle without moving index logic into Blog. Done
   when an event-to-index integration test and recovery behavior are recorded.
4. **Execute owner-boundary and event-projection evidence end to end.** Run the
   comments consumer and Blog projection against an available runtime, including
   duplicate delivery, missing-post behavior, outbox publication, retry, and the
   next admin/storefront host parity slice. Preserve native `#[server]` plus
   GraphQL paths.

## Verification

- `cargo test -p rustok-blog graphql::rate_limit`
- `cargo check -p rustok-server --features mod-blog`
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
