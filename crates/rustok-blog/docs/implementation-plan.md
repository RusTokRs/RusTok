# rustok-blog implementation plan

## Current state

`rustok-blog` owns localized posts, module-owned categories, Blog tag relations,
channel-aware publication visibility, GraphQL/HTTP adapters, and admin/storefront
packages. It consumes `rustok-comments` through `CommentsThreadPort` and shared
taxonomy through its public boundary. Native `#[server]` and GraphQL remain
parallel transports; the owner packages have core/transport/UI splits.

The host path limiter protects `/api/*`, including Blog REST and GraphQL. Blog
adds a field-aware GraphQL policy backed by the host `SharedApiRateLimiter`.
Anonymous keys use only the host-resolved trusted client IP. Exceeded responses
carry the same value in GraphQL `retryAfter` and HTTP `Retry-After`; the Axum
GraphQL controller preserves async-graphql response headers. Backend-unavailable
and unauthorized-write responses remain headerless. Source harnesses are
present, while mounted Redis execution remains user-owned.

Search consumes Blog lifecycle and `ReindexRequested` events without importing
the Blog crate. The projector denormalizes `category_name` and `category_slug`
into Blog documents, so category update/delete publish tenant Blog-scope reindex
requests. Search table discovery follows the active PostgreSQL `search_path`.
Routing and env-gated PostgreSQL harnesses cover lifecycle, stale cleanup,
module toggles, missing-post cleanup, and tenant isolation.

Blog category create/update/delete now use owner transactions. Update/delete
publish `ReindexRequested { target_type: "blog", target_id: None }` through the
same outbox transaction. Production HTTP CRUD constructs the event-aware
service from `HostRuntimeContext`; a compatibility constructor remains for old
in-process callers. Translation and parent lookups are tenant-scoped. Category
names that normalize to no route characters require an explicit non-empty ASCII slug.
Both service and HTTP pagination clamp `per_page` to 1..100. HTTP errors preserve
404/403/400 semantics and return a safe 500 for unexpected infrastructure
failures.

The category permission boundary is migration-safe. `blog_posts:*` is the
primary Blog capability namespace because categories are part of the Blog
aggregate. Legacy `categories:*` claims are temporarily accepted by owner and
HTTP preflight so existing tokens keep working, but `BlogModule::permissions()`
no longer advertises or seeds catalog category permissions. Categories do not
have a user owner, so update/delete use resource scope rather than comparing a
user UUID with a category UUID. A future dedicated `blog_categories:*` resource
must be introduced only together with permission parsing, OAuth scope groups,
default-role snapshots, persistence migration, and compatibility evidence.

Canonical result navigation belongs to Search.
`canonical_search_result_url` derives product, content, and Blog URLs before
GraphQL and storefront-native serialization. Blog URLs require the canonical
Blog source/entity pair and a bounded safe owner-projected slug. Storefront
post-processing remains only as an idempotent rolling-compatibility fallback.
The admin native mapper is the final transport-local URL switch.

Public comments use the Comments-owned approved-only projection. Pending, spam,
trash, and deleted comments cannot cross the public boundary. Storefront native
and GraphQL paths share pagination and payloads. Admin moderation is separately
permission-gated and paginated. Comment counter projection uses a durable
ledger, optimistic version locking, retryable missing-post behavior, and
transactional outbox publication.

## FFA/FBA status

- FFA: `in_progress`.
- FBA: `boundary_ready` (`core_transport_ui`).
- Load protection: `implementation_ready`; mounted Redis evidence pending.
- Rate-limit harness: `executable_no_compile`; execution is user-owned.
- Search Blog projection harness: `executable_no_run`; PostgreSQL execution is
  user-owned.
- Category search reindex: `source_verified_no_compile`.
- Canonical Search URL: `source_verified_no_compile`; admin native cutover and
  compatibility-fallback removal remain.
- Category production writes use `CategoryService::new_with_event_bus`.
- Category update/delete and Blog reindex outbox publication share one
  transaction.
- `blog_posts:*` is primary for Blog categories; `categories:*` is legacy-only.
- `BlogModule` does not register generic catalog category permissions.
- Owner and HTTP list boundaries both cap `per_page` at 100.
- Translation reads/writes and parent validation are tenant-scoped.
- Empty normalized category slugs fail before database writes.
- Category HTTP errors retain typed status semantics.
- GraphQL rate-limit exceeded responses preserve HTTP `Retry-After`.
- Search GraphQL/storefront-native projections use the shared URL policy.
- Comment public/admin projections remain isolated by owner contracts.

## Evidence and guardrails

- `crates/rustok-blog/contracts/blog-fba-registry.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-consumer-runtime-order-smoke.json`
- `crates/rustok-blog/contracts/evidence/blog-graphql-rate-limit-runtime-harness.json`
- `crates/rustok-blog/contracts/evidence/blog-category-search-reindex-contract.json`
- `crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`
- `crates/rustok-search/contracts/evidence/search-canonical-url-contract.json`
- `scripts/verify/verify-blog-graphql-rate-limit.mjs`
- `scripts/verify/verify-blog-category-search-reindex.mjs`
- `scripts/verify/verify-blog-fba.mjs`
- `scripts/verify/verify-blog-admin-boundary.mjs`
- `scripts/verify/verify-blog-storefront-boundary.mjs`
- `scripts/verify/verify-search-blog-navigation.mjs`
- `scripts/verify/verify-search-blog-projection.mjs`
- `scripts/verify/verify-search-canonical-url-contract.mjs`

## Completed implementation slices

1. Reconciled Blog load protection with host composition and avoided a duplicate
   REST limiter.
2. Added field-aware GraphQL classification, structured rate-limit errors,
   metrics, host adapter wiring, trusted-IP identity, and matching
   `Retry-After` HTTP handoff.
3. Aligned post mutation permissions across REST, GraphQL, domain, and limiter.
4. Added Blog lifecycle Search projection, targeted/full reindex, module-toggle
   handling, missing-post cleanup, isolated PostgreSQL harnesses, and schema
   discovery through the active `search_path`.
5. Hardened comment projection delivery with a durable ledger, optimistic
   locking, retryable ordering, and transactional outbox publication.
6. Added Comments-owned approved public reads, fail-closed provider defaults,
   transport parity, moderation parity, and bounded storefront/admin pagination.
7. Added Search-owned canonical result URLs and migrated GraphQL plus
   storefront-native mappings while retaining an idempotent compatibility
   fallback.
8. Added Blog category HTTP CRUD, list DTOs, OpenAPI wiring, module routes,
   transactional owner writes, Search reindex publication, tenant-scoped
   translations, and machine-readable evidence.
9. Hardened category owner invariants: tenant-safe parents, non-empty slugs,
   service and HTTP pagination caps, typed HTTP errors, primary Blog permission
   scope, legacy permission fallback, and removal of catalog permission
   advertisement from `BlogModule`.

## Next results

1. **Execute category runtime evidence.** Exercise HTTP CRUD using primary
   `blog_posts:*` and legacy `categories:*` claims; verify tenant isolation,
   parent validation, slug rejection, typed statuses, service/HTTP caps,
   outbox rollback, and one committed reindex event for update/delete.
2. **Execute Search refresh evidence.** Consume category-triggered Blog reindex
   and retain changed `category_name` / `category_slug` documents for related
   posts.
3. **Execute mounted rate-limit evidence.** Run policy, memory adapter,
   controller handoff, focused verifier, then Redis-backed host requests with a
   real HTTP `Retry-After` matching GraphQL `retryAfter`.
4. **Finish admin native URL cutover.** Migrate the final Search admin mapper to
   `canonical_search_result_url`; remove storefront compatibility enrichment
   only after all consumers prove backend URL adoption.
5. **Close comments runtime evidence.** Cover approved-only reads, moderation,
   pagination, independent create commands, duplicate delivery, concurrent
   counters, missing-post retry, rollback, and outbox publication.
6. **Plan dedicated category permission resource.** Introduce
   `blog_categories:*` only as a platform-wide migration covering parser,
   OAuth groups, role snapshots, persistence, seeded grants, and compatibility.

## Verification

- `node scripts/verify/verify-blog-category-search-reindex.mjs`
- `node scripts/verify/verify-blog-category-search-reindex.test.mjs`
- Category HTTP CRUD, primary/legacy RBAC, outbox rollback, tenant isolation,
  typed errors, pagination, slug, parent, and Search refresh integration tests.
- `cargo test -p rustok-blog --test graphql_rate_limit_policy_test`
- `cargo test -p rustok-blog graphql::rate_limit`
- `cargo test -p rustok-server graphql_http_response_preserves_extension_headers`
- `node scripts/verify/verify-blog-graphql-rate-limit.mjs`
- `cargo test -p rustok-search engine::tests::canonical_url`
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
- `node scripts/verify/verify-search-canonical-url-contract.mjs`
- `node scripts/verify/verify-search-canonical-url-contract.test.mjs`
- `cargo xtask module validate blog`

## References

- [Crate README](../README.md)
- [Blog documentation](./README.md)
- [Comments consumer registry](../contracts/blog-fba-registry.json)
