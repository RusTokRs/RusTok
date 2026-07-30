# rustok-blog implementation plan

## Current state

`rustok-blog` owns localized posts, Blog categories, Blog tag relations,
channel-aware publication visibility, GraphQL/HTTP adapters, and admin/storefront
packages. It consumes `rustok-comments` through `CommentsThreadPort` and shared
taxonomy through its public boundary. Native `#[server]` and GraphQL remain
parallel transports over the same owner services.

The neutral `rustok-api::richtext` contract and executable
`rustok-content::richtext` profiles are available. Blog comments are a typed
consumer of the Comments owner: comment writes use `RichTextDocument`, and
moderation responses return `RichTextView` plus server-derived plain text. The
Blog article source cutover is complete: owner and GraphQL writes accept only
the fixed `article` document, reads expose `RichTextView` plus derived text, and
`body`, `body_format`, `content_json`, Markdown aliases, and raw JSON transport
paths are absent from production DTOs. The GraphQL mutation layer delegates
typed `input.into()` values and the recursive guardrail requires the removed
fields to remain absent.

The owner-specific offline backfill at
`crates/rustok-blog/src/bin/blog_article_richtext_backfill.rs` now closes the
pre-migration operational gap. It is dry-run by default, scans the real Blog
owner tables, emits a content-free NDJSON report, requires `--apply` for writes,
requires a separate `--allow-markdown-plain-text` acknowledgement for lossy
historical Markdown handling, uses optimistic updates, and verifies the result
again. It neither executes the irreversible migration nor triggers Search
reindex; those remain explicit operator steps.

The Next admin Forum reply composer is no longer owned by Blog. Forum navigation,
GraphQL helpers, the reply editor, and its contained `rt_json_v1` compatibility
adapter now live under `apps/next-admin/packages/forum/src`; the host registers
that package independently. Blog and Forum consume the same thin shared React
lifecycle adapter at `apps/next-admin/src/shared/ui/rich-text-editor.tsx`, while
profile selection remains owner-specific (`article` versus `discussion`).

The Blog admin FFA guardrail now matches the canonical article editor. It
requires typed `RichTextDocument` state and the owner `BlogRichTextEditor`,
rejects reintroduction of body-format selectors and raw-body warnings in both
core and Leptos UI, and validates machine evidence plus self-regression
fixtures. The guardrail is part of the Blog FBA command chain.

The Blog storefront selected-post path now consumes the owner read projection
across both transports. GraphQL requests `content { document html }` plus
`contentPlainText`; native SSR maps `PostResponse.content` and
`content_plain_text`; Leptos renders only server-rendered `RichTextView` HTML and
uses server-derived plain text when the projection is absent. The storefront DTO
and active UI path no longer expose or interpret `body` / `bodyFormat`. Blog SEO
also consumes `content_plain_text`. Search canonical richtext rows are parsed as
`RichTextDocument` and projected with `rustok-content::plain_text` under the fixed
`Article` profile in the same projector transaction; legacy storage rows retain a
owner-derived plain text. The target-only source cutover is implemented:
`blog_post_translations.body` stores canonical Article documents, the migration
validates every row before dropping `body_format`, GraphQL exposes only typed
content, Search has no raw-body fallback, AI constructs owner documents directly,
and storefront summarizers are removed. Forum-to-Blog orchestration fails closed
unless the source is canonical Article-compatible richtext. Compilation,
migration execution, PostgreSQL evidence, and browser parity remain user-owned.
Do not add format aliases, raw JSON fields, selectors, or local renderers.

The host GraphQL composition binds `rustok-profiles::ProfileSummaryLoader` to
the current request audience. Existing Blog post/list author batches therefore
apply the Profiles owner visibility matrix before localized profile/tag loading.
Anonymous requests receive only active public authors, authenticated requests
also receive active authenticated authors and their own private profile, and
service principals never claim profile ownership. Restricted, hidden, blocked,
missing, and cross-tenant summaries remain absent without per-author reads or a
Blog GraphQL schema change.

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
`search_path`. Canonical article bodies are never indexed as raw JSON: their
search text is derived by the shared richtext policy, while invalid canonical
content fails the transaction instead of committing a partial projection.

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
- Blog article richtext cutover: `implemented_source_verified_no_compile`;
  target-only owner/storage/GraphQL/Search/AI/storefront source is implemented,
  the irreversible migration is fail-closed, and execution is user-owned.
- Blog article offline backfill: `executable_no_run`; dry-run preflight,
  content-free reporting, explicit apply/Markdown acknowledgement, orphan
  detection, stable cursoring, optimistic writes, and post-apply verification
  are implemented.
- Next admin Forum UI ownership: `source_verified_no_compile`; Blog no longer
  registers or exports Forum navigation, GraphQL helpers, reply UI, or legacy
  format adapters, and both owners use the shared richtext lifecycle adapter.
- Blog admin canonical richtext guardrail: `source_verified_no_compile`; the
  FFA verifier requires typed document/editor state, rejects removed selector
  and raw-body helpers, validates machine evidence, and has negative fixtures.
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
- Blog GraphQL author cards use the request-scoped Profiles privacy loader.
- Blog Next post forms use one shared `RichTextDocument` editor and consume
  server-rendered `RichTextView` HTML; no format selector or local post
  renderer remains in that path.
- Blog storefront selected posts consume the same server-rendered `RichTextView`
  HTML and server-derived plain text through GraphQL and native SSR;
  `source_verified_no_compile`, execution is user-owned.
- Blog SEO projection consumes server-derived plain text and no longer reads the
  legacy post body.
- Blog GraphQL richtext boundary: `source_verified_no_compile`; canonical-only
  create/update fields, typed conversion, resolver delegation, recursive absence
  guards, verifier, self-test, and npm/FBA registration are implemented.
- AI Blog draft owner writes: `source_verified_no_compile`; generated create/update
  text is converted directly to `RichTextDocument`, existing source content uses
  server-derived plain text, and no Markdown-shaped compatibility adapter remains.

## Evidence and guardrails

- `crates/rustok-blog/contracts/blog-fba-registry.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-consumer-runtime-order-smoke.json`
- `crates/rustok-comments/contracts/evidence/comments-thread-write-invariants.json`
- `crates/rustok-blog/contracts/evidence/blog-graphql-rate-limit-runtime-harness.json`
- `crates/rustok-blog/contracts/evidence/blog-category-search-reindex-contract.json`
- `crates/rustok-blog/contracts/evidence/blog-graphql-richtext-boundary.json`
- `crates/rustok-blog/contracts/evidence/blog-storefront-richtext-view.json`
- `crates/rustok-blog/contracts/evidence/blog-richtext-cutover-inventory.json`
- `crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json`
- `crates/rustok-blog/contracts/evidence/blog-forum-ui-ownership.json`
- `crates/rustok-blog/contracts/evidence/blog-admin-richtext-boundary.json`
- `crates/rustok-blog/docs/richtext-cutover-inventory.md`
- `crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`
- `crates/rustok-search/contracts/evidence/search-canonical-url-contract.json`
- `scripts/verify/verify-blog-graphql-rate-limit.mjs`
- `scripts/verify/verify-blog-category-search-reindex.mjs`
- `scripts/verify/verify-blog-graphql-richtext-boundary.mjs`
- `scripts/verify/verify-blog-graphql-richtext-boundary.test.mjs`
- `scripts/verify/verify-blog-richtext-offline-backfill.mjs`
- `scripts/verify/verify-blog-forum-ui-ownership.mjs`
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
14. Bound GraphQL post/list `authorProfile` batches to the request audience through
    the Profiles owner privacy loader, preserving one base-row privacy query per
    batch and omitting restricted summaries before localized profile/tag reads.
15. Added a Blog GraphQL richtext containment boundary: machine-readable evidence,
    an executable canonical-field/legacy-alias guardrail, self-regression coverage,
    named npm commands, and inclusion in the Blog FBA verification chain.
16. Narrowed the GraphQL legacy allowance from all of `mutation.rs` to the exact
    `UpdatePostInput` compatibility conversion, while keeping every resolver and
    production helper under recursive leak detection.
17. Moved the update compatibility conversion and its regression test beside the
    GraphQL input types, removed all legacy richtext access from `mutation.rs`,
    and tightened the guardrail to one adapter/conversion owner file.
18. Added symmetric create/update GraphQL input conversion regression coverage:
    canonical create payload preservation, temporary legacy defaults, exact
    mapping checks, and ownership guards that keep both conversions out of
    `mutation.rs`.
19. Migrated the Blog storefront selected-post read path to the owner projection:
    GraphQL and native SSR now return `RichTextView` plus server-derived plain
    text, Leptos renders owner HTML with a plain-text fallback, and the active
    storefront DTO/UI path no longer accepts `body` or `bodyFormat`.
20. Added the Blog owner plain-text article import adapter with canonical paragraph
    semantics and regression coverage, without accepting Markdown aliases, raw
    JSON, HTML, or caller-selected profiles.
21. Migrated canonical Blog Search rows to the shared `Article` plain-text policy
    in the same projector transaction, retained a contained legacy storage
    fallback, and made invalid canonical documents fail closed before commit.
22. Added a `rustok-ai` Blog owner adapter that converts AI draft create/update
    text into canonical `RichTextDocument` content before owner service calls,
    prefers server-derived plain text for existing-post source material, and
    prevents direct-task compatibility fields from reaching the owner unchanged.
23. Completed the target-only Blog article source cutover: added a fail-closed
    irreversible storage migration, removed owner/GraphQL/AI/Search compatibility,
    deleted storefront summarizers, and guarded Forum-to-Blog orchestration.
24. Added the owner-specific Blog article offline backfill: dry-run-first scanning
    of current owner tables, content-free NDJSON reporting, explicit apply and
    Markdown acknowledgement, fail-closed format/profile validation, optimistic
    batch writes, and post-apply verification.
25. Removed Forum Next admin ownership from the Blog package, introduced the
    Forum-owned package registration/navigation/API/editor boundary, and moved
    the reusable React richtext lifecycle adapter to the host shared UI layer.
26. Reconciled the Blog admin FFA guardrail with the canonical article editor:
    removed stale required legacy helpers, added typed editor/document evidence,
    negative regression fixtures, and FBA-chain execution.

## Next results

1. **Execute category runtime evidence.** Exercise HTTP CRUD using
   `blog_categories:*`; verify that `blog_posts:*` and catalog `categories:*` are
   denied, then retain tenant-isolation, parent, slug, typed-status, pagination,
   authorization-order, and outbox rollback evidence.
2. **Execute Search refresh evidence.** Consume category-triggered Blog reindex
   and retain changed `category_name` / `category_slug` documents for related
   posts. Include canonical richtext body projection and invalid-document rollback
   in the retained PostgreSQL evidence.
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
6. **Execute and retain Blog article richtext cutover evidence.** Run the new
   owner-specific offline backfill in default dry-run mode and retain its NDJSON
   report. Resolve unknown formats manually; use `--allow-markdown-plain-text`
   only after accepting literal-text conversion, then rerun with `--apply`.
   Complete a final unscoped dry-run before the global migration. Execute the
   irreversible migration, perform Blog Search reindex/rollback, and
   retain Next/Leptos save-reload-SSR, GraphQL/native, AI draft persistence, and
   browser evidence on the same commit. **Done when:** representative PostgreSQL
   rows pass post-apply verification, the migration has executed, and no runtime
   path accepts Markdown, format aliases, or raw JSON.

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
- request-audience composition and `ProfileSummaryLoader` privacy tests cover
  anonymous, authenticated, owner-private, service-principal, hidden, missing,
  and cross-tenant author summaries
- `node scripts/verify/verify-blog-graphql-rate-limit.mjs`
- `npm run verify:blog:graphql-richtext-boundary`
- `npm run test:verify:blog:graphql-richtext-boundary`
- `npm run verify:blog:richtext-offline-backfill`
- `npm run verify:blog:forum-ui-ownership`
- `cargo run -p rustok-blog --bin blog_article_richtext_backfill -- --help`
- `cargo test -p rustok-comments --test thread_write_invariants`
- `node scripts/verify/verify-comments-thread-write-invariants.mjs`
- `cargo test -p rustok-search engine::tests::canonical_url`
- `cargo test -p rustok-search --test blog_ingestion_contract_test`
- `RUSTOK_SEARCH_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-search --test blog_projection_postgres_test`
- `cargo check -p rustok-server --features mod-blog`
- `npm run verify:blog:admin-boundary`
- `npm run test:verify:blog:admin-boundary`
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
