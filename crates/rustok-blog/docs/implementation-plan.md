# rustok-blog implementation plan

## Current state

`rustok-blog` owns localized posts, Blog categories, Blog tag relations,
channel-aware publication visibility, GraphQL/HTTP adapters, and admin/storefront
packages. It consumes `rustok-comments` through `CommentsThreadPort` and shared
taxonomy through its public boundary. Native `#[server]` and GraphQL remain
parallel transports over the same owner services.

The neutral `rustok-api::richtext` contract and executable
`rustok-content::richtext` profiles are now available. Blog comments are
already a typed consumer of the Comments owner: comment writes use
`RichTextDocument`, and moderation responses return `RichTextView` plus the
server-derived plain text. Blog posts remain on their separate article
cutover. The owner now has a fixed `article` profile boundary and a
canonical-document write/read projection for the Next admin contract. Do not
add new `rt_json`/Markdown aliases, `content_json` fields, or local renderers;
the Leptos/storefront and storage-schema cutover must finish atomically.

The host path limiter protects `/api/*`, including Blog REST and GraphQL. Blog
adds field-aware GraphQL classification backed by the host
`SharedApiRateLimiter`. Anonymous keys use only the host-resolved client IP.
Exceeded responses carry the same value in GraphQL `retryAfter` and HTTP
`Retry-After`; the Axum controller preserves async-graphql response headers.

Search consumes Blog lifecycle and `ReindexRequested` events without importing
the Blog crate. The projector denormalizes `category_name`, `category_slug`, and
the canonical post slug into Blog documents. Category update/delete therefore
publish `ReindexRequested { target_type: "blog", target_id: None }` in the same
owner transaction. Search table discovery follows the active PostgreSQL
`search_path`.

Search owns result navigation through `canonical_search_result_url`. Blog results
are navigable only for the canonical `source_module=blog` /
`entity_type=blog_post` pair with a bounded safe projected slug. GraphQL,
storefront native Search, Search admin preview, and admin global search delegate
to that single owner policy. Blog and Search transport packages contain no local
Blog route construction and no post-transport navigation fallback.

Blog categories use one platform permission resource: `blog_categories:*`.
`Resource::BlogCategories`, parser/display strings, permission constants,
built-in role snapshots, public-read authority, OAuth content scopes, and
storefront scopes all use that resource. Catalog `categories:*` and
`blog_posts:*` do not authorize Blog category operations.

`CategoryService` has one constructor:
`CategoryService::new(db, event_bus)`. `TransactionalEventBus` is mandatory and
cannot be omitted. Category update/delete, localized translation changes, and
Blog reindex outbox publication share one database transaction. Authorization
runs before lookup. Parent and translation reads are tenant-scoped. A category
name that cannot produce a route key requires an explicit non-empty ASCII slug.
Owner service and HTTP pagination clamp `per_page` to `1..100`. HTTP errors
preserve `404`, `403`, and `400` semantics and return a safe `500` for unexpected
infrastructure failures.

Public comments use the Comments-owned approved-only projection. Pending, spam,
trash, and deleted comments cannot cross the public boundary. Storefront native
and GraphQL paths share pagination and payloads. Admin moderation is separately
permission-gated and paginated. The Comments owner now serializes per-thread
position allocation, derives exact active comment counts under the same lock, and
enforces unique `(thread_id, position)` storage after repairing historical rows.
The separate Blog post reply-count projection continues to use a durable ledger,
optimistic version locking, retryable missing-post behavior, and transactional
outbox publication.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Load protection: `implementation_ready`; mounted Redis evidence is pending.
- Rate-limit harness: `executable_no_compile`; execution is user-owned.
- Search Blog projection harness: `executable_no_run`; PostgreSQL execution is
  user-owned.
- Comments thread write invariants: `executable_no_run`; owner hooks, repair
  migration, unique index, test, evidence, and FBA guardrail are implemented.
- Category search reindex: `source_verified_no_compile`.
- Canonical Search URL: `source_verified_no_compile`; one owner policy and no
  transport fallback.
- Blog category authority is exclusively `blog_categories:*`.
- Category writes require `CategoryService::new(db, event_bus)`.
- Category mutation and reindex publication share one transaction.
- Owner and HTTP list boundaries cap `per_page` at 100.
- Translation reads/writes and parent validation are tenant-scoped.
- Empty normalized category slugs fail before database writes.
- Category HTTP errors retain typed status semantics.
- GraphQL rate-limit exceeded responses preserve HTTP `Retry-After`.
- Comment public/admin projections remain isolated by owner contracts.
- Blog Next post forms use one shared `RichTextDocument` editor and consume
  server-rendered `RichTextView` HTML; no format selector or local post
  renderer remains in that path.

## Evidence and guardrails

- `crates/rustok-blog/contracts/blog-fba-registry.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-consumer-runtime-order-smoke.json`
- `crates/rustok-comments/contracts/evidence/comments-thread-write-invariants.json`
- `crates/rustok-blog/contracts/evidence/blog-graphql-rate-limit-runtime-harness.json`
- `crates/rustok-blog/contracts/evidence/blog-category-search-reindex-contract.json`
- `crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`
- `crates/rustok-search/contracts/evidence/search-canonical-url-contract.json`
- `scripts/verify/verify-blog-graphql-rate-limit.mjs`
- `scripts/verify/verify-blog-category-search-reindex.mjs`
- `scripts/verify/verify-blog-fba.mjs`
- `scripts/verify/verify-blog-admin-boundary.mjs`
- `scripts/verify/verify-blog-storefront-boundary.mjs`
- `scripts/verify/verify-comments-thread-write-invariants.mjs`
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
   handling, missing-post cleanup, isolated PostgreSQL harnesses, and active
   `search_path` discovery.
5. Hardened comment projection delivery with a durable ledger, optimistic
   locking, retryable ordering, and transactional outbox publication.
6. Added Comments-owned approved public reads, fail-closed provider defaults,
   transport parity, moderation parity, and bounded storefront/admin pagination.
7. Added Comments-owned serialized position allocation, exact active-row thread
   counts, historical repair, and a unique thread-position database invariant.
8. Added Search-owned canonical result URL policy and migrated GraphQL,
   storefront native, Search admin, and admin global search to that policy.
9. Removed storefront navigation post-processing and every transport-local Blog
   URL builder.
10. Added Blog category HTTP CRUD, list DTOs, OpenAPI wiring, module routes,
    transactional owner writes, Search reindex publication, tenant-scoped
    translations, and machine-readable evidence.
11. Added dedicated `blog_categories:*` authority across the platform permission
    parser, constants, OAuth groups, built-in roles, public authority, Blog owner,
    HTTP adapter, module registration, tests, evidence, and guardrails.
12. Removed alternate category permission paths and made
   `TransactionalEventBus` a required `CategoryService` constructor argument.
13. Added the Blog article richtext owner boundary: fixed `article` profile
    validation, canonical root JSON writes, and server HTML/plain-text
    projections for the Next admin post API/form.

## Next results

1. **Execute category runtime evidence.** Exercise HTTP CRUD using
   `blog_categories:*`; verify that `blog_posts:*` and catalog `categories:*` are
   denied, then retain tenant-isolation, parent, slug, typed-status, pagination,
   authorization-order, and outbox rollback evidence.
2. **Execute Search refresh evidence.** Consume category-triggered Blog reindex
   and retain changed `category_name` / `category_slug` documents for related
   posts.
3. **Execute canonical navigation evidence.** Verify Blog results through GraphQL,
   storefront native Search, Search admin preview, and admin global search; retain
   fail-closed malformed-slug and canonical click-href evidence.
4. **Execute mounted rate-limit evidence.** Run policy, memory adapter,
   controller handoff, focused verifier, then Redis-backed host requests with a
   real HTTP `Retry-After` matching GraphQL `retryAfter`.
5. **Close comments runtime evidence.** Run the Comments invariant test and real
   concurrent PostgreSQL create/delete transactions, then cover approved-only
   reads, moderation, pagination, independent create commands, duplicate event
   delivery, concurrent counters, missing-post retry, rollback, and outbox
   publication.
6. **Finish the atomic richtext cutover for Blog posts.** **Owner article
   boundary and Next admin slice implemented; storage/Leptos/storefront parity
   remains.** Replace the string body plus `content_json` transport everywhere
   with `RichTextDocument`, assign the `article` profile in the owner service,
   migrate `blog_post_translations` and relevant revision/audit data, and use
   the canonical server HTML/plain-text projections for admin, both
   storefronts, Search, AI/SEO, and the already-typed Comments integration.
   The Blog package must not own Forum editor/API code.
   **Depends on:** the
   [central Richtext plan](../../../docs/modules/rich-text-implementation-plan.md)
   and target `rustok-api`/`rustok-content` contracts.
   **Done when:** Next and Leptos save/reload/SSR match on the target-only
   contract, public comments rendering parity uses the same server projection,
   and no Blog path accepts Markdown, format aliases, or raw JSON.

## Verification

- Contract tests cover every public use case.
- `node scripts/verify/verify-blog-category-search-reindex.mjs`
- `node scripts/verify/verify-blog-category-search-reindex.test.mjs`
- Category HTTP CRUD, dedicated RBAC, required event bus, outbox rollback,
  tenant isolation, typed errors, pagination, slug, parent, and Search refresh
  integration tests.
- `cargo test -p rustok-blog --test graphql_rate_limit_policy_test`
- `cargo test -p rustok-blog graphql::rate_limit`
- `cargo test -p rustok-server graphql_http_response_preserves_extension_headers`
- `node scripts/verify/verify-blog-graphql-rate-limit.mjs`
- `cargo test -p rustok-comments --test thread_write_invariants`
- `node scripts/verify/verify-comments-thread-write-invariants.mjs`
- `cargo test -p rustok-search engine::tests::canonical_url`
- `cargo test -p rustok-search --test blog_ingestion_contract_test`
- `RUSTOK_SEARCH_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-search --test blog_projection_postgres_test`
- `cargo check -p rustok-server --features mod-blog`
- `npm run verify:blog:admin-boundary`
- `npm run verify:blog:storefront-boundary`
- `npm run verify:blog:fba`
- `npm run verify:comments:fba`
- `npm run verify:consumer:fba-runtime-order`
- `node scripts/verify/verify-search-blog-projection.mjs`
- `node scripts/verify/verify-search-blog-projection.test.mjs`
- `node scripts/verify/verify-search-canonical-url-contract.mjs`
- `node scripts/verify/verify-search-canonical-url-contract.test.mjs`
- `cargo xtask module validate blog`

## References

- [Crate README](../README.md)
- [Blog documentation](./README.md)
- [Comments consumer registry](../contracts/blog-fba-registry.json)
- [Richtext implementation plan](../../../docs/modules/rich-text-implementation-plan.md)
