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

The pre-release initial migration creates the target schema directly: article
body is canonical richtext JSON in `blog_post_translations.body`, with no format
selector. Corrective cutover migrations and conversion executables are forbidden;
`scripts/verify/verify-blog-fba.mjs` rejects their reintroduction.

The Blog storefront selected-post path consumes the owner projection through
both transports. The selected post has exactly one shared `RichTextHtml` sink
for server-rendered `RichTextView` HTML and uses server-derived plain text as
fallback. The public comments projection is Comments-owned and approved-only.
The storefront comment pagination is route-owned and bounded consistently
across GraphQL and native SSR. Public comment reads carry typed `AVAILABLE`,
`UNAVAILABLE`, or `TIMEOUT`
availability across both transports, while the article remains renderable for the
two degraded states. The active DTO/UI path has no legacy body or format field.
The typed storefront comments availability is source-locked. The remote transport remains pending.
The Leptos article SSR path renders an inert localized comment marker and loads
an isolated authenticated comment island instead of hydrating the storefront.
Its CSP-nonced bootstrap imports the shared Tiptap frame only for an active
browser session, while the canonical authorized server-function client attaches
the stored bearer and tenant context to `blog/comment-create`. The minimal WASM
artifact builds successfully. Content serves the canonical manifest-selected
frame HTML, adapter, script, and stylesheet; Blog serves only its island assets.
Mounted persistence/reload evidence remains open.

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
schema v4 and the Comments FBA verify/test chains. Evidence schema v3 locks an
owner-classified tenant/target identity marker with a valid canonical thread UUID,
the registered Rust classifier harness, canonical lookup only for the expected
first-thread conflict, and typed propagation of unrelated insert storage errors.
Blog consumes the owner result and does not duplicate thread locking, counter
policy, or error classification.

The Comments consumer call surface is a first-class Blog source boundary.
`CommentService` depends on `Arc<dyn CommentsThreadPort>`, uses the in-process
provider only through `in_process_comments_thread_port`, and routes all seven
operations through typed `PortContext` / `PortError` contracts. Reads and writes
carry a two-second deadline; writes add command-scoped idempotency keys; public
lists use the dedicated approved-only operation and service actor; richtext input,
view, and plain-text projections remain typed. `CommentService::new` remains the
in-process convenience constructor, while public
`CommentService::with_comments_thread_port` accepts a host-owned
`Arc<dyn CommentsThreadPort>` without changing the owner service API. The Blog
facade re-exports `CommentsThreadPort` so UI packages can name the already-public
injection contract without depending directly on the provider crate. The retained
compile-only harness is
`services::comment::port_injection_tests::comment_service_accepts_an_injected_comments_thread_port`
in `crates/rustok-blog/src/services/comment.rs`, with suggested command
`cargo test -p rustok-blog --lib services::comment::port_injection_tests::comment_service_accepts_an_injected_comments_thread_port -- --exact`.
Evidence schema v3 lives at
`crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`.
The active fallback/error source evidence is
`crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`,
with `consumer_error_mapping` bound to `crates/rustok-blog/src/services/comment.rs`
rather than the legacy `CommentsError` conversion. The fail-closed gate is
`scripts/verify/verify-blog-comments-port-boundary.mjs` with focused fixture
`scripts/verify/verify-blog-comments-port-boundary.test.mjs`; exact commands
`verify:blog:comments-port-boundary` and
`test:verify:blog:comments-port-boundary` run after storefront and before event
projection. Typed storefront comments availability is source-verified across the
owner GraphQL resolver, GraphQL client, native SSR adapter, shared DTO, and Leptos
UI. Only `ExternalService` and `Timeout` become empty `UNAVAILABLE` or `TIMEOUT`
comment payloads; every other `BlogError` remains fail-closed. The in-process
source profile and transport-neutral injection seam are source-locked. The remote
transport remains pending; degraded UI modes remain planned. Cached snapshot and
comment-form fallback remain planned, and runtime evidence is pending.

HTTP moderation composition is retained separately. `BlogHttpRuntime::from_host`
reads an optional `Arc<dyn CommentsThreadPort>` through
`HostRuntimeContext::shared_get`, and `BlogHttpRuntime::comment_service` selects
`CommentService::with_comments_thread_port` when the host supplies one while
preserving `CommentService::new` as the in-process fallback. The moderation
controller delegates only to that selector. Schema-v1 evidence lives at
`crates/rustok-blog/contracts/evidence/blog-comments-http-port-injection.json`,
guarded by `scripts/verify/verify-blog-comments-http-port-injection.mjs` and
focused fixture
`scripts/verify/verify-blog-comments-http-port-injection.test.mjs`. The registered
`verify:blog:comments-port-boundary` verifier imports the standalone HTTP verifier,
and the registered `test:verify:blog:comments-port-boundary` self-test imports its
focused fixture. `scripts/verify/verify-blog-fba.test.mjs` locks both imports, so
the existing first-class Comments port leaf cannot silently drop HTTP composition
coverage. The retained compile-only harness is
`controllers::tests::blog_http_runtime_exposes_comments_port_selection`, with
suggested command
`cargo test -p rustok-blog --lib controllers::tests::blog_http_runtime_exposes_comments_port_selection -- --exact`.
HTTP moderation host selection is source-locked; the remote network transport
remains pending.

GraphQL Comments composition is manifest-attached. The Blog package declares
`runtime_data_factory = "graphql::attach_schema_data"`; generated server code
passes `GraphqlRuntimeInputs` through
`schema_codegen::attach_module_graphql_data`, and `BlogGraphqlRuntimeData` reads an
optional `Arc<dyn CommentsThreadPort>` with `GraphqlRuntimeInputs::shared_get`.
Its single `BlogGraphqlRuntimeData::comment_service` selector chooses
`CommentService::with_comments_thread_port` or the existing in-process
`CommentService::new` fallback. Public comments, moderation comments, and the
moderation mutation all consume that schema data rather than constructing a
provider in resolver source. Schema-v1 evidence lives at
`crates/rustok-blog/contracts/evidence/blog-comments-graphql-port-injection.json`,
guarded by `scripts/verify/verify-blog-comments-graphql-port-injection.mjs` and
focused fixture
`scripts/verify/verify-blog-comments-graphql-port-injection.test.mjs`. The retained
compile-only harness is
`graphql::runtime_data::tests::graphql_runtime_data_exposes_comments_port_selection`,
with suggested command
`cargo test -p rustok-blog --lib graphql::runtime_data::tests::graphql_runtime_data_exposes_comments_port_selection -- --exact`.
The registered `verify:blog:comments-port-boundary` verifier imports the standalone
GraphQL verifier, and the registered `test:verify:blog:comments-port-boundary`
self-test imports all twelve focused cases. `scripts/verify/verify-blog-fba.test.mjs`
locks both imports alongside the HTTP composition imports. GraphQL Comments host selection is source-locked.
It is mandatory inside the existing first-class Comments port leaf. Blog FBA package-chain registration remains pending
only for a dedicated parallel GraphQL leaf, which is intentionally absent. The
remote network transport remains pending.

Storefront native SSR Comments composition is host-attached. The selected-post
server function reads `HostRuntimeContext`, and its single `comment_service`
selector looks up an optional `Arc<dyn rustok_blog::CommentsThreadPort>` with
`HostRuntimeContext::shared_get`. It chooses
`CommentService::with_comments_thread_port` when the host supplies a port and
preserves `CommentService::new` as the in-process fallback. The approved-only
public read delegates through that selector without changing pagination or typed
`AVAILABLE` / `UNAVAILABLE` / `TIMEOUT` degradation. Schema-v1 evidence lives at
`crates/rustok-blog/contracts/evidence/blog-comments-storefront-native-port-injection.json`,
guarded by
`scripts/verify/verify-blog-comments-storefront-native-port-injection.mjs` and
focused fixture
`scripts/verify/verify-blog-comments-storefront-native-port-injection.test.mjs`.
The retained compile-only harness is
`transport::native_server_adapter::tests::storefront_native_runtime_exposes_comments_port_selection`,
with suggested command
`cargo test -p rustok-blog-storefront --features ssr transport::native_server_adapter::tests::storefront_native_runtime_exposes_comments_port_selection -- --exact`.
The registered `verify:blog:comments-port-boundary` verifier imports the standalone
storefront native verifier, and the registered
`test:verify:blog:comments-port-boundary` self-test imports all sixteen focused
cases. `scripts/verify/verify-blog-fba.test.mjs` locks both imports alongside the
HTTP and GraphQL composition imports. The source marker `storefront native SSR
Comments host selection is source-locked` is mandatory inside the existing
first-class Comments port leaf. Blog FBA package-chain registration remains
pending only as a dedicated parallel storefront leaf; that duplicate leaf is
intentionally not added. The stale `admin_native_ssr_composition` pending marker
was removed after admin native composition became source-locked. The remote
network transport remains pending.

Admin native SSR Comments composition is host-attached through the existing
`NativeContext`. `native_context()` reads an optional
`Arc<dyn rustok_blog::CommentsThreadPort>` from `HostRuntimeContext`, and the
single `comment_service(&NativeContext)` selector chooses
`CommentService::with_comments_thread_port` or the existing in-process
`CommentService::new` fallback. Both `blog/admin/moderation-comments` and
`blog/admin/moderate-comment` require authenticated/routed tenant equality and
`blog_posts:manage` before selecting the service. Moderation list pagination
remains bounded to page at least one and `per_page` in `1..100`; all Blog errors
remain fail-closed and no storefront empty-success degradation is reused.
Schema-v1 evidence lives at
`crates/rustok-blog/contracts/evidence/blog-comments-admin-native-port-injection.json`,
guarded by `scripts/verify/verify-blog-comments-admin-native-port-injection.mjs`
and focused fixture
`scripts/verify/verify-blog-comments-admin-native-port-injection.test.mjs`. The
retained compile-only harness is
`transport::native_server_adapter::tests::admin_native_runtime_exposes_comments_port_selection`,
with suggested command
`cargo test -p rustok-blog-admin --features ssr transport::native_server_adapter::tests::admin_native_runtime_exposes_comments_port_selection -- --exact`.
The registered `verify:blog:comments-port-boundary` verifier imports the standalone
admin native verifier, and the registered
`test:verify:blog:comments-port-boundary` self-test imports all seventeen focused
cases. `scripts/verify/verify-blog-fba.test.mjs` locks both imports beside the
HTTP, GraphQL, and storefront native composition imports. The source marker
`admin native SSR Comments host selection is source-locked` is mandatory inside
the existing first-class Comments port leaf. Blog FBA package-chain registration
remains pending only as a dedicated parallel admin native leaf; that duplicate
leaf is intentionally not added. The remote network transport remains pending.

Comments lifecycle projection into Blog-owned `comment_count` is a Blog consumer
boundary. `BlogCommentProjectionHandler` accepts only `comment.created` and
`comment.deleted` for `blog_post`, uses the envelope id as the durable delivery
identity, and commits the tenant-scoped optimistic count update, delivery row,
and `BlogPostUpdated` outbox publication in one transaction. `project()` and
`EventHandler::handles()` share the pure `comment_projection_change` classifier,
and the pure counter transition floors deletes at zero while saturating count and
version overflow. Evidence schema v4 is retained at
`crates/rustok-blog/contracts/evidence/blog-comments-event-projection.json`,
guarded by `scripts/verify/verify-blog-comments-event-projection.mjs` and focused
fixture `scripts/verify/verify-blog-comments-event-projection.test.mjs`. The Rust
source harness is the `services::comment_projection::tests` module in
`crates/rustok-blog/src/services/comment_projection.rs`, with status
`executable_no_run` and suggested command
`cargo test -p rustok-blog --lib services::comment_projection::tests`.

The production optimistic-update loop delegates every zero-row/success result to
the shared pure `ProjectionUpdateDecision` helper. One updated row is applied
immediately; zero rows produce seven retry decisions before the eighth attempt
reaches `LimitReached`. The retained Rust cases are
`optimistic_retry_policy_applies_success_without_retry` and
`optimistic_retry_policy_allows_seven_retries_then_stops_on_eighth_conflict`.
They are `executable_no_run` source-policy evidence and do not measure PostgreSQL
contention or observed natural retry frequency.

The retained deterministic PostgreSQL retry-limit target is
`optimistic_retry_limit_rolls_back_and_replays_after_conflict_clears` in
`crates/rustok-blog/tests/comment_projection_postgres_test.rs`. It installs a
schema-local `BEFORE UPDATE` trigger that returns `NULL`, so the real handler
observes eight zero-row update results. A PostgreSQL sequence records all eight
attempts outside transaction rollback. The written assertions require the
terminal error, unchanged post state, no delivery or outbox row, removal of the
probe, and successful replay of the same envelope. Its suggested command is
`RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test optimistic_retry_limit_rolls_back_and_replays_after_conflict_clears -- --exact`.
The target is `executable_no_run`; it does not record execution or measure natural
contention frequency.

The retained host registration target is the `tests` module in
`crates/rustok-blog/src/lib.rs`. It creates the real module listener context,
invokes `BlogModule::register_event_listeners`, extracts the single registered
handler, verifies the `blog_comment_projection` identity, accepts Blog
`comment.created` / `comment.deleted`, and rejects a non-Blog target. Its suggested
command is
`cargo test -p rustok-blog --lib tests::module_registers_comment_projection_handler_with_host_routing`.
The target is `executable_no_run` and intentionally does not call `handle()`;
EventBus/EventDispatcher delivery is covered by the separate PostgreSQL source
target below.

The retained dispatcher target is the filtered
`event_dispatcher_routes_registered_handler_and_commits_projection` case in
`crates/rustok-blog/tests/comment_projection_postgres_test.rs`. It builds the real
module listener context, registers handlers through
`BlogModule::register_event_listeners`, moves them into `EventDispatcher`, starts
the subscriber, publishes one envelope through `EventBus`, waits for the durable
delivery marker, and then requires one counter transition, one delivery row, and
one outbox row. Its suggested command is
`RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test event_dispatcher_routes_registered_handler_and_commits_projection -- --exact`.
The target is `executable_no_run`; no dispatcher or PostgreSQL output is recorded.

The retained concurrency target is the filtered
`concurrent_created_events_converge_without_lost_updates` case in
`crates/rustok-blog/tests/comment_projection_postgres_test.rs`. It creates four
independent PostgreSQL connections against one isolated schema, constructs one
handler per connection, releases four unique `comment.created` envelopes through
a shared barrier, and requires the shared post to finish at `comment_count = 4`
and `version = 5` with four delivery-ledger rows and four outbox rows. Its
suggested command is
`RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test concurrent_created_events_converge_without_lost_updates -- --exact`.
The target is `executable_no_run`. It records a written convergence contract, not
an observed retry count or optimistic-exhaustion result.

The retained concurrent duplicate-delivery target is
`concurrent_duplicate_envelope_commits_once_and_replays_cleanly` in
`crates/rustok-blog/tests/comment_projection_duplicate_race_postgres_test.rs`.
A control transaction locks the Blog post before two named one-connection workers
start with the same envelope. The harness waits until `pg_stat_activity` reports
both workers blocked on a lock, proving both initial delivery-ledger lookups have
completed before the row lock is released. The written assertions require exactly
one successful handler, one failed losing transaction, final `comment_count = 1`
and `version = 2`, one delivery row, one outbox row, and a clean replay of the same
envelope. Evidence schema v1 is retained at
`crates/rustok-blog/contracts/evidence/blog-comments-duplicate-delivery-race.json`,
guarded by `scripts/verify/verify-blog-comments-duplicate-delivery-race.mjs` and
focused fixture
`scripts/verify/verify-blog-comments-duplicate-delivery-race.test.mjs`. Its
suggested command is
`RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_duplicate_race_postgres_test concurrent_duplicate_envelope_commits_once_and_replays_cleanly -- --exact`.
Exact leaf commands `verify:blog:comments-duplicate-delivery-race` and
`test:verify:blog:comments-duplicate-delivery-race` are registered immediately
after the main Comments event-projection leaf in both Blog FBA chains. The target
and focused guard remain `executable_no_run` / source-only; no PostgreSQL or
dispatcher-level duplicate result is recorded.

The retained dispatcher duplicate-delivery target is
`event_dispatcher_replays_duplicate_envelope_without_double_commit` in
`crates/rustok-blog/tests/comment_projection_dispatcher_duplicate_postgres_test.rs`.
It registers the real projection handler through
`BlogModule::register_event_listeners`, wraps that handler only to count completed
calls and errors, publishes the same envelope twice through `EventBus` and
`EventDispatcher`, and waits for both handler calls to finish. The written
assertions require two completed calls, zero handler errors, final
`comment_count = 1` and `version = 2`, one delivery row, and one outbox row.
Evidence schema v1 is retained at
`crates/rustok-blog/contracts/evidence/blog-comments-dispatcher-duplicate-delivery.json`,
guarded by
`scripts/verify/verify-blog-comments-dispatcher-duplicate-delivery.mjs` and focused
fixture
`scripts/verify/verify-blog-comments-dispatcher-duplicate-delivery.test.mjs`. Its
suggested command is
`RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_dispatcher_duplicate_postgres_test event_dispatcher_replays_duplicate_envelope_without_double_commit -- --exact`.
Exact leaf commands `verify:blog:comments-dispatcher-duplicate-delivery` and
`test:verify:blog:comments-dispatcher-duplicate-delivery` are registered
immediately after the concurrent duplicate-delivery race leaf in both Blog FBA
chains. This closes the source-only dispatcher-level duplicate delivery gap
without claiming PostgreSQL execution or the separate concurrent handler race.
The retained evidence remains `source_verified_no_compile` and the target remains
`executable_no_run`.

The retained PostgreSQL target is
`crates/rustok-blog/tests/comment_projection_postgres_test.rs`. It uses
`RUSTOK_BLOG_TEST_DATABASE_URL` (or PostgreSQL `DATABASE_URL`), a unique schema,
and a one-connection pool for each direct handler. Its five direct-handler cases
cover duplicate-envelope idempotency, deterministic retry-limit rollback/replay,
delete-before-create ordering, missing-post replay after source creation, and
rollback/retry when the outbox table is unavailable. The suggested command is
`RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test`.
The target is registered as `executable_no_run`; no PostgreSQL result is recorded.
All four Blog projection PostgreSQL harnesses set `search_path` to the generated
test schema only. They do not include `public`, so a deliberately missing local
table cannot fall through to a developer database table and invalidate rollback
coverage. The three focused source verifiers reject that fallback explicitly.

The retained same-process restart target is
`crates/rustok-blog/tests/comment_projection_restart_postgres_test.rs`. It applies
one envelope, drops the first handler, opens a new PostgreSQL connection against
the same isolated schema, creates a new handler, and replays the same envelope.
The written assertions require one counter transition, one durable delivery row,
and one outbox row. Its suggested command is
`RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_restart_postgres_test`.
This target is `executable_no_run`; it represents handler/connection
re-instantiation inside one test process.

The filtered process restart target is
`restarted_process_reuses_delivery_ledger_without_reapplying_counter` in the same
integration-test file. Its parent creates one isolated schema and envelope, then
launches the integration-test executable as two sequential OS test processes.
Each child runs only `process_restart_worker_applies_envelope_from_env`, rebuilds
the same envelope ID from private test environment variables, creates a fresh
PostgreSQL connection and handler, and exits before the next child starts. Final
assertions require one counter transition, one delivery row, and one outbox row.
Its suggested command is
`RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_restart_postgres_test restarted_process_reuses_delivery_ledger_without_reapplying_counter -- --exact`.
The target is `executable_no_run`; it proves a written OS process boundary, not a
full application server-host restart, and no execution result is recorded.

Exact commands `verify:blog:comments-event-projection` /
`test:verify:blog:comments-event-projection`,
`verify:blog:comments-duplicate-delivery-race` /
`test:verify:blog:comments-duplicate-delivery-race`, and
`verify:blog:comments-dispatcher-duplicate-delivery` /
`test:verify:blog:comments-dispatcher-duplicate-delivery` run after the Comments
port gate in that exact order. Overall status remains
`source_verified_no_compile`; the deterministic retry policy, retry-limit target,
concurrent duplicate race, and dispatcher duplicate replay are source-locked,
The four registered PostgreSQL targets passed locally against PostgreSQL 16 on
2026-08-01: 7 main projection cases, 3 restart/process-restart cases, 1 concurrent
duplicate-delivery race, and 1 dispatcher duplicate replay. This local run found
and removed a `public` search-path fallback that had allowed the outbox-failure
case to reach a developer table. Retained executable runtime evidence, naturally
contended retry-frequency evidence, full server-host restart recovery, and all
other runtime evidence remain pending, so the evidence status stays
`source_verified_no_compile` / `executable_no_run`.

Blog categories use the exclusive `blog_categories:*` permission resource.
`CategoryService::new(db, event_bus)` is the only owner constructor. Category
mutation and Blog reindex publication share one transaction; authorization
precedes lookup; parent and translation operations are tenant-scoped; a name
that cannot derive a route key requires a non-empty ASCII slug. Service and HTTP
pagination clamp `per_page` to `1..100`; service and HTTP pagination use the
same bound. The retained source contract is
`blog-category-search-reindex-contract.json`, verified by
`verify-blog-category-search-reindex.mjs` and its focused fixture
`verify-blog-category-search-reindex.test.mjs`. Both are registered as the
`category_search_reindex` leaf gate in the Blog FBA verify/test chain.

### Blog category Translation target pilot

`m20260803_000016_add_blog_category_translation_target_support` adds positive
resource and exact-locale revisions to Blog categories plus the append-only,
content-free `blog_translation_changes` owner journal. `CategoryService` writes
one journal entry for create, update, delete, and exact-locale Translation
apply. Its target-only apply operation performs resource/source/target CAS,
localized slug validation, and Blog Search reindex publication. The Translation
provider completes or replays the shared durable owner-operation receipt in the
same transaction.

`BlogCategoryTranslationTargetProvider` is registered by the server as
`blog/category`. It exposes exact `name`, review-only `slug`, and optional
`description`; it does not use locale fallback for target coverage. The shared
`rustok-translation-targets::provider_support` helpers centralize target-level
field hashing, patch CAS validation, revision encoding, and receipt decoding,
while Blog retains authorization, persistence, validation, and error mapping.
The focused SQLite suite in `src/translation_target_tests.rs` proves migration
`up/down/up`, exact apply, idempotent replay, same-key conflict rejection,
stale validation, cursor/progress facts, denied access, and one transactional
Search reindex outbox row.

This is a registered pilot, not a production-enablement claim. Retained
PostgreSQL migration, concurrent CAS, and change-cursor recovery evidence are
still required before production inventory enablement. Blog posts and
Taxonomy-owned tags remain out of scope for this pilot.

GraphQL load protection is field-aware over the host `SharedApiRateLimiter`.
Exceeded responses expose matching GraphQL `retryAfter` and HTTP `Retry-After`;
backend failure is fail-closed. The retained no-compile harness is
`blog-graphql-rate-limit-runtime-harness.json`, guarded by
`verify-blog-graphql-rate-limit.mjs` and its focused fixture
`scripts/verify/verify-blog-graphql-rate-limit.test.mjs`. Both are registered as the
`graphql_rate_limit` leaf gate in the Blog FBA test chain; mounted Redis
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

The audit confirmed the latest admin, GraphQL, storefront, Forum ownership, and
aggregate FBA evidence. It also found later commit
`2f1a4f20f530b1ec8e3cee3c1f51efc36aa5017f` widening the private AI Blog shim
with an unused `migrations` re-export. Slice 35 removes that drift, binds the
exact owner-only surface to machine evidence and a negative fixture, reconciles
the canonical AI richtext boundary, and registers the new gate in the Blog FBA
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

## 2026-07-31 source continuation audit

The continuation audit at `ea72eb8679ccecc16fcfd3ae895e14333fa33232`
found that the Comments event projection had a static transaction/ledger gate but
no executable Rust harness for event classification or counter transition policy.
`project()` and `EventHandler::handles()` also encoded classification separately,
allowing future source drift between dispatch and projection. Slice 44 extracts a
shared pure classifier and counter transition, adds three unit cases, upgrades the
evidence to schema v2 and Blog registry schema v11, and extends both the focused
and aggregate fail-closed guards. No Rust test, verifier, compile, database,
workflow, browser, or CI execution is recorded.

The continuation audit at `a7be516f87195e6b940f8b1aeb2bc5135c289b4e`
found that duplicate, out-of-order, missing-post recovery, and transactional
rollback remained prose-only runtime requirements after the pure unit harness was
added. Slice 45 adds an env-gated isolated PostgreSQL integration target with four
written cases, upgrades projection evidence to schema v3 and Blog registry to
schema v12, and retains the target in the focused and aggregate source gates. No
Rust test, verifier, compile, PostgreSQL, workflow, browser, or CI execution is
recorded.

The continuation audit at `069a9a7c74dbf0971c724d2f35ad0b9aa11d345b`
found that restart recovery was still represented only as an open runtime result.
Slice 46 adds a dedicated env-gated PostgreSQL target that replays one envelope
through a new database connection and a newly constructed handler over the same
durable delivery ledger. Projection evidence advances to schema v4 and Blog
registry to schema v13; focused, shared-chain, and aggregate guards retain the
new target without recording execution or claiming process-level restart proof.

The continuation audit at `3e0fa3a33ad0419591b4f7fa36924fbad1dcd354`
found that public Comments unavailability or timeout still failed the entire
selected-article request in both native SSR and GraphQL. Slice 47 adds typed
`AVAILABLE` / `UNAVAILABLE` / `TIMEOUT` transport parity, degrades only
`ExternalService` and `Timeout` to an empty comment payload, renders explicit
Leptos states while preserving the article, and extends the registered fallback
evidence plus focused fail-closed fixture. Blog registry schema v13 and package
order remain unchanged; no runtime or browser result is recorded.

The continuation audit at `df2e19a31098e1747441d513726fc9f21c82059e`
found that module listener registration was retained only as a production source
marker: no executable harness passed through `BlogModule::register_event_listeners`
and inspected the registered handler identity or routing behavior. Slice 48 adds
that module-level Rust target, extends projection evidence schema v4 and its
focused negative fixture, and preserves Blog registry schema v13 plus package
order. The target is source-only and does not claim actual host dispatcher, DB,
or process-level execution.

The continuation audit at `12f20d5e53b3f4a19ee9b1ab439900efdde3e33e`
found that module registration and routing were retained, but actual delivery
through `EventBus` and `EventDispatcher` still had no executable Blog target.
Slice 49 adds a filtered env-gated PostgreSQL dispatcher case through the real
module registration path, waits for the durable delivery marker, and asserts the
counter, ledger, and outbox commit. Evidence schema v4 and Blog registry schema
v13 remain compatible; focused guards retain the target without recording
execution.

The continuation audit at `733359d7102013fa91e5fc7df0dbd2ac8f919e06`
found that the optimistic counter loop still had no executable multi-connection
convergence target. Slice 50 adds four independent PostgreSQL connections and
handler instances synchronized by one barrier, delivers four unique envelopes to
one Blog post, and retains final counter/version plus ledger/outbox cardinality in
evidence and focused negative fixtures. It does not claim an observed retry count,
retry exhaustion, or PostgreSQL execution.

The continuation audit at `144332e78c2ba64cb64c7468a718778a0d0d9183`
found that restart evidence still stopped at a new connection and handler inside
one process. Slice 51 adds a filtered env-gated target that starts the integration
test executable twice in sequence, reconstructs the same envelope ID in each
child, and retains one durable application across the OS process boundary.
Evidence schema v4 and Blog registry schema v13 remain compatible; focused guards
retain the parent, worker, two child launches, exact command, and non-claim for a
full server-host restart without recording execution.

The continuation audit at `c4149652fa9c5835336daa29156b1d3e7ba68ac9`
found that the eight-attempt optimistic loop exposed a constant and final error but
had no shared source decision boundary tying success, retry, and terminal-limit
behavior to executable cases. Slice 52 introduces `ProjectionUpdateDecision`,
routes the production loop through it, and retains deterministic Rust cases for
immediate success plus seven retry decisions followed by the eighth-attempt
limit. Evidence schema v4 and Blog registry schema v13 remain compatible; no Rust
test, verifier, compile, PostgreSQL, browser, workflow, or CI result is recorded.

The source re-audit at current `main` commit
`763bd8ab2912af00b3c4034b5af13da3afbbfb46` rechecked all 52 recorded slices
against their source paths, evidence, focused verifiers, registry bindings, and
explicit non-claims. The recorded source artifacts remain present. No compile,
Rust/JavaScript test, PostgreSQL, Redis, browser, workflow, CI, or production
result was promoted. The remaining concrete projection gap was that the pure
retry decision proved the policy but no PostgreSQL target forced the real handler
to reach the eighth zero-row result and then demonstrated transaction rollback
plus same-envelope recovery.

Slice 53 adds the env-gated
`optimistic_retry_limit_rolls_back_and_replays_after_conflict_clears` target. A
schema-local trigger skips each update, a nontransactional sequence counts exactly
eight attempts, and the written assertions retain terminal failure, unchanged
post state, zero delivery/outbox rows, probe removal, and successful replay of the
same envelope. Evidence schema v4 and Blog registry schema v13 remain compatible;
focused source verification is updated, while execution and naturally contended
retry-frequency evidence remain pending.

The continuation audit at `e7224d7deecaecc8d8f0a10ca9423b2ee8b5c16c`
found that duplicate idempotency was retained only as sequential replay, while the
concurrency target used four different envelope identities. The production comment
explicitly relies on the delivery-ledger unique insert to roll back a losing
concurrent duplicate transaction, but no retained target forced two handlers past
the initial ledger lookup with the same envelope.

Slice 54 adds the env-gated
`concurrent_duplicate_envelope_commits_once_and_replays_cleanly` target. A control
transaction holds the Blog post row, two named workers pass the initial delivery
lookup and block on the optimistic update, and `pg_stat_activity` is used to prove
both are waiting before release. The written contract requires one winner, one
failed losing transaction, one post transition, one delivery row, one outbox row,
and a clean same-envelope replay. New schema-v1 evidence, a standalone fail-closed
verifier, and a focused negative fixture retain the source contract. Blog registry
schema v13 and its package order remain unchanged; no Rust, JavaScript,
PostgreSQL, dispatcher, workflow, or CI execution is recorded.

The continuation audit at `caa2d3fd49aa0fd9dad8bfd183c94d31808ace17`
found that slice 54's source artifacts were complete but intentionally remained
outside the registry-owned Blog FBA package order. The verifier and focused
self-test therefore had no named npm leaf commands, the registry did not bind the
evidence or PostgreSQL target, and aggregate chain drift could silently omit the
same-envelope race contract.

Slice 55 registers `comments_duplicate_delivery_race` as a first-class Blog FBA
source gate. It adds exact verify/test npm leaf commands immediately after the
main Comments event-projection gate, binds the schema-v1 evidence and PostgreSQL
target in registry schema v13, and locks the same order through the shared chain
policy and existing aggregate self-test fixture. Runtime code and the standalone
verifier are unchanged. No Rust test, JavaScript verifier, compile, PostgreSQL,
browser, workflow, CI, or production result is recorded.

The continuation audit at `ee8ec47cc151b4215d96d467880a238752dde3f5`
found that the real dispatcher path retained only one-envelope delivery, while
duplicate replay was proven either directly against the handler or through the
separate controlled race target. No retained source target published the same
envelope twice through `EventBus` and `EventDispatcher` and also proved that both
module-registered handler calls completed before final cardinality assertions.

Slice 56 adds the env-gated
`event_dispatcher_replays_duplicate_envelope_without_double_commit` target. It
wraps the real module-registered handler with completed-call and error counters,
publishes one envelope identity twice through the dispatcher, waits for exactly
two completed calls with zero errors, and requires one post transition, one
delivery row, and one outbox row. New schema-v1 evidence, a standalone fail-closed
verifier, and focused negative fixtures retain the dispatcher-level duplicate
delivery contract. Blog registry schema v13 and package order remain unchanged;
no Rust, JavaScript, PostgreSQL, browser, workflow, CI, or production execution is
recorded.

The continuation audit at `ca47b6153d09f3665d8a355894883496ccbf0a6e`
found that slice 56's source target, evidence, verifier, and focused fixture were
complete but remained outside the registry-owned Blog FBA chain. The standalone
verifier had no named npm leaf commands, the registry did not bind its evidence or
PostgreSQL target, and aggregate chain drift could silently omit dispatcher-level
duplicate replay.

Slice 57 registers `comments_dispatcher_duplicate_delivery` as a first-class Blog
FBA source gate immediately after `comments_duplicate_delivery_race`. It adds exact
verify/test npm leaf commands, binds schema-v1 evidence and the PostgreSQL target
in registry schema v13, and locks both aggregate orders through the shared chain
policy plus focused negative cases for step, command, and target drift. Runtime
code, the dispatcher target, and its standalone verifier are unchanged. No Rust
test, JavaScript verifier, compile, PostgreSQL, browser, workflow, CI, or
production execution is recorded.

The continuation audit at `bef0ffcce06551914175c00fa5e540aa4ceed720`
found that `CommentService` stored a transport-neutral `Arc<dyn CommentsThreadPort>`
but exposed only `new(db, event_bus)`, which always constructed the in-process
provider. A future host-owned remote adapter therefore had no public composition
seam even though all seven calls already routed through the trait boundary.

Slice 58 adds public `CommentService::with_comments_thread_port`, keeps `new` as
the in-process convenience path, and adds a compile-only exact-signature harness.
Comments consumer evidence advances to schema v3 and Blog registry schema v13
retains the harness under `contract_tests.adapter_injection`. The existing
fail-closed verifier now rejects constructor, harness, metadata, and unearned
runtime-status drift. No remote transport implementation, Rust/JavaScript test,
compile, database, browser, workflow, CI, or production execution is recorded.

The continuation audit at `99418b0ea424dbb56835ee61105de4294cb75337`
found that the new public injection seam was not yet consumed by the Blog HTTP
moderation runtime. `BlogHttpRuntime` retained only database and event-bus handles,
and the controller still constructed the in-process service directly, so a
host-provided Comments port could not reach the HTTP moderation surface.

Slice 59 adds optional `Arc<dyn CommentsThreadPort>` lookup from
`HostRuntimeContext`, centralizes injected/in-process selection in
`BlogHttpRuntime::comment_service`, and removes direct provider construction from
the moderation controller. New schema-v1 evidence, a standalone verifier, focused
negative fixtures, and a compile-only selector harness retain the HTTP composition
contract. GraphQL/native SSR wiring, package-chain registration, the remote
network transport, and all execution remain pending.

The continuation audit at `c1870d5473f0cecc2d95fa19fcd9377118a9a2d2`
found that the HTTP composition evidence, standalone verifier, focused fixture,
and compile-only selector harness were complete, but the registered
`comments_port_boundary` leaf did not execute either standalone JavaScript asset.
The HTTP contract could therefore drift while both Blog FBA package chains still
passed their existing source order.

Slice 60 imports the standalone HTTP verifier from the registered Comments port
verifier and imports its focused fixture from the registered Comments port
self-test. The aggregate Blog FBA self-test locks both import statements. Registry
schema v13, package scripts, and verify/test order remain unchanged because HTTP
composition is now a required sub-contract of the existing first-class Comments
port leaf rather than a parallel duplicate leaf. Runtime source, evidence, remote
transport status, and all execution claims remain unchanged.

The continuation audit at `8853b4431536ceb86dbe1d5be2090977105878af`
found that GraphQL public reads, moderation reads, and moderation mutation still
constructed `CommentService::new` directly. Blog also had no manifest runtime-data
factory, so host shared values could not enter the Blog GraphQL schema even though
the transport-neutral service constructor and generic server attachment mechanism
already existed.

Slice 61 adds manifest-declared `graphql::attach_schema_data`, materializes
`BlogGraphqlRuntimeData` from `GraphqlRuntimeInputs`, and centralizes injected or
in-process selection in `BlogGraphqlRuntimeData::comment_service`. All three
GraphQL Comments operations consume that schema data. New schema-v1 evidence, a
standalone verifier, twelve focused positive/negative cases, and a compile-only
factory/selector harness retain the complete host→generated-schema→resolver path.
Blog FBA package-chain integration, native SSR wiring, the remote network
transport, and all execution remain pending.

The continuation audit at `f0a342a8713fd9a29ce485e6e1481a7099810b77`
found that slice 61's GraphQL composition evidence, standalone verifier, focused
fixture, and compile-only harness were complete, but the registered
`comments_port_boundary` verifier and self-test still executed only their HTTP
composition sub-contracts. Both aggregate package chains could therefore omit the
GraphQL host→schema→resolver guard while retaining the same registry order.

Slice 62 imports the standalone GraphQL verifier from the registered Comments port
verifier and imports all twelve focused GraphQL cases from the registered Comments
port self-test. The aggregate Blog FBA self-test locks both imports beside the
existing HTTP assertions. Registry schema v13, package scripts, verify/test order,
runtime source, evidence statuses, remote transport status, and all execution
claims remain unchanged. GraphQL composition is now a mandatory sub-contract of
the existing first-class Comments port leaf rather than a parallel duplicate leaf.

The continuation audit at `f7ffe816328f051bc00dcb3efa29f6ccbc0d8055`
found that storefront native SSR still constructed `CommentService::new` directly
for the selected post's approved public comments even though it already received
`HostRuntimeContext`. The typed degradation path was complete, but a host-owned
Comments port could not reach the native server function.

Slice 63 re-exports the provider port contract through the Blog facade, adds the
storefront native `comment_service` selector over `HostRuntimeContext::shared_get`,
and routes the approved-only public read through injected or in-process
construction while preserving typed availability. New schema-v1 evidence, a
standalone verifier, focused positive/negative fixtures, and a compile-only
selector harness retain the source contract. Blog FBA package-chain integration,
admin native SSR composition, the remote network transport, and all execution
remain pending.

The continuation audit at `8ceec8eaa952162a825f25a5e664d41e5a06296f`
found that slice 63's storefront native composition evidence, standalone verifier,
focused fixture, and compile-only selector harness were complete, but the
registered `comments_port_boundary` verifier and self-test still imported only the
HTTP and GraphQL composition sub-contracts. Aggregate package-chain policy could
therefore omit storefront native host selection without changing registry order.

Slice 64 imports the standalone storefront native verifier from the registered
Comments port verifier and imports all sixteen focused storefront native cases
from the registered Comments port self-test. The parent positive fixture now
materializes the Blog facade, native selector, approved public operation, typed
degradation, harness, and schema-v1 evidence under the same temporary repo root.
The aggregate Blog FBA self-test locks both imports beside the existing HTTP and
GraphQL assertions. Registry schema v13, package scripts, verify/test order,
runtime source, evidence statuses, remote transport status, and all execution
claims remain unchanged. Storefront native composition is now a mandatory
sub-contract of the existing first-class Comments port leaf rather than a parallel
duplicate leaf.

The continuation audit at `ffd1c3f47335d7809856737e922b3402c68a9a5c`
found that both Blog admin native moderation endpoints still constructed
`CommentService::new` directly even though `native_context()` already received
`HostRuntimeContext`. A host-owned Comments port could therefore reach HTTP,
GraphQL, and storefront native consumers but not admin native SSR.

Slice 65 adds optional `Arc<dyn rustok_blog::CommentsThreadPort>` storage to
`NativeContext`, reads it through `HostRuntimeContext::shared_get`, and routes both
moderation list and moderation mutation through one injected/in-process
`comment_service` selector. Tenant binding, `blog_posts:manage`, pagination, and
fail-closed error propagation are preserved. New schema-v1 evidence, a standalone
verifier, seventeen focused positive/negative cases, and a compile-only selector
harness retain the source contract. Blog FBA package-chain integration, the remote
network transport, and all execution remain pending.

The continuation audit at `f6cd8b598d2ff57e19ec50a1455bf8d8bb31a972`
found that slice 65's admin native evidence, standalone verifier, focused fixture,
and compile-only harness were complete, but the registered
`comments_port_boundary` verifier and self-test still imported only HTTP, GraphQL,
and storefront native composition sub-contracts. The audit also found storefront
native evidence still carrying the now-stale `admin_native_ssr_composition`
pending marker.

Slice 66 imports the standalone admin native verifier from the registered Comments
port verifier and imports all seventeen focused admin native cases from the
registered Comments port self-test. The parent positive fixture now materializes
the admin `NativeContext`, selector, tenant/permission admission, pagination,
fail-closed handoffs, harness, and schema-v1 evidence under the same temporary
repo root. The aggregate Blog FBA self-test locks both imports beside the existing
HTTP, GraphQL, and storefront assertions. Storefront evidence and its sixteen-case
fixture now retain only the remote transport as pending and reject reintroduction
of the stale admin marker. Registry schema v13, package scripts, verify/test order,
runtime source, remote transport status, and all execution claims remain
unchanged. Admin native composition is now a mandatory sub-contract of the
existing first-class Comments port leaf rather than a parallel duplicate leaf.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Richtext UI evidence: the shared Next and Leptos adapters now receive the
  owner-selected article locale separately from host UI messages, propagate
  derived direction and spellcheck, and follow dynamic form busy/read-only
  state. Next edit hydration uses the post's requested/effective translation
  locale and reserves the host UI locale for new posts. `npm run verify
  --prefix packages/richtext`, the Chromium frame
  harness, native Blog admin check and shared WASM check cover this slice;
  authenticated Next storefront now also has mounted canonical comment
  submission and moderation-pending PostgreSQL persistence evidence. The
  isolated Leptos Blog comment island builds as real WASM and retains the
  authenticated native server-function path without full storefront hydration.
  Mounted Blog article save/reload, Leptos comment persistence/reload and
  rejection, and dirty locale-switch evidence remain open.
- Blog FBA source-gate chain: `source_verified_no_compile`; registry schema v13
  locks exact verify/test order, source-gate paths, leaf npm commands, evidence,
  self-tests, the Comments projection classifier, deterministic retry policy,
  PostgreSQL retry-limit rollback/replay, concurrent duplicate-delivery race, and
  dispatcher duplicate-delivery targets, host registration, dispatcher,
  concurrency, PostgreSQL, same-process restart, and process-restart harnesses,
  plus aggregate/consumer bindings for admin, storefront, Comments port boundary,
  Comments event projection, category Search reindex, GraphQL rate limiting,
  GraphQL richtext, AI richtext, Forum ownership, and runtime
  order. The existing Comments port leaf requires HTTP, GraphQL, storefront
  native, and admin native composition verifiers plus focused fixtures through
  aggregate-locked imports; package order remains unchanged.
- Comments consumer port boundary: Blog-owned `source_verified_no_compile` for
  the in-process profile; evidence schema v3, all seven operations, approved public
  read, typed richtext projection, two-second deadlines, write idempotency, active
  typed `PortErrorKind` mapping, public transport-neutral injection constructor,
  facade port re-export, compile-only exact-signature harness, exact npm leaf
  commands, focused fixture, and Blog FBA ordering are locked. HTTP moderation
  selects an optional host-provided port through `BlogHttpRuntime::comment_service`;
  GraphQL public/moderation operations use manifest-attached
  `BlogGraphqlRuntimeData`; storefront native SSR approved public reads use
  `comment_service` with typed `AVAILABLE` / `UNAVAILABLE` / `TIMEOUT`
  degradation; admin native SSR moderation list and mutation use
  `comment_service` with tenant/permission admission and fail-closed errors. All
  four composition guards and their focused fixtures are retained inside the
  registered Comments port gate. Dedicated parallel composition leaves remain
  intentionally absent. The remote network transport, remote adapter runtime
  parity, cached snapshot, comment-form fallback, browser/runtime evidence, and
  broader degraded UI modes remain planned or pending.
- Comments event projection: Blog-owned `source_verified_no_compile`; evidence
  schema v4, shared classifier/counter/retry-decision helpers, `executable_no_run`
  source harness, deterministic PostgreSQL retry-limit target, module-registration,
  dispatcher, concurrency, PostgreSQL transaction, same-process restart, and
  process-restart targets, verifier, focused self-test, exact npm leaf commands,
  delivery-ledger identity, transactional outbox markers, and Blog FBA ordering
  are locked. Separate schema-v1 first-class source gates retain the concurrent
  same-envelope race and dispatcher duplicate replay. The race gate requires both
  named workers to pass their initial ledger lookups before one commits and the
  loser rolls back; the dispatcher gate requires two completed module-registered
  handler calls with zero errors and one database application. Their exact npm
  leaf commands and order immediately after the main projection gate are locked.
  All four PostgreSQL integration targets passed locally on 2026-08-01 (12 cases
  total) after their isolated schemas were hardened to forbid fallback to
  `public`. No retained executable output was generated, so evidence promotion,
  naturally contended PostgreSQL retry frequency, full server-host restart
  recovery, and retained execution evidence remain pending.
- Load protection: `implementation_ready`; mounted Redis evidence is pending.
- Rate-limit harness: `executable_no_compile`; evidence, verifier, self-test,
  npm leaf commands, and aggregate FBA registration are locked; execution is
  maintainer-owned.
- Search Blog projection harness: Search-owned `executable_no_run`; evidence,
  verifier, focused fixture, exact npm leaf commands, test targets, and Search FBA
  ordering are locked. Routing and PostgreSQL execution remain maintainer-owned.
- Blog article richtext storage: `implemented_source_verified_no_compile`; the
  initial schema is canonical and the source gate forbids corrective migration
  and conversion artifacts.
- Next admin Forum UI ownership: `source_verified_no_compile`.
- Blog admin canonical richtext guardrail: `source_verified_no_compile`.
- Blog GraphQL richtext boundary: `implemented_source_verified_no_compile`.
- Blog storefront richtext boundary: `next_mounted_postgresql_verified_leptos_wasm_built`;
  Leptos mounted persistence parity and rejection/reload evidence remain pending.
- AI Blog draft owner writes and shim: `source_verified_no_compile`.
- Comments thread write invariants: Comments-owned `executable_no_run`; registry
  schema v4, evidence schema v3, owner identity classifier, guarded canonical
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
- `crates/rustok-blog/contracts/evidence/blog-comments-http-port-injection.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-graphql-port-injection.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-storefront-native-port-injection.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-admin-native-port-injection.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-event-projection.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-duplicate-delivery-race.json`
- `crates/rustok-blog/contracts/evidence/blog-comments-dispatcher-duplicate-delivery.json`
- `crates/rustok-blog/rustok-module.toml`
- `crates/rustok-blog/src/lib.rs`
- `crates/rustok-blog/src/controllers/mod.rs`
- `crates/rustok-blog/src/controllers/comments.rs`
- `crates/rustok-blog/src/services/comment.rs`
- `crates/rustok-blog/src/graphql/mod.rs`
- `crates/rustok-blog/src/graphql/runtime_data.rs`
- `crates/rustok-blog/src/graphql/types.rs`
- `crates/rustok-blog/src/graphql/mutation.rs`
- `apps/server/build.rs`
- `apps/server/src/graphql/schema.rs`
- `crates/rustok-blog/storefront/src/model.rs`
- `crates/rustok-blog/storefront/src/transport/graphql_adapter.rs`
- `crates/rustok-blog/storefront/src/transport/native_server_adapter.rs`
- `crates/rustok-blog/storefront/src/ui/leptos.rs`
- `crates/rustok-blog/admin/src/transport/native_server_adapter.rs`
- `crates/rustok-blog/src/services/comment_projection.rs`
- `crates/rustok-blog/tests/comment_projection_postgres_test.rs`
- `crates/rustok-blog/tests/comment_projection_duplicate_race_postgres_test.rs`
- `crates/rustok-blog/tests/comment_projection_dispatcher_duplicate_postgres_test.rs`
- `crates/rustok-blog/tests/comment_projection_restart_postgres_test.rs`
- `crates/rustok-comments/contracts/comments-fba-registry.json`
- `crates/rustok-comments/contracts/evidence/comments-thread-write-invariants.json`
- `crates/rustok-blog/contracts/evidence/blog-graphql-rate-limit-runtime-harness.json`
- `crates/rustok-blog/contracts/evidence/blog-category-search-reindex-contract.json`
- `crates/rustok-blog/contracts/evidence/blog-graphql-richtext-boundary.json`
- `crates/rustok-blog/contracts/evidence/blog-storefront-richtext-view.json`
- `crates/rustok-blog/contracts/evidence/blog-ai-richtext-boundary.json`
- `crates/rustok-blog/contracts/evidence/blog-forum-ui-ownership.json`
- `crates/rustok-blog/contracts/evidence/blog-admin-richtext-boundary.json`
- `crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`
- `crates/rustok-search/contracts/evidence/search-canonical-url-contract.json`
- `scripts/verify/verify-blog-comments-port-boundary.mjs`
- `scripts/verify/verify-blog-comments-port-boundary.test.mjs`
- `scripts/verify/verify-blog-comments-http-port-injection.mjs`
- `scripts/verify/verify-blog-comments-http-port-injection.test.mjs`
- `scripts/verify/verify-blog-comments-graphql-port-injection.mjs`
- `scripts/verify/verify-blog-comments-graphql-port-injection.test.mjs`
- `scripts/verify/verify-blog-comments-storefront-native-port-injection.mjs`
- `scripts/verify/verify-blog-comments-storefront-native-port-injection.test.mjs`
- `scripts/verify/verify-blog-comments-admin-native-port-injection.mjs`
- `scripts/verify/verify-blog-comments-admin-native-port-injection.test.mjs`
- `scripts/verify/verify-blog-comments-event-projection.mjs`
- `scripts/verify/verify-blog-comments-event-projection.test.mjs`
- `scripts/verify/verify-blog-comments-duplicate-delivery-race.mjs`
- `scripts/verify/verify-blog-comments-duplicate-delivery-race.test.mjs`
- `scripts/verify/verify-blog-comments-dispatcher-duplicate-delivery.mjs`
- `scripts/verify/verify-blog-comments-dispatcher-duplicate-delivery.test.mjs`
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
24. Consolidated the pre-release initial schema and deleted corrective
    migration/conversion artifacts.
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
44. Extracted a shared pure Comments event classifier and saturating counter
    transition, added three Rust unit cases, upgraded event evidence to schema v2
    and Blog registry schema v11, and extended focused/aggregate source guards
    without recording execution.
45. Added an env-gated isolated PostgreSQL event-projection target for duplicate,
    delete-before-create, missing-post replay, and outbox rollback/retry behavior;
    upgraded projection evidence to schema v3 and Blog registry schema v12, and
    retained the target in focused/aggregate source gates without running it.
46. Added an env-gated PostgreSQL restart target that replays the same envelope
    through a new database connection and newly constructed handler, upgraded
    projection evidence to schema v4 and Blog registry schema v13, and retained
    the target in focused/shared/aggregate gates without recording execution.
47. Added typed storefront public-comments availability across GraphQL and native
    SSR, degraded only external-service and timeout failures while propagating all
    other Blog errors, rendered explicit Leptos unavailable/timeout states, and
    extended fallback evidence plus focused negative fixtures without changing
    Blog registry schema v13 or recording runtime execution.
48. Added an executable-no-run module registration and routing harness through
    `BlogModule::register_event_listeners`, retained handler identity and Blog-only
    lifecycle routing in projection evidence schema v4 and focused negative
    fixtures, and kept registry schema v13, package order, dispatcher execution,
    database delivery, and process-level recovery unchanged or pending.
49. Added an executable-no-run PostgreSQL dispatcher case that registers the Blog
    projection through `BlogModule`, publishes through `EventBus` and
    `EventDispatcher`, waits for the durable delivery marker, and retains
    counter/ledger/outbox assertions in evidence plus focused negative fixtures
    without changing registry schema v13 or recording execution.
50. Added an executable-no-run four-connection PostgreSQL convergence case for
    unique `comment.created` envelopes on one post, synchronized start with a
    shared barrier, and retained final counter/version plus ledger/outbox
    cardinality in evidence and focused fail-closed fixtures without claiming
    measured retries, exhaustion, or execution.
51. Added an executable-no-run process restart target that launches the integration
    test executable twice with one durable envelope identity, retained the parent,
    private worker, exact two-launch boundary, final counter/ledger/outbox
    assertions, evidence metadata, and focused negative fixtures without claiming
    a full server-host restart or recording execution.
52. Added a shared pure optimistic retry decision used by the production projection
    loop, retained immediate-success and seven-retry/eighth-limit Rust cases in
    evidence plus focused fail-closed fixtures, and kept PostgreSQL-observed retry
    behavior, registry schema v13, package order, and all execution unchanged or
    pending.
53. Re-audited all recorded Blog slices on current `main`, added an env-gated
    PostgreSQL retry-limit target that forces eight real zero-row handler updates,
    retained atomic rollback and same-envelope recovery in evidence and the focused
    guard, and kept natural contention frequency plus all execution pending.
54. Added an env-gated same-envelope concurrent delivery target with a controlled
    row lock and two named workers, retained one winner, losing-transaction
    rollback, final ledger/outbox cardinality, and clean replay in schema-v1
    evidence plus a standalone verifier and focused negative fixture, and kept
    registry package order plus all execution unchanged or pending.
55. Registered the same-envelope duplicate-delivery race as a first-class Blog FBA
    source gate, added exact verify/test npm leaf commands, bound its evidence and
    PostgreSQL target in registry schema v13, and locked aggregate verify/test
    order without changing runtime code or recording execution.
56. Added an env-gated dispatcher duplicate-delivery target that routes the same
    envelope twice through the module-registered handler, observes two successful
    completed calls with zero errors, and retains one counter/delivery/outbox
    application in schema-v1 evidence plus a standalone verifier and focused
    negative fixture without changing registry package order or recording
    execution.
57. Registered the dispatcher duplicate-delivery target as a first-class Blog FBA
    source gate, added exact verify/test npm leaf commands after the concurrent
    duplicate-delivery race gate, bound its evidence and PostgreSQL target in
    registry schema v13, and locked aggregate verify/test order without changing
    runtime code or recording execution.
58. Added a public transport-neutral Comments port injection constructor while
    preserving the in-process convenience path, retained its exact trait-object
    signature in an executable-no-run Rust harness, advanced consumer evidence to
    schema v3, registered the harness in Blog registry schema v13, and extended the
    existing fail-closed verifier and focused fixture without implementing or
    claiming a remote transport.
59. Wired optional host-provided `CommentsThreadPort` selection into Blog HTTP
    moderation, centralized injected/in-process construction in
    `BlogHttpRuntime::comment_service`, retained the selector in schema-v1 evidence
    plus a standalone verifier, focused negative fixture, and compile-only harness,
    and kept GraphQL/native SSR composition, package registration, remote transport,
    and all execution pending.
60. Bound the standalone HTTP composition verifier and focused fixture into the
    already registered `comments-port-boundary` verify/test leaf, added aggregate
    regressions for both required imports, preserved registry schema v13 and exact
    package order, and changed no runtime source or execution status.
61. Added manifest-attached `BlogGraphqlRuntimeData`, carried the optional
    host-owned `CommentsThreadPort` through generated schema data, routed public
    and moderation reads plus moderation mutation through one injected/in-process
    selector, retained the full host attachment path in schema-v1 evidence,
    standalone verification, twelve focused cases, and a compile-only harness,
    and kept package registration, native SSR wiring, remote transport, and all
    execution pending.
62. Bound the standalone GraphQL composition verifier and all twelve focused cases
    into the already registered `comments-port-boundary` verify/test leaf, added
    aggregate regressions for both required imports, preserved registry schema v13
    and exact package order, and changed no runtime source or execution status.
63. Re-exported the Comments port contract through the Blog facade, wired optional
    host-provided port selection into storefront native SSR approved public reads,
    preserved typed availability/error propagation, retained the selector in
    schema-v1 evidence plus a standalone verifier, focused fixtures, and a
    compile-only harness, and kept package integration, admin SSR, remote transport,
    and all execution pending.
64. Bound the standalone storefront native composition verifier and all sixteen
    focused cases into the already registered `comments-port-boundary` verify/test
    leaf, made the parent positive fixture self-contained, added aggregate
    regressions for both required imports, preserved registry schema v13 and exact
    package order, and changed no runtime source or execution status.
65. Wired optional host-provided `CommentsThreadPort` selection into Blog admin
    native SSR moderation list and mutation, centralized injected/in-process
    construction in `comment_service(&NativeContext)`, preserved tenant binding,
    manage permission, bounded pagination, and fail-closed errors, retained the
    source contract in schema-v1 evidence plus a standalone verifier, seventeen
    focused cases, and a compile-only harness, and kept package integration,
    remote transport, and all execution pending.
66. Bound the standalone admin native composition verifier and all seventeen
    focused cases into the already registered `comments-port-boundary` verify/test
    leaf, made the parent positive fixture self-contained, added aggregate
    regressions for both required imports, reconciled storefront evidence to remove
    the stale admin pending marker, preserved registry schema v13 and exact package
    order, and changed no runtime source or execution status.
67. Executed all four Blog Comments projection PostgreSQL targets locally against
    PostgreSQL 16, fixed their unique-schema isolation by removing the `public`
    search-path fallback, and added focused fail-closed verifier regressions. The
    12 integration cases pass; retained runtime evidence and status promotion
    remain pending.
68. Added the Blog-owned `createBlogComment` GraphQL mutation and
    `blog/comment-create` native server function over one transport-neutral
    storefront request/result contract. Both paths require authenticated
    `comments:create`, exact tenant scope, enabled Blog tenant/channel state,
    and a published post visible in the current channel before invoking the
    Comments port. The Leptos Blog detail now composes the reusable
    `rustok-comments-storefront-support::CommentComposer`; the component owns
    editor/form state and receives no arbitrary Comments target identity.
69. Added the matching Comments-owned React composer and composed it into the
    selected Next Blog detail surface. The detail renders the Blog-provided safe
    HTML projection and approved comment previews, while the write passes the
    exact post id only to the Blog-owned GraphQL command. Shared Next frame
    route handling now lives in `@rustok/richtext/next` and is reused by both
    Next hosts. Next storefront, Next admin, and richtext package typechecks
    pass. Removed the superseded Blog service create method that checked only
    post existence, so all Blog-owned comment creation now uses the single
    public target policy. A focused integration test proves draft and hidden
    channel rejection followed by a pending write for the visible channel;
    mounted auth/save/rejection/reload evidence remains pending.
70. Reconciled the remaining executable Search PostgreSQL fixture with the
    canonical Blog schema by deleting its `body_format` column and Markdown
    value and storing a valid richtext document directly in `body`. The focused
    Search projection target compiles. Blog module validation now passes after
    synchronizing its required `outbox` dependency and correcting the validator
    so always-linked required modules do not require invented `mod-*` features.
71. Mounted the authenticated Next Blog detail against the built server and a
    fresh PostgreSQL schema. The selected article rendered the server-owned
    richtext projection, the Comments-owned iframe exposed the generated
    `comment` toolbar, and submission persisted the canonical root document in
    `comment_bodies` with `pending` moderation state. The Next host now owns the
    same-origin `/api/*` rewrite, and the server and every client agree on the
    canonical no-trailing-slash `/api/graphql` route.
72. Added the isolated Leptos Blog comment island. Selected article SSR emits a
    localized inert marker and a CSP-nonced same-origin bootstrap; only an active
    canonical browser session loads the minimal WASM artifact and shared Tiptap
    frame. The shared authorized server-function client supplies bearer and
    tenant headers to `blog/comment-create`, and the full storefront graph stays
    outside the island build. The real wasm32 artifact build passes. Shared WASM
    build and embedded-asset response helpers remove duplication with Pages;
    the Content-owned frame router is generated from the canonical richtext asset
    manifest so Leptos does not duplicate Next route logic or Blog asset ownership.

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
5. **Close comments runtime evidence.** Run the registered Comments port boundary
   verifier and self-test, which include the standalone HTTP, GraphQL, storefront
   native, and admin native composition verifiers plus their focused fixtures.
   Execute the compile-only injection-signature, HTTP selection, GraphQL
   runtime-data, storefront native, and admin native selector harnesses, shared
   consumer runtime-order verifier, Blog projection classifier and deterministic
   retry policy, module registration/routing, dispatcher, concurrency, retry-limit,
   duplicate-delivery, and restart targets. Retain deterministic rollback,
   cardinality, process-boundary, authorization, moderation, pagination, and
   fail-closed outputs; then implement the remote network transport through
   `CommentService::with_comments_thread_port`, and retain all-seven-operation
   adapter parity, naturally contended PostgreSQL retry-frequency evidence, full
   server-host restart recovery, browser parity for typed unavailable/timeout
   article rendering, cached thread snapshots, selected-path comment-form
   failures, approved-only reads, moderation, pagination, first-thread identity, and
   unrelated insert storage error propagation.
6. **Execute and retain canonical Blog article evidence.** Apply all migrations
   from zero on PostgreSQL and SQLite, then retain Next/Leptos, GraphQL/native,
   AI draft persistence, Search reindex, and browser save/reload evidence on the
   same commit. No conversion or corrective migration path may be restored.
7. **Complete public comment storefront parity.** The Comments-owned React
   composer is connected to the selected Next Blog detail surface through the
   implemented Blog-bound GraphQL mutation; mounted Next authentication,
   canonical submission, and moderation-pending persistence now pass. Retain
   hidden/draft/channel rejection and save/reload evidence for Next, plus the
   full mounted Leptos path. Do not add a generic Comments target mutation.

## Verification

Execution is intentionally not recorded by this source-only update. Maintainers
should run the relevant subset, including:

- `npm run verify:blog:comments-port-boundary`
- `npm run test:verify:blog:comments-port-boundary`
- `cargo test -p rustok-blog --lib services::comment::port_injection_tests::comment_service_accepts_an_injected_comments_thread_port -- --exact`
- `node scripts/verify/verify-blog-comments-http-port-injection.mjs`
- `node --test scripts/verify/verify-blog-comments-http-port-injection.test.mjs`
- `cargo test -p rustok-blog --lib controllers::tests::blog_http_runtime_exposes_comments_port_selection -- --exact`
- `node scripts/verify/verify-blog-comments-graphql-port-injection.mjs`
- `node --test scripts/verify/verify-blog-comments-graphql-port-injection.test.mjs`
- `cargo test -p rustok-blog --lib graphql::runtime_data::tests::graphql_runtime_data_exposes_comments_port_selection -- --exact`
- `node scripts/verify/verify-blog-comments-storefront-native-port-injection.mjs`
- `node --test scripts/verify/verify-blog-comments-storefront-native-port-injection.test.mjs`
- `cargo test -p rustok-blog-storefront --features ssr transport::native_server_adapter::tests::storefront_native_runtime_exposes_comments_port_selection -- --exact`
- `node scripts/verify/verify-blog-comments-admin-native-port-injection.mjs`
- `node --test scripts/verify/verify-blog-comments-admin-native-port-injection.test.mjs`
- `cargo test -p rustok-blog-admin --features ssr transport::native_server_adapter::tests::admin_native_runtime_exposes_comments_port_selection -- --exact`
- `npm run verify:blog:comments-event-projection`
- `npm run test:verify:blog:comments-event-projection`
- `npm run verify:blog:comments-duplicate-delivery-race`
- `npm run test:verify:blog:comments-duplicate-delivery-race`
- `npm run verify:blog:comments-dispatcher-duplicate-delivery`
- `npm run test:verify:blog:comments-dispatcher-duplicate-delivery`
- `cargo test -p rustok-blog --lib services::comment_projection::tests`
- `cargo test -p rustok-blog --lib services::comment_projection::tests::optimistic_retry_policy_applies_success_without_retry -- --exact`
- `cargo test -p rustok-blog --lib services::comment_projection::tests::optimistic_retry_policy_allows_seven_retries_then_stops_on_eighth_conflict -- --exact`
- `cargo test -p rustok-blog --lib tests::module_registers_comment_projection_handler_with_host_routing`
- `RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test event_dispatcher_routes_registered_handler_and_commits_projection -- --exact`
- `RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_dispatcher_duplicate_postgres_test event_dispatcher_replays_duplicate_envelope_without_double_commit -- --exact`
- `RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test concurrent_created_events_converge_without_lost_updates -- --exact`
- `RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test optimistic_retry_limit_rolls_back_and_replays_after_conflict_clears -- --exact`
- `RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_duplicate_race_postgres_test concurrent_duplicate_envelope_commits_once_and_replays_cleanly -- --exact`
- `RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test`
- `RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_restart_postgres_test restarted_process_reuses_delivery_ledger_without_reapplying_counter -- --exact`
- `RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_restart_postgres_test`
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
- `cargo check -p rustok-blog-storefront --features ssr`
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
