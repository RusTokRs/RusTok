# rustok-blog implementation plan

## Current state

`rustok-blog` owns localized posts, categories, blog-specific tag relations,
channel-aware publication visibility, GraphQL/HTTP adapters, and admin/storefront
packages. It consumes `rustok-comments` through `CommentsThreadPort` and uses
shared taxonomy without sharing blog storage. Native `#[server]` and GraphQL
remain parallel transports; the owner packages have core/transport/UI splits.

The host-level path limiter protects every `/api/*` HTTP request, including Blog
REST routes and `/api/graphql`. Blog adds a field-aware GraphQL policy through a
Blog-owned rate-limit port backed by the host `SharedApiRateLimiter`. Anonymous
actor keys consume only the host-resolved trusted client IP; raw forwarded
headers are not interpreted inside the Blog module. An executable async-graphql
integration harness exercises the module policy and structured errors without
claiming mounted Redis or HTTP `Retry-After` evidence. The real host memory
adapter also has executable mapping tests for exceeded and disabled modes.

The search lifecycle is implemented in `rustok-search`: Blog events upsert or
delete `blog_post` search documents, and `ReindexRequested` supports both one
post and the complete Blog scope. Search owns the SQL projection and does not
depend on the Blog crate. Routing and env-gated PostgreSQL harnesses now cover
Blog lifecycle projection, payload replacement, stale-document cleanup, and
cross-tenant rebuild isolation. Table discovery follows the same active
PostgreSQL `search_path` as the projector SQL rather than hard-coding `public`.
The Rust Search storefront still provides compatibility navigation by deriving
`/modules/blog?slug=...` only from a bounded safe Blog slug when no backend URL
is present.

Public comment listing uses a Comments-owned approved-only projection. Pending,
spam, trash, and deleted comments cannot leave the owner boundary. The selected
storefront post renders and paginates the same public payload through native
`#[server]` and GraphQL transports. `commentsPage` is route-owned, bounded before
GraphQL serialization, and canonically removed for page one. Admin moderation is
a separate GraphQL slice: a current-tenant actor with `blog_posts:manage` can
inspect the non-deleted owner queue and apply approve/spam/trash transitions
without coupling the CRUD editor to that permission. The admin queue is also
paginated through bounded GraphQL variables and resets page state when the
selected post changes.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Structural shape: `core_transport_ui`.
- Load-protection status: `implementation_ready`, mounted runtime evidence pending.
- Rate-limit harness status: `executable_no_compile`; the user owns execution.
- Search Blog projection harness status: `executable_no_run`; PostgreSQL execution
  remains user-owned.
- REST protection is host-owned; Blog does not instantiate a second limiter or
  duplicate the `/api/*` middleware counter.
- GraphQL protection is split into a Blog-owned policy/port and a host adapter
  over the configured memory/Redis API limiter.
- The integration harness covers allowed reads, exceeded reads, backend failure,
  authenticated write identity, unauthorized-write bypass, trusted client IP,
  structured GraphQL extensions, document-wide fail-closed accounting, and the
  `moderate_comment` manage surface.
- Mutation gates are aligned: update uses `blog_posts:update`; publish,
  unpublish, and archive use `blog_posts:publish`; comment moderation uses
  `blog_posts:manage`.
- The comments consumer contract is `CommentsThreadPort` /
  `comments.thread.v1`. Public list reads use
  `list_public_comments_for_target`; writes carry operation-scoped idempotency
  keys, deadline, locale, actor claims, and typed port-error mapping.
- `GqlPost.publicComments` and native `BlogPostDetail.publicComments` consume the
  same owner-approved projection and fixed page size. The storefront route query
  controls the requested page for both transport paths.
- `GqlPost.moderationComments` is tenant-bound and permission-gated. After the
  Blog manage decision, it performs the trusted owner read used by the existing
  REST moderation adapter; `moderateComment` uses the same domain service and is
  represented as a dedicated field-aware rate-limit surface.
- Admin and storefront comment pagination share bounded inputs, total-page
  calculation, disabled invalid navigation, and isolated transport failures.
- Search Blog-result navigation runs after Rust storefront transport selection,
  requires `source_module=blog` and `entity_type=blog_post`, validates the
  projected slug, preserves backend URLs, and fails closed for malformed data.
- Search projection table discovery, source reads, and destination writes now
  share one connection `search_path`; a focused verifier rejects a return to
  `public.blog_*` table probes.
- `BlogCommentProjectionHandler` consumes `comment.created` and
  `comment.deleted`, records a durable event-id delivery ledger, updates the
  Blog-owned reply count with optimistic version locking, and publishes
  `BlogPostUpdated` in the same transaction. Missing posts fail delivery so the
  event runtime can retry instead of acknowledging an out-of-order event.
- Evidence: `crates/rustok-blog/contracts/blog-fba-registry.json`,
  `crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`,
  `crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`,
  `crates/rustok-blog/contracts/evidence/blog-comments-consumer-runtime-order-smoke.json`,
  `crates/rustok-blog/contracts/evidence/blog-graphql-rate-limit-runtime-harness.json`,
  `crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`,
  `crates/rustok-blog/tests/graphql_rate_limit_policy_test.rs`,
  `crates/rustok-search/tests/blog_ingestion_contract_test.rs`,
  `crates/rustok-search/tests/blog_projection_postgres_test.rs`,
  `scripts/verify/verify-blog-fba.mjs`,
  `scripts/verify/verify-blog-admin-boundary.mjs`,
  `scripts/verify/verify-blog-storefront-boundary.mjs`,
  `scripts/verify/verify-search-blog-navigation.mjs`, and
  `scripts/verify/verify-search-blog-projection.mjs`.

## Completed implementation slices

1. Reconciled load protection with host composition and avoided a duplicate
   Blog REST limiter.
2. Added Blog GraphQL document classification and rate-limit enforcement for
   public reads and post mutations, including fragments and multi-operation
   documents.
3. Added the host adapter, schema injection, structured GraphQL errors, metrics,
   and host-trusted client-IP propagation.
4. Aligned REST, GraphQL, domain, and rate-limit mutation permission gates.
5. Added Blog post search projection for create, update, publish, unpublish,
   archive, delete, targeted reindex, and full Blog-scope rebuild.
6. Hardened comment projection delivery with a durable ledger, optimistic
   locking, retryable missing-post behavior, and transactional outbox
   publication.
7. Isolated comment write idempotency keys by operation and command.
8. Added a Comments-owned approved-only public thread projection, bounded public
   pagination, a fail-closed remote-adapter default, and matching provider /
   consumer FBA registry evidence.
9. Added selected-post public comments parity: a nested GraphQL complex field,
   native owner read, shared storefront DTO, Leptos rendering, English/Russian
   copy, and a guardrail that requires approved-only parity in both transports.
10. Added admin moderation parity: tenant-bound `moderationComments`, typed
    `moderateComment`, manage permission and rate-limit gates, a separate admin
    transport adapter, selected-post approve/spam/trash UI, localized copy, and
    canonical/negative boundary fixtures.
11. Added admin moderation pagination: bounded GraphQL variables, page reset on
    post selection, total-page calculation, disabled invalid navigation, and
    localized previous/next/page controls.
12. Added Rust Search storefront Blog navigation: transport-neutral payload
    enrichment, canonical module route, backend-URL precedence, strict slug
    validation, unit tests, and focused verifier fixtures.
13. Added storefront comment pagination: framework-free `commentsPage` policy,
    bounded route parsing, shared native/GraphQL page arguments, canonical page
    one URL behavior, localized controls, and pagination boundary fixtures.
14. Added executable GraphQL rate-limit policy and host-memory-adapter harnesses
    with machine-readable evidence for allowed, exceeded, backend-unavailable,
    identity, RBAC bypass, trusted-IP, moderation, and document-wide behavior.
15. Added Search Blog ingestion routing and isolated-schema PostgreSQL lifecycle
    harnesses, removed the hard-coded `public` source-table probe, and locked the
    schema contract with focused verifier fixtures.

## Next results

1. **Close mounted rate-limit runtime evidence.** Execute the new integration
   harnesses, then exercise Redis-backed host composition, GraphQL extensions,
   HTTP `Retry-After`, and publication/channel/RBAC non-regression.
2. **Close search runtime evidence.** Execute the routing/PostgreSQL/verifier
   targets, then add targeted missing-post recovery and module-disabled cleanup
   against PostgreSQL.
3. **Close comments owner/projection runtime evidence.** Exercise approved-only
   public reads, public/admin pagination, moderation queue/status changes,
   independent create commands on one post, duplicate delivery, concurrent
   count updates, missing-post retry, delivery-ledger rollback, and outbox
   publication.
4. **Promote canonical Search URL ownership.** Move the Blog route fallback from
   Rust storefront post-processing into the shared Search result contract when
   the large GraphQL projection can be changed atomically; keep compatibility
   post-processing until all consumers use the backend URL.

## Verification

- `cargo test -p rustok-blog --test graphql_rate_limit_policy_test`
- `cargo test -p rustok-blog graphql::rate_limit`
- `cargo test -p rustok-search --test blog_ingestion_contract_test`
- `RUSTOK_SEARCH_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-search --test blog_projection_postgres_test`
- `cargo check -p rustok-server --features mod-blog`
- `npm run verify:blog:admin-boundary`
- `npm run verify:blog:storefront-boundary`
- `npm run verify:blog:fba`
- `npm run verify:comments:fba`
- `npm run verify:consumer:fba-runtime-order`
- `node scripts/verify/verify-search-blog-navigation.mjs`
- `node scripts/verify/verify-search-blog-projection.mjs`
- `node scripts/verify/verify-search-blog-projection.test.mjs`
- `cargo xtask module validate blog`
- Targeted PostgreSQL lifecycle, channel visibility, comments, indexing,
  navigation, pagination, and rate-limit integration tests.

## References

- [Crate README](../README.md)
- [Blog documentation](./README.md)
- [Comments consumer registry](../contracts/blog-fba-registry.json)
