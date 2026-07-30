# rustok-blog implementation plan

## Current state

`rustok-blog` owns localized posts, Blog categories and tags, channel-aware
publication visibility, GraphQL/HTTP/native adapters, and admin/storefront
packages. It consumes `rustok-comments` through `CommentsThreadPort`; native
`#[server]` and GraphQL remain parallel transports over the same owner services.

The Blog article boundary is target-only richtext. Owner and GraphQL writes
accept `rustok_api::RichTextDocument`; reads expose `rustok_api::RichTextView`
and server-derived plain text under the fixed `article` profile. Production DTOs
do not expose `body`, `body_format`, `content_json`, Markdown aliases, raw JSON
write fields, caller-selected profiles, or local renderers.

The owner-specific offline backfill lives at
`crates/rustok-blog/src/bin/blog_article_richtext_backfill.rs`. Dry-run is the
default, reports are content-free NDJSON, writes require `--apply`, and historical
Markdown conversion requires explicit `--allow-markdown-plain-text`
acknowledgement. The utility does not execute the irreversible migration or
trigger Search reindex.

The Blog storefront selected-post path consumes the owner projection through
both transports. It renders server-rendered `RichTextView` HTML with exactly one
`content.html` sink and uses server-derived plain text as fallback. The public
comments projection is Comments-owned and approved-only, and storefront comment
pagination is route-owned and bounded consistently across GraphQL and native
SSR. The active DTO/UI path has no legacy body or format field.

The Blog admin uses typed `RichTextDocument` state and the owner
`BlogRichTextEditor`. The fixed Article frame is isolated with an
`allow-scripts`-only/no-referrer policy and disposed during Leptos cleanup.
Moderation remains separately permission-gated and paginated. Forum navigation,
GraphQL helpers, reply UI, and its contained format adapter are Forum-owned.

Search consumes Blog lifecycle and reindex events without importing the Blog
crate. It denormalizes `category_name`, `category_slug`, and the canonical post
slug. Canonical article JSON is parsed and projected through the shared Article
plain-text policy in the same transaction; invalid content fails closed.
Navigation is owned by `canonical_search_result_url` across GraphQL, storefront
native Search, Search admin preview, and admin global search.

Canonical navigation is a Search-owned provider boundary, not a Blog-owned gate.
Blog consumes the projected canonical URL and must not reconstruct routes. The
owner evidence is
`crates/rustok-search/contracts/evidence/search-canonical-url-contract.json`,
verified by `scripts/verify/verify-search-canonical-url-contract.mjs` and focused
fixture `scripts/verify/verify-search-canonical-url-contract.test.mjs`. Exact
leaf commands `verify:search:canonical-url` and `test:verify:search:canonical-url`
are locked into the Search FBA verify/test chains.

Blog lifecycle projection is also Search-owned. Its retained executable harness
is `crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`,
guarded by `scripts/verify/verify-search-blog-projection.mjs` and focused fixture
`scripts/verify/verify-search-blog-projection.test.mjs`. Exact commands
`verify:search:blog-projection` and `test:verify:search:blog-projection` are locked
into the Search FBA verify/test chains after canonical navigation. This source
registration does not record routing or PostgreSQL execution.

Comments thread positioning, active counters, first-thread identity, and repair
migrations are Comments-owned provider invariants. Their retained evidence is
`crates/rustok-comments/contracts/evidence/comments-thread-write-invariants.json`,
guarded by `scripts/verify/verify-comments-thread-write-invariants.mjs` and focused
self-test `scripts/verify/verify-comments-thread-write-invariants.test.mjs`. Exact
commands `verify:comments:thread-write-invariants` and
`test:verify:comments:thread-write-invariants` are registered in Comments registry
schema v2 and the Comments FBA verify/test chains. Evidence schema v2 also locks
an owner-classified tenant/target identity marker: canonical lookup occurs only
for that expected first-thread conflict, while unrelated insert storage errors
propagate as typed database failures. Blog consumes the owner result and does not
duplicate thread locking, counter policy, or error classification.

The Comments consumer call surface is now a first-class Blog source boundary.
`CommentService` depends on `Arc<dyn CommentsThreadPort>`, uses the in-process
provider only through `in_process_comments_thread_port`, and routes all seven
operations through typed `PortContext` / `PortError` contracts. Reads and writes
carry a two-second deadline; writes add command-scoped idempotency keys; public
lists use the dedicated approved-only operation and service actor; richtext input,
view, and plain-text projections remain typed. Evidence schema v2 lives at
`crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`.
The active fallback/error source evidence is
`crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`,
with `consumer_error_mapping` bound to `crates/rustok-blog/src/services/comment.rs`
rather than the legacy `CommentsError` conversion. The fail-closed gate is
`scripts/verify/verify-blog-comments-port-boundary.mjs` with focused fixture
`scripts/verify/verify-blog-comments-port-boundary.test.mjs`; exact commands
`verify:blog:comments-port-boundary` and
`test:verify:blog:comments-port-boundary` run after storefront and before event
projection. The in-process source profile is verified; the remote adapter and
degraded UI modes remain planned, and runtime evidence is pending.

Comments lifecycle projection into Blog-owned `comment_count` is a Blog consumer
boundary. `BlogCommentProjectionHandler` accepts only `comment.created` and
`comment.deleted` for `blog_post`, uses the envelope id as the durable delivery
identity, and commits the tenant-scoped optimistic count update, delivery row,
and `BlogPostUpdated` outbox publication in one transaction. The retained source
evidence is `crates/rustok-blog/contracts/evidence/blog-comments-event-projection.json`,
guarded by `scripts/verify/verify-blog-comments-event-projection.mjs` and focused
fixture `scripts/verify/verify-blog-comments-event-projection.test.mjs`. Exact
commands `verify:blog:comments-event-projection` and
`test:verify:blog:comments-event-projection` run after the Comments port gate in
the Blog FBA chains. Status remains `source_verified_no_compile`; runtime delivery
and recovery evidence is not recorded.

Blog categories use the exclusive `blog_categories:*` permission resource.
`CategoryService::new(db, event_bus)` is the only owner constructor. Category
mutation and Blog reindex publication share one transaction; authorization
precedes lookup; parent and translation operations are tenant-scoped; a name
that cannot derive a route key requires a non-empty ASCII slug; service and HTTP
pagination clamp `per_page` to `1..100`. The retained source contract is
`blog-category-search-reindex-contract.json`, verified by
`verify-blog-category-search-reindex.mjs` and its focused fixture
`verify-blog-category-search-reindex.test.mjs`. Both are registered as the
`category_search_reindex` leaf gate in the Blog FBA verify/test chain.

GraphQL load protection is field-aware over the host `SharedApiRateLimiter`.
Exceeded responses expose matching GraphQL `retryAfter` and HTTP `Retry-After`;
backend failure is fail-closed. The retained no-compile harness is
`blog-graphql-rate-limit-runtime-harness.json`, guarded by
`verify-blog-graphql-rate-limit.mjs` and its focused fixture
`verify-blog-graphql-rate-limit.test.mjs`. Both are registered as the
`graphql_rate_limit` leaf gate in the Blog FBA verify/test chain; mounted Redis
execution remains maintainer-owned.

### AI Blog owner boundary

The AI Blog draft writer in `crates/rustok-ai/src/direct.rs` reads existing
source material through `content_plain_text`, converts generated create and
update text with `article_document_from_plain_text`, and persists drafts with
`publish: false` through `PostService`.

The private AI Blog owner shim at `crates/rustok-ai/src/rustok_blog.rs` exports
only `CreatePostInput`, `PostResponse`, `PostService`, `UpdatePostInput`, and
`richtext`. Owner migrations and other Blog internals are outside this boundary.
Evidence is recorded in
`crates/rustok-blog/contracts/evidence/blog-ai-richtext-boundary.json`; the
fail-closed source gate is `scripts/verify/verify-blog-ai-richtext-boundary.mjs`
with fixture `scripts/verify/verify-blog-ai-richtext-boundary.test.mjs`.

## 2026-07-30 source re-audit

The 34 previously recorded implementation slices were rechecked against `main`
at `1c27a58320db2d91179beabfda064f22bcf82619`, their machine evidence, and the
registered source gates. No compile, database, browser, workflow, or CI status
was promoted; execution remains maintainer-owned.

The audit confirmed the latest admin, GraphQL, storefront, offline-backfill,
Forum ownership, and aggregate FBA evidence. It also found later commit
`2f1a4f20f530b1ec8e3cee3c1f51efc36aa5017f` widening the private AI Blog shim
with an unused `migrations` re-export. Slice 35 removes that drift, binds the
exact owner-only surface to machine evidence and a negative fixture, reconciles
the richtext source inventory, and registers the new gate in the Blog FBA
verify/test chain.

The continuation audit at `ce3b1690bbf7d67e5ea80cad071180deae7e62dc`
found that the category/Search source contract and focused negative fixture were
present but had no named npm leaf commands and were absent from the registry-owned
Blog FBA chain. Slice 36 registers that existing evidence without changing the
runtime contract or promoting its execution status.

The continuation audit at `e1cd838c4cb2580f271f8c58708416067057e530`
found the same aggregate omission for GraphQL load protection: the executable
no-compile harness, verifier, and focused negative fixture existed, but had no
named npm leaf commands and were absent from the registry-owned Blog FBA chain.
Slice 37 registers that source gate while preserving mounted Redis execution as
a separate maintainer-owned result.

The continuation audit at `d0e2a1cea5f0cba6102ca857a881f357c1cbd40e`
confirmed that canonical navigation is already Search-owned and therefore must
not be duplicated in the Blog FBA registry. It also found the canonical positive
fixture stale after the verifier gained Forum reply and admin permission checks.
Slice 38 repairs the owner fixture, adds exact Search leaf commands, and locks
their order into the Search FBA package chains without promoting runtime status.

The continuation audit at `9dcbcf31e527a406cb9556bdc2e20165c141e6bf`
found the remaining Search-owned Blog projection harness, verifier, and focused
fixture outside the Search FBA package chains and without named npm leaf commands.
Slice 39 registers that owner evidence and exact order while leaving routing and
PostgreSQL execution in maintainer-owned `Next results`.

The continuation audit at `8db76d1ae6e1bd5dce2314b9a5c11829373fa93d`
found the same source-chain omission at the Comments owner boundary. Thread write
evidence, verifier, negative fixture, and both Rust concurrency targets existed,
but the JS self-test was absent from the registry and no named leaf or Comments
FBA test chain existed. Slice 40 registers registry schema v2 and exact package
order without recording PostgreSQL execution.

The continuation audit at `ee93fd94a35d4200299a717b562c658642709c7b`
found that the implemented Comments-to-Blog event projection was checked only by
several inline aggregate markers. It had no dedicated machine evidence, focused
negative fixture, named npm leaf commands, or first-class Blog source gate. Slice
41 adds those owner-side artifacts, upgrades Blog registry schema v9, and locks
the exact verify/test order while leaving runtime delivery and recovery pending.

The continuation audit at `b788f806bd600595b79425d85cacc13d70f08158`
found the broader Comments consumer port contract still represented as
`planned_cases_locked`, with `get_comment` missing from the static matrix and
fallback evidence pointing to the legacy `error.rs` conversion instead of the
active `PortErrorKind` mapper. Slice 42 upgrades the matrix to source evidence,
adds a focused fail-closed fixture and exact leaf commands, aligns the shared
runtime-order evidence, upgrades Blog registry schema v10, and keeps the remote
adapter plus degraded UI modes explicitly pending.

The continuation audit at `6b5cd3f94265ff7ba382ca89916a73065806a0b5`
found that the Comments owner still retried canonical lookup after every thread
insert error. Slice 43 keeps classification with the provider: Comments emits a
scope-specific identity-conflict marker, retries only that expected first-thread
race, propagates unrelated storage errors, upgrades its retained invariant
evidence to schema v2, and adds focused negatives. Blog receives the corrected
typed provider result without adding a consumer-side classifier.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Blog FBA source-gate chain: `source_verified_no_compile`; registry schema v10
  locks exact verify/test order, source-gate paths, leaf npm commands, evidence,
  self-tests, and aggregate/consumer bindings for admin, storefront, Comments
  port boundary, Comments event projection, category Search reindex, GraphQL rate
  limiting, GraphQL richtext, AI richtext, offline backfill, Forum ownership, and
  runtime order.
- Comments consumer port boundary: Blog-owned `source_verified_no_compile` for
  the in-process profile; all seven operations, approved public read, typed
  richtext projection, two-second deadlines, write idempotency, active typed
  `PortErrorKind` mapping, exact npm leaf commands, focused fixture, and Blog FBA
  ordering are locked. The remote adapter and degraded UI modes remain planned;
  runtime evidence is pending.
- Comments event projection: Blog-owned `source_verified_no_compile`; evidence,
  verifier, focused self-test, exact npm leaf commands, delivery-ledger identity,
  transactional outbox markers, and Blog FBA ordering are locked. Duplicate,
  out-of-order, retry, rollback, restart, and PostgreSQL execution remain pending.
- Load protection: `implementation_ready`; mounted Redis evidence is pending.
- Rate-limit harness: `executable_no_compile`; evidence, verifier, self-test,
  npm leaf commands, and aggregate FBA registration are locked; execution is
  maintainer-owned.
- Search Blog projection harness: Search-owned `executable_no_run`; evidence,
  verifier, focused fixture, exact npm leaf commands, test targets, and Search FBA
  ordering are locked. Routing and PostgreSQL execution remain maintainer-owned.
- Blog article richtext cutover: `implemented_source_verified_no_compile`.
- Blog article offline backfill: `executable_no_run`.
- Next admin Forum UI ownership: `source_verified_no_compile`.
- Blog admin canonical richtext guardrail: `source_verified_no_compile`.
- Blog GraphQL richtext boundary: `implemented_source_verified_no_compile`.
- Blog storefront richtext boundary: `source_verified_no_compile`.
- AI Blog draft owner writes and shim: `source_verified_no_compile`.
- Comments thread write invariants: Comments-owned `executable_no_run`; registry
  schema v2, evidence schema v2, owner identity classifier, guarded canonical
  fallback, unrelated-storage propagation, verifier, focused self-test, exact npm
  leaf commands, Rust targets, and Comments FBA ordering are locked. PostgreSQL
  and injected storage-error execution remain maintainer-owned.
- Category search reindex: `source_verified_no_compile`; evidence, verifier,
  self-test, npm leaf commands, and aggregate FBA registration are locked.
- Canonical Search URL: Search-owned `source_verified_no_compile`; evidence,
  verifier, synchronized fixture, exact npm leaf commands, and Search FBA package
  ordering are locked. Runtime navigation evidence remains pending.

## Evidence and guardrails

- `crates/rustok-blog/contracts/blog-fba-registry.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-consumer-runtime-order-smoke.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-event-projection.json`
- `crates/rustok-comments/contracts/comments-fba-registry.json`
- `crates/rustok-comments/contracts/evidence/comments-thread-write-invariants.json`
- `crates/rustok-blog/contracts/evidence/blog-graphql-rate-limit-runtime-harness.json`
- `crates/rustok-blog/contracts/evidence/blog-category-search-reindex-contract.json`
- `crates/rustok-blog/contracts/evidence/blog-graphql-richtext-boundary.json`
- `crates/rustok-blog/contracts/evidence/blog-storefront-richtext-view.json`
- `crates/rustok-blog/contracts/evidence/blog-ai-richtext-boundary.json`
- `crates/rustok-blog/contracts/evidence/blog-richtext-cutover-inventory.json`
- `crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json`
- `crates/rustok-blog/contracts/evidence/blog-forum-ui-ownership.json`
- `crates/rustok-blog/contracts/evidence/blog-admin-richtext-boundary.json`
- `crates/rustok-blog/docs/richtext-cutover-inventory.md`
- `crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`
- `crates/rustok-search/contracts/evidence/search-canonical-url-contract.json`
- `scripts/verify/verify-blog-comments-port-boundary.mjs`
- `scripts/verify/verify-blog-comments-port-boundary.test.mjs`
- `scripts/verify/verify-blog-comments-event-projection.mjs`
- `scripts/verify/verify-blog-comments-event-projection.test.mjs`
- `scripts/verify/verify-blog-graphql-rate-limit.mjs`
- `scripts/verify/verify-blog-graphql-rate-limit.test.mjs`
- `scripts/verify/verify-blog-category-search-reindex.mjs`
- `scripts/verify/verify-blog-category-search-reindex.test.mjs`
- `scripts/verify/verify-blog-graphql-richtext-boundary.mjs`
- `scripts/verify/verify-blog-graphql-richtext-boundary.test.mjs`
- `scripts/verify/verify-blog-storefront-boundary.mjs`
- `scripts/verify/verify-blog-storefront-boundary.test.mjs`
- `scripts/verify/verify-blog-ai-richtext-boundary.mjs`
- `scripts/verify/verify-blog-ai-richtext-boundary.test.mjs`
- `scripts/verify/verify-blog-richtext-offline-backfill.mjs`
- `scripts/verify/verify-blog-richtext-offline-backfill.test.mjs`
- `scripts/verify/verify-blog-forum-ui-ownership.mjs`
- `scripts/verify/verify-blog-forum-ui-ownership.test.mjs`
- `scripts/verify/verify-blog-fba.mjs`
- `scripts/verify/verify-blog-fba.test.mjs`
- `scripts/verify/verify-blog-admin-boundary.mjs`
- `scripts/verify/verify-comments-fba.mjs`
- `scripts/verify/verify-comments-thread-write-invariants.mjs`
- `scripts/verify/verify-comments-thread-write-invariants.test.mjs`
- `scripts/verify/verify-search-blog-projection.mjs`
- `scripts/verify/verify-search-blog-projection.test.mjs`
- `scripts/verify/verify-search-canonical-url-contract.mjs`
- `scripts/verify/verify-search-canonical-url-contract.test.mjs`

## Completed implementation slices

1. Reconciled Blog load protection with host composition and avoided a duplicate
   REST limiter.
2. Added field-aware GraphQL classification, structured rate-limit errors,
   metrics, trusted-IP identity, and matching `Retry-After` handoff.
3. Aligned post mutation permissions across REST, GraphQL, domain, and limiter.
4. Added lifecycle Search projection, targeted/full reindex, module-toggle
   handling, cleanup, PostgreSQL harnesses, and active `search_path` discovery.
5. Hardened comment projection with a durable ledger, optimistic locking,
   retryable ordering, and transactional outbox publication.
6. Added Comments-owned approved public reads, moderation parity, and bounded
   storefront/admin pagination.
7. Added serialized thread positions, exact active counts, repair, and a unique
   thread-position invariant.
8. Added Search-owned canonical result URL policy across all consumers.
9. Removed transport-local Blog URL builders and navigation post-processing.
10. Added Blog category HTTP CRUD, typed DTOs/errors, OpenAPI/routes,
    transactional writes, Search reindex, tenant scope, and evidence.
11. Added exclusive `blog_categories:*` authority across platform and module
    permission surfaces.
12. Removed alternate category permission paths and required
    `TransactionalEventBus` in `CategoryService`.
13. Added the Blog article richtext owner boundary and server projections.
14. Bound GraphQL author batches to request-audience Profiles privacy policy.
15. Added typed GraphQL richtext evidence, guardrail, self-test, and FBA wiring.
16. Kept GraphQL resolvers as thin typed adapters over owner services.
17. Migrated storefront selected posts to `RichTextView` and derived text.
18. Added the owner plain-text article import adapter without format aliases.
19. Migrated Search rows to the shared Article plain-text policy.
20. Added the AI Blog adapter for canonical draft create/update writes.
21. Completed target-only source cutover and removed runtime compatibility paths.
22. Added Blog admin native CRUD/moderation adapters over owner services.
23. Mounted the shared framed editor through a browser-only hydration bridge.
24. Added the dry-run-first owner-specific offline backfill.
25. Moved Forum Next admin ownership out of Blog and shared the React lifecycle
    adapter at host scope.
26. Reconciled the Blog admin guardrail with the canonical editor.
27. Removed dead body-format/raw-payload locale contracts.
28. Locked the Blog FBA package chain and restored the storefront gate.
29. Extracted pure aggregate chain policy and self-regression coverage.
30. Bound each FBA leaf npm script to its exact verifier command.
31. Locked the complete FBA self-test chain in registry schema v5.
32. Bound the admin owner adapter to fixed Article/no-referrer/cleanup evidence.
33. Bound GraphQL create/update conversions to evidence schema v3 and negative
    fixtures.
34. Bound storefront rendering to evidence schema v2, one owner HTML sink, and
    rejection of local document/Markdown renderers.
35. Audited the AI Blog owner shim, removed the accidental unused `migrations`
    re-export, added machine evidence plus fail-closed verifier/fixture, reconciled
    the richtext inventory, and registered the gate in registry schema v6.
36. Registered the existing category/Search contract as a first-class Blog FBA
    leaf gate, added exact verify/test npm commands, bound its evidence path in
    registry schema v7, and locked aggregate order through the shared chain policy.
37. Registered the existing GraphQL rate-limit harness as a first-class Blog FBA
    leaf gate, added exact verify/test npm commands, bound its evidence path in
    registry schema v8, and kept mounted Redis execution explicitly pending.
38. Kept canonical navigation with the Search owner, repaired its stale positive
    fixture for Forum/admin expansion, added exact Search verify/test leaf commands,
    and locked both commands into the Search FBA package chains.
39. Registered the existing Search-owned Blog projection harness as a first-class
    Search FBA leaf, bound evidence/verifier/fixture/test targets and exact package
    order, and kept routing plus PostgreSQL execution explicitly pending.
40. Registered the existing Comments-owned thread invariant evidence as a
    first-class Comments FBA leaf, retained the focused JS self-test in registry
    schema v2, added exact verify/test package commands, and kept both PostgreSQL
    concurrency targets explicitly pending.
41. Added first-class Blog Comments event-projection evidence and a focused
    fail-closed fixture, registered exact verify/test leaf commands in registry
    schema v9, and kept delivery, retry, rollback, restart, and PostgreSQL execution
    explicitly pending.
42. Promoted the implemented Comments consumer port surface from a planned matrix
    to source-verified evidence, added the missing `get_comment` case, bound the
    active `PortErrorKind` mapper and runtime-order evidence to `services/comment.rs`,
    registered a focused negative fixture plus exact verify/test commands in Blog
    registry schema v10, and kept the remote adapter and degraded UI modes pending.
43. Kept thread-insert error classification with the Comments provider, added a
    tenant/target-scoped identity-conflict classifier, narrowed canonical fallback,
    propagated unrelated storage errors, upgraded owner evidence to schema v2, and
    retained all runtime and PostgreSQL proof as maintainer-owned.

## Next results

1. **Execute category runtime evidence.** Exercise HTTP CRUD using
   `blog_categories:*`; verify unrelated post/catalog permissions are denied and
   retain tenant, parent, slug, typed-status, pagination, authorization-order,
   and outbox rollback evidence.
2. **Execute Search refresh evidence.** Consume category-triggered reindex and
   retain changed `category_name` / `category_slug` documents. Include canonical
   richtext body projection and invalid-document rollback.
3. **Execute canonical navigation evidence.** Verify Blog results through
   GraphQL, storefront native Search, Search admin preview, and admin global
   search; retain malformed-slug failure and canonical click-href evidence.
4. **Execute mounted rate-limit evidence.** Run policy, memory adapter,
   controller handoff, focused verifier, then Redis-backed host requests with a
   real HTTP `Retry-After` matching GraphQL `retryAfter`.
5. **Close comments runtime evidence.** Run the Comments port boundary fixture,
   shared consumer runtime-order verifier, both thread invariant concurrency
   targets, and concurrent PostgreSQL create/delete transactions; cover the
   remote adapter, degraded UI modes, approved-only reads, moderation, pagination,
   duplicate delivery, counters, first-thread identity, unrelated insert storage
   error propagation, missing-post retry, rollback, outbox publication, and restart
   recovery.
6. **Execute and retain Blog article richtext cutover evidence.** Run the offline
   backfill in default dry-run mode, review its report, apply accepted conversion,
   execute the irreversible migration, reindex/rollback Search, and retain
   Next/Leptos, GraphQL/native, AI draft persistence, and browser evidence on the
   same commit.

## Verification

Execution is intentionally not recorded by this source-only update. Maintainers
should run the relevant subset, including:

- `npm run verify:blog:comments-port-boundary`
- `npm run test:verify:blog:comments-port-boundary`
- `npm run verify:blog:comments-event-projection`
- `npm run test:verify:blog:comments-event-projection`
- `npm run verify:blog:category-search-reindex`
- `npm run test:verify:blog:category-search-reindex`
- `npm run verify:blog:graphql-rate-limit`
- `npm run test:verify:blog:graphql-rate-limit`
- `npm run verify:blog:graphql-richtext-boundary`
- `npm run test:verify:blog:graphql-richtext-boundary`
- `npm run verify:blog:storefront-boundary`
- `npm run test:verify:blog:storefront-boundary`
- `npm run verify:blog:ai-richtext-boundary`
- `npm run test:verify:blog:ai-richtext-boundary`
- `npm run verify:blog:richtext-offline-backfill`
- `npm run test:verify:blog:richtext-offline-backfill`
- `npm run verify:blog:forum-ui-ownership`
- `npm run test:verify:blog:forum-ui-ownership`
- `npm run verify:blog:admin-boundary`
- `npm run test:verify:blog:admin-boundary`
- `npm run verify:blog:fba`
- `npm run test:verify:blog:fba`
- `npm run verify:search:canonical-url`
- `npm run test:verify:search:canonical-url`
- `npm run verify:search:blog-projection`
- `npm run test:verify:search:blog-projection`
- `npm run verify:search:fba`
- `npm run test:verify:search:fba`
- `npm run verify:comments:thread-write-invariants`
- `npm run test:verify:comments:thread-write-invariants`
- `npm run verify:comments:fba`
- `npm run test:verify:comments:fba`
- `cargo run -p rustok-blog --bin blog_article_richtext_backfill -- --help`
- `cargo test -p rustok-blog --test graphql_rate_limit_policy_test`
- `cargo test -p rustok-blog graphql::rate_limit`
- `cargo test -p rustok-server graphql_http_response_preserves_extension_headers`
- `cargo test -p rustok-comments --test thread_write_invariants`
- `cargo test -p rustok-comments --test thread_creation_concurrency`
- Targeted injected storage-error coverage for Comments thread creation
- `cargo test -p rustok-search engine::tests::canonical_url`
- `cargo test -p rustok-search --test blog_ingestion_contract_test`
- `RUSTOK_SEARCH_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-search --test blog_projection_postgres_test`
- `cargo check -p rustok-ai --features server`
- `cargo check -p rustok-server --features mod-blog`
- `cargo check -p rustok-blog-admin --features ssr`
- `cargo check -p rustok-blog-admin --target wasm32-unknown-unknown --features hydrate`
- `npm run verify --prefix packages/richtext`
- `node packages/richtext/test/browser-spike.mjs`
- `npm run verify:consumer:fba-runtime-order`
- `cargo xtask module validate blog`

## References

- [Crate README](../README.md)
- [Blog documentation](./README.md)
- [Comments consumer registry](../contracts/blog-fba-registry.json)
- [Richtext implementation plan](../../../docs/modules/rich-text-implementation-plan.md)
