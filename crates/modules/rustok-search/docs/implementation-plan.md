# rustok-search implementation plan

## FFA/FBA status

- FFA status: `phase_b_ready`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`

## Current state

`rustok-search` owns normalized search documents, PostgreSQL FTS, catalog, Blog,
and Forum projection ingestion, analytics, dictionaries, query rules,
rebuild/diagnostics, and module-owned admin/storefront surfaces. It remains
separate from `rustok-index`; consumers depend on published Search contracts
rather than index runtime types. The FFA split is `phase_b_ready` with focused
core, transport, and UI packages.

Canonical result navigation has a single owner policy:
`canonical_search_result_url` in `crates/rustok-search/src/engine.rs`. It derives
product, content, Blog, and Forum URLs from normalized `SearchResultItem` values
before transport serialization. Blog navigation requires the canonical
`source_module=blog` / `entity_type=blog_post` pair and a bounded ASCII slug from
the owner-projected payload. Forum navigation requires canonical Forum
source/entity pairs and validates reply payload identity. Missing, malformed,
spoofed, traversal, whitespace, and overlong values fail closed. Content
source-module values are bounded before they enter a query string.

GraphQL Search, storefront native Search, Search admin preview, and admin global
search all delegate to this single owner policy. The storefront transport facade
returns the selected transport payload unchanged: there is no transport fallback,
no post-processing navigation module, and no local Blog or Forum route builder.
The Search admin native adapter is split into focused include-parts for API
facade, read handlers, write handlers, normalization, execution pipeline,
mapping, and support. Only the mapping part converts normalized results to admin
DTOs, and it delegates URL resolution to `canonical_search_result_url`.

The canonical URL fixture now mirrors the complete current verifier contract,
including Forum category/topic/reply routes, reply identity checks, admin Forum
permission gates, and all evidence cases. The exact leaf commands
`verify:search:canonical-url` and `test:verify:search:canonical-url` are part of
the Search FBA verify/test chains; `verify-search-fba.mjs` rejects command, order,
fixture, or evidence drift. This remains source-only evidence and does not record
runtime execution.

Blog ingestion has two executable, unrun harness layers. A routing target locks
Blog lifecycle, module-toggle, and targeted/full reindex events. An env-gated
PostgreSQL target creates an isolated schema, runs Search migrations, projects
real Blog source rows through `SearchIngestionHandler`, verifies lifecycle and
payload replacement, checks tenant-scoped full rebuild, targeted missing-post
cleanup, and module-disabled cleanup followed by enable-time rebuild. Source-table
availability resolves through the active PostgreSQL `search_path` instead of
hard-coding `public`.

The retained Blog projection evidence is
`crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`,
guarded by `scripts/verify/verify-search-blog-projection.mjs` and focused fixture
`scripts/verify/verify-search-blog-projection.test.mjs`. Exact commands
`verify:search:blog-projection` and `test:verify:search:blog-projection` run after
the canonical URL leaf in the Search FBA verify/test chains. The aggregate guard
locks their paths, commands, order, test targets, and `executable_no_run` status;
PostgreSQL execution remains maintainer-owned.

Forum public category, topic, and approved-reply projections support exact
category filtering through the existing bounded `category_ids` query field.
Product queries retain normalized `index_product_categories` matching. Forum
category documents match their document identifier, while topic and reply
documents match the owner-projected `facets.category_id`.

The source-ready Forum-only storefront Search path composes two neutral optional
owner ports without importing Forum into Search. `StorefrontSearchCategoryScopePort`
expands the richer Forum category audience, while `FORUM-23B2D` adds
`StorefrontSearchResultEligibilityPort` for exact topic-local and reply
reauthorization. GraphQL `forumStorefrontSearch` and native
`search/forum-storefront-search` share one Search execution owner. It rejects a
raw result set above 100 rows, sends only typed topic/reply candidates to Forum,
preserves allowed ranking order, and computes visible totals, facets, offset, and
limit after owner eligibility. Mixed, unspecified, Product, Blog, Content, and
Forum-without-category requests remain on the existing exact-category
`storefrontSearch` path.

`FORUM-23B2F1` adds an exact bounded Forum author filter without changing the
neutral `SearchQuery`, `SearchPreviewInput`, or shared storefront filter DTO.
GraphQL carries an optional Forum-specific argument and an additive native
author endpoint carries at most ten UUIDs without changing the existing native
endpoint signature. Search
matches only the existing public `payload.author.user_id` projection on Forum
topics and replies, excludes categories and missing/redacted authors while the
filter is active, then applies exact Forum owner eligibility before visible totals,
facets and pagination. The stable raw 100-candidate cap remains before author
narrowing. Query-rule pins are disabled for an active author scope so a pinned
document cannot escape the requested author set. Ordinary, mixed, Product and
admin Search paths remain unchanged. Runtime evidence remains pending.

`FORUM-23B2F2` adds exact bounded tag and solved-state filters to the same
explicit Forum-only execution owner. Tag values are trimmed, case-sensitive,
exact and intersect with AND semantics. Topics use `payload.tags`; approved
replies use Forum-projected parent `payload.topic_tags`. Solved topics require a
valid UUID or explicit null in `solution_reply_id`, while replies use the exact
current boolean `is_solution` marker; malformed projected values fail closed.
The raw 100-candidate cap remains before narrowing, all active document filters
intersect before owner eligibility, visible totals/facets/pagination are computed
after authorization, and query-rule pins remain disabled under any active document
filter. Legacy replies without `topic_tags` fail closed for tag-scoped queries until
a Forum Search reindex. Existing GraphQL/native legacy and author-only operations,
neutral DTOs, mixed/Product/admin Search and unfiltered behavior remain unchanged.
Runtime and reindex evidence remain pending.

`FORUM-23B2F3` adds a shared Forum-only locale/date execution owner while
preserving the legacy, author-only and B2F2 GraphQL/native wire operations. The requested
locale or tenant fallback is normalized once and becomes the exact PostgreSQL
FTS/typo locale, category-scope locale, owner-eligibility locale and post-scan
result assertion. Optional inclusive RFC3339 `published_from` / `published_to`
bounds use Forum-projected `payload.published_at` on topics and approved replies.
The stable raw 100-candidate cap remains before date narrowing; exact locale and
active author/tag/solved/date predicates intersect before owner eligibility and
visible totals/facets/pagination. Locale-only execution retains categories and
query-rule pins, while an active date window excludes categories and suppresses
pins. Legacy topic/reply documents without the new timestamp fail closed until a
Forum Search reindex. Neutral DTOs, `SearchQuery`, mixed/Product/admin Search and
existing wire signatures remain unchanged. Runtime and reindex evidence remain
pending.

`FORUM-23B2F4` adds an optional exact trusted-current-channel filter to the
same explicit Forum-only execution owner. Topics match their Forum-projected
`channel_slugs`; approved replies inherit `topic_channel_slugs` from the parent
topic. The filter accepts no arbitrary channel input, excludes global topics and
categories, runs before owner eligibility/totals/facets/pagination and suppresses
pins. Existing transports remain unchanged while additive current-channel
GraphQL/native operations carry the boolean. Arbitrary channel/group selection,
future topic kinds and attachment presence remain blocked on their Forum owner
contracts. Runtime and reindex evidence remain pending.

`FORUM-23B2G1` adds a durable PostgreSQL-issued Forum inbox ingest sequence.
Existing rows are deterministically backfilled, new successful inserts receive a
positive unique sequence, and claim, retry blocking, due-tenant order plus scope
watermarks use that value instead of producer timestamps and event UUIDs.
Envelope `revision_at` and `event_id` remain identity/diagnostic fields, author
redaction barriers remain unskippable, and event schemas, Forum writes, rebuilds,
public APIs and storefront query behavior remain unchanged. This is not the final
Forum-owner-issued projection revision; that owner contract and rollout
reconciliation remain pending. Runtime evidence remains pending.

Search settings have one owner boundary. Tenant-effective settings are read and
written through `SearchSettingsService` and the `search_settings` table. The
server-wide generic `platform_settings` service no longer admits a `search`
category, does not serialize bootstrap `search.api_key`, filters historical generic
Search rows from list responses, and rejects generic Search reads and writes before
database access.

`FORUM-23B2E1/B2E2` close the storefront channel authority and Product visibility
source boundary. GraphQL and native storefront Search derive channel ID and slug
from trusted `RequestContext`; caller-provided `channel_id` is only a compatibility
assertion. Search projects Product-owned
`metadata.channel_visibility.allowed_channel_slugs`, hides missing or malformed
projections, and applies one storefront-only predicate before FTS or typo ranking.
Rows, totals, facets, attribute-filtered queries, query-rule pins and document
suggestions therefore share the trusted channel decision. Storefront query-text
suggestions are disabled because aggregate logs cannot be channel-authorized;
admin/global query suggestions remain unchanged. A Search-owned bounded
reconciler and host startup worker repair missing legacy Product projections in
PostgreSQL batches. Malformed explicit owner values remain hidden until Product is
corrected instead of entering an endless rebuild loop. Admin preview/global Search
retain the previous non-storefront path. Runtime evidence remains pending.

The durable projection contract is also incomplete. The generic
`search_projection_inbox` and watermark schema exists, but only Forum events use
it and its background reconciler. Content, Product, Blog, locale, tenant, and
ordinary reindex work can still run directly through the process-local event
handler. Terminal handler failure or broadcast lag is logged but has no durable
Search consumer receipt, automatic retry lane, or guaranteed source rebuild, so a
projection can remain stale after recovery.

## FFA/FBA status

- FFA status: `phase_b_ready`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Provider contracts: `SearchQueryPort` and `SearchSuggestionPort` in
  `crates/rustok-search/contracts/search-fba-registry.json`.
- Static provider evidence:
  `crates/rustok-search/contracts/evidence/search-contract-test-static-matrix.json`.
- Executable provider fallback evidence:
  `crates/rustok-search/contracts/evidence/search-runtime-fallback-smoke.json`.
- Executable provider contract evidence:
  `crates/rustok-search/contracts/evidence/search-runtime-contract-smoke.json`.
- Provider invocation evidence:
  `crates/rustok-search/contracts/evidence/search-runtime-invocation-trace.json`.
- Canonical URL status: `source_verified_no_compile`.
- Canonical URL evidence:
  `crates/rustok-search/contracts/evidence/search-canonical-url-contract.json`.
- Canonical URL guardrail:
  `scripts/verify/verify-search-canonical-url-contract.mjs`.
- Canonical URL focused fixture:
  `scripts/verify/verify-search-canonical-url-contract.test.mjs`.
- Canonical URL leaf commands: `verify:search:canonical-url` and
  `test:verify:search:canonical-url`; both are locked into the Search FBA package
  chains by `scripts/verify/verify-search-fba.mjs`.
- Blog projection evidence:
  `crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`.
- Blog projection guardrail and fixture:
  `scripts/verify/verify-search-blog-projection.mjs` and
  `scripts/verify/verify-search-blog-projection.test.mjs`.
- Blog projection leaf commands: `verify:search:blog-projection` and
  `test:verify:search:blog-projection`; exact commands and aggregate order are
  locked by `scripts/verify/verify-search-fba.mjs`.
- Blog projection harness status: `executable_no_run`; execution remains user-owned
  and requires `RUSTOK_SEARCH_TEST_DATABASE_URL` or PostgreSQL `DATABASE_URL`.
- Exact Forum category filter status: `source_complete_execution_pending`.
- Exact Forum category filter contract:
  `crates/rustok-forum/contracts/forum-search-exact-category-filter.json`.
- Exact Forum category filter guardrail:
  `scripts/verify/verify-forum-search-exact-category-filter.mjs`.
- Forum category-subtree owner status: `source_complete_execution_pending`.
- Forum richer-audience subtree contract:
  `crates/rustok-forum/contracts/forum-search-category-audience-scope.json`.
- Forum-only storefront Search composition status:
  `source_complete_execution_pending`.
- Forum-only storefront Search contract and guardrail:
  `crates/rustok-forum/contracts/forum-search-storefront-scope.json` and
  `scripts/verify/verify-forum-search-storefront-scope.mjs`.
- Forum topic/reply result eligibility status:
  `source_complete_execution_pending` under `FORUM-23B2D`.
- Forum result eligibility contract and guardrail:
  `crates/rustok-forum/contracts/forum-search-result-eligibility.json` and
  `scripts/verify/verify-forum-search-result-eligibility.mjs`.
- Trusted storefront channel authority status:
  `source_complete_execution_pending` under `FORUM-23B2E1`.
- Trusted channel contract and guardrail:
  `crates/rustok-forum/contracts/forum-search-trusted-channel-authority.json` and
  `scripts/verify/verify-forum-search-trusted-channel-authority.mjs`.
- Product channel visibility status:
  `source_complete_execution_pending` under `FORUM-23B2E2`.
- Product channel visibility contract and guardrail:
  `crates/rustok-forum/contracts/forum-search-product-channel-visibility.json` and
  `scripts/verify/verify-forum-search-product-channel-visibility.mjs`.
- Exact Forum author filter status:
  `source_complete_execution_pending` under `FORUM-23B2F1`.
- Exact Forum author filter contract and guardrail:
  `crates/rustok-forum/contracts/forum-search-author-filter.json` and
  `scripts/verify/verify-forum-search-author-filter.mjs`.
- Exact Forum tag and solved filter status:
  `source_complete_execution_pending` under `FORUM-23B2F2`.
- Exact Forum tag and solved contract and guardrail:
  `crates/rustok-forum/contracts/forum-search-tag-solved-filter.json` and
  `scripts/verify/verify-forum-search-tag-solved-filter.mjs`.
- Exact Forum locale and date filter status:
  `source_complete_execution_pending` under `FORUM-23B2F3`.
- Exact Forum locale/date contract and guardrail:
  `crates/rustok-forum/contracts/forum-search-locale-date-filter.json` and
  `scripts/verify/verify-forum-search-locale-date-filter.mjs`.
- Trusted current-channel Forum filter status:
  `source_complete_execution_pending` under `FORUM-23B2F4`.
- Trusted current-channel contract and guardrail:
  `crates/rustok-forum/contracts/forum-search-current-channel-filter.json` and
  `scripts/verify/verify-forum-search-current-channel-filter.mjs`.
- Durable Forum inbox ingest-sequence status:
  `source_complete_execution_pending` under `FORUM-23B2G1`.
- Durable ingest-sequence contract and guardrail:
  `crates/rustok-forum/contracts/forum-search-durable-ingest-sequence.json` and
  `scripts/verify/verify-forum-search-durable-ingest-sequence.mjs`.
- GraphQL and all native/admin mappings use the same Search-owned URL function.
- The removed storefront `transport/navigation.rs` path is forbidden by the
  canonical URL guardrail.
- Transport-local `derive_search_result_url`, `derive_admin_search_result_url`,
  `enrich_search_result_urls`, and Blog route constants are forbidden.
- Generic `platform_settings/search` is forbidden by the Search FBA guard.
- Blog projection table discovery and reads use the active `search_path`.
- `TenantModuleToggled(blog, false)` deletes only the current tenant Blog search
  scope; enabling the module rebuilds it from retained owner rows.
- Targeted Blog reindex deletes stale documents before source lookup, so a missing
  owner post cannot leave obsolete search data behind.
- Trusted storefront channel authority is `source_complete_execution_pending`
  under `FORUM-23B2E1`.
- Product channel visibility is `source_complete_execution_pending` under
  `FORUM-23B2E2`.
- Exact Forum author filtering is `source_complete_execution_pending` under
  `FORUM-23B2F1`.
- Exact Forum tag and solved filtering is `source_complete_execution_pending` under
  `FORUM-23B2F2`.
- Exact Forum locale and date filtering is `source_complete_execution_pending` under
  `FORUM-23B2F3`.
- Trusted current-channel Forum filtering is `source_complete_execution_pending`
  under `FORUM-23B2F4`.
- Durable Forum inbox ingest ordering is `source_complete_execution_pending` under
  `FORUM-23B2G1`; Forum-owner-issued revisions remain pending.
- Durable non-Forum projection replay/recovery remains `blocked`.

## Deployment and connector boundary

Search is a whole-module extraction pilot. Remote deployment contains the complete
`rustok-search` owner, including `SearchEngine`, ranking, dictionaries, query
rules, analytics, URL policy, and PostgreSQL baseline. Storefront and admin
consumers call normalized Search contracts and never construct application routes.
The extraction boundary follows
[Media and Search Extraction Boundaries](../../../DECISIONS/2026-07-16-media-search-extraction-boundaries.md).

Meilisearch, Typesense, and Algolia remain connector implementations inside the
Search service. They receive canonical `SearchQuery` and document inputs and
return normalized `SearchResult` and suggestion DTOs. Connector results must pass
through Search-owned mapping before transport serialization.

`rustok-index` remains a separate ingestion/read-model owner. Query-time reads of
index-owned category and attribute tables should move to Search-owned denormalized
fields before database isolation. Search continues to own event ingestion and
rebuild behavior through replayable event transport.

## Completed implementation slices

1. Added Blog lifecycle Search projection, targeted/full reindex, module-toggle
   handling, stale cleanup, and tenant isolation.
2. Added isolated-schema PostgreSQL Blog projection harnesses and active
   `search_path` discovery.
3. Added `canonical_search_result_url` with product, content, Blog, and Forum
   routing, bounded payload validation, source/entity ownership, and
   query-injection guards.
4. Exported the owner policy and migrated GraphQL result projection.
5. Migrated storefront native result projection to the owner policy.
6. Removed storefront post-transport navigation enrichment and deleted its source
   and focused verifier fixtures.
7. Migrated Search admin preview mapping to the owner policy.
8. Migrated admin global search to the owner policy and admitted canonical
   `blog_post` results through the Blog read permission.
9. Split the Search admin native adapter into focused source parts while preserving
   its public transport API.
10. Added current-only machine-readable evidence and negative fixtures that reject
    every transport-local URL implementation and require no transport fallback.
11. Added canonical URL ownership to the standard Search FBA gate alongside the
    provider port, fallback, runtime contract, and invocation evidence.
12. Removed the generic `platform_settings/search` execution path and added a
    fail-closed FBA ownership guard so runtime connector secrets stay inside the
    Search owner boundary.
13. Reused bounded `category_ids` for exact Forum category, topic, and approved-reply
    filtering while preserving product category relations, parameterized SQL, and
    the shared FTS/typo filter path.
14. Reconciled the canonical URL fixture with the expanded Forum/admin verifier,
    added exact verify/test leaf commands, and locked both commands plus their
    order into the Search FBA aggregate package chains.
15. Registered the existing Blog projection harness as a first-class Search FBA
    leaf, added exact verify/test commands, and bound evidence, verifier, fixture,
    test targets, status, and aggregate order without recording PostgreSQL execution.
16. Added the neutral Forum category-scope port, host adapter, explicit GraphQL and
    native Forum-only storefront Search paths, and one Search execution owner while
    preserving the existing mixed/product storefront Search behavior.
17. Added the neutral Forum result-eligibility port, Forum exact topic/reply owner,
    host adapter, bounded 100-row candidate scan, and post-authorization totals,
    facets, offset, and limit under `FORUM-23B2D`.
18. Added the Search-owned trusted storefront channel authority and bound ordinary
    plus Forum-only GraphQL/native Search to middleware `RequestContext`; the
    legacy public `channel_id` is assertion-only under `FORUM-23B2E1`.
19. Projected canonical Product channel allowlists, added bounded startup repair
    for missing legacy projections, and applied one storefront predicate to FTS,
    typo fallback, rows, totals, facets, query-rule pins and document suggestions
    under `FORUM-23B2E2`.
20. Added the exact bounded Forum author filter on public projected author identity,
    optional GraphQL plus additive native transport parity, pre-eligibility narrowing,
    post-filter totals/facets/pagination, and active-scope pin suppression under
    `FORUM-23B2F1`.
21. Added exact bounded Forum tag and solved-state filters, parent-topic tag
    projection for approved replies, additive GraphQL/native filter operations,
    pre-eligibility intersection, post-authorization totals/facets/pagination, and
    fail-closed legacy reply behavior under `FORUM-23B2F2`.
22. Added exact requested/fallback locale enforcement and inclusive RFC3339
    published date-window filtering through an additive Forum-only execution owner,
    Forum-owned topic/reply timestamp projection, post-scan locale assertion and
    fail-closed legacy projection behavior under `FORUM-23B2F3`.
23. Added trusted current-channel Forum narrowing, parent-topic channel projection
    for approved replies, additive transport parity and transactional topic-update
    invalidation under `FORUM-23B2F4`.
24. Added a PostgreSQL-issued immutable Forum inbox ingest sequence, deterministic
    existing-row backfill, sequence-based claims/due-tenant ordering and completed
    sequence watermarks under `FORUM-23B2G1`.

## Next results

1. **Complete owner-backed Forum storefront query filters.** Add arbitrary
   channel/group selection only through an exact authorized Forum owner contract;
   add topic kinds after `FORUM-22` and attachment presence after `FORUM-14`.
   **Done when:** no caller-selected audience identifier or owner policy is copied
   into Search and every result still passes exact post-retrieval eligibility.
2. **Add owner-issued Forum projection revisions.** Carry a monotonic Forum-owned
   revision in versioned invalidation events and reconcile it with the delivered
   Search ingest sequence during rolling deployment. **Done when:** source revision
   watermarks reject stale owner state independently of delivery order.
3. **Generalize durable Search projection recovery.** Use the existing generic
   inbox/watermark schema for Content, Product, Blog, locale, tenant, and reindex
   events; add bounded retry, dead-letter diagnostics, ordered replay, restart and
   lag recovery, and source-of-truth rebuild evidence.
   **Done when:** terminal handler failure, transport lag, duplicate/out-of-order
   delivery, and process restart cannot leave a stale projection without an
   observable durable recovery action.
4. **Execute Forum Search eligibility evidence.** Run the neutral port tests,
   Forum owner scenarios, GraphQL/native composition, PostgreSQL candidate/result
   proof, denied topic/reply cases, broad-query failure, and `LINK-FORUM-03` after
   projection ordering is stable.
5. **Execute canonical URL evidence.** Run core URL-policy tests, GraphQL
   storefront Search, native storefront Search, Search admin preview, and admin
   global search against projected product, content, Blog, and Forum documents.
   Retain proof that malformed owner payloads remain non-navigable everywhere.
6. **Verify click analytics.** Confirm every Search surface records the canonical
   href without reconstructing routes in analytics code.
7. **Execute live Blog projection evidence.** Run routing and PostgreSQL harnesses
   and retain migration/`pg_trgm`, event-delivery, targeted missing-post cleanup,
   module-disable cleanup, and category reindex results.
8. **Execute live provider evidence.** Run query and suggestion providers under
   deadline, error, locale, tenant, channel, ranking, and catalog-filter conditions.
9. **Add external engines only as adapters.** Meilisearch, Typesense, or Algolia
   connectors must not bypass Search ports, owner URL mapping, or PostgreSQL
   baseline selection.

## Verification

Execution is intentionally not recorded by this source-only update. Maintainers
should run the relevant subset, including:

- `cargo test -p rustok-server settings_service`
- `cargo test -p rustok-search engine::tests::canonical_url`
- `cargo test -p rustok-search category_filter_preserves_product_and_adds_exact_forum_scope -- --nocapture`
- `cargo test -p rustok-search storefront_category_scope -- --nocapture`
- `cargo test -p rustok-search storefront_result_eligibility -- --nocapture`
- `cargo test -p rustok-search forum_document_filters -- --nocapture`
- `cargo test -p rustok-search forum_storefront_locale_date_filters -- --nocapture`
- `cargo test -p rustok-search storefront_product_channel_visibility -- --nocapture`
- `cargo test -p rustok-search product_channel_visibility_legacy_projection_is_detected -- --nocapture`
- `cargo test -p rustok-search product_channel_reconciliation -- --nocapture`
- `cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture`
- `cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture`
- `cargo test -p rustok-server --features mod-forum forum_search_category_scope -- --nocapture`
- `cargo test -p rustok-server --features mod-forum forum_search_result_eligibility -- --nocapture`
- `node scripts/verify/verify-forum-search-exact-category-filter.mjs`
- `node scripts/verify/verify-forum-search-storefront-scope.mjs`
- `node scripts/verify/verify-forum-search-result-eligibility.mjs`
- `node scripts/verify/verify-forum-search-product-channel-visibility.mjs`
- `node scripts/verify/verify-forum-search-author-filter.mjs`
- `node scripts/verify/verify-forum-search-tag-solved-filter.mjs`
- `node scripts/verify/verify-forum-search-locale-date-filter.mjs`
- `npm run verify:search:canonical-url`
- `npm run test:verify:search:canonical-url`
- `npm run verify:search:blog-projection`
- `npm run test:verify:search:blog-projection`
- `cargo test -p rustok-search --test blog_ingestion_contract_test`
- `RUSTOK_SEARCH_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-search --test blog_projection_postgres_test`
- `npm run verify:search:fba`
- `npm run test:verify:search:fba`
- `npm run verify:search:ui-boundary`
- `cargo xtask module validate search`

## References

- [Crate README](../README.md)
- [Search documentation](./README.md)
- [Search FBA registry](../contracts/search-fba-registry.json)
- [Forum exact-category contract](../../rustok-forum/contracts/forum-search-exact-category-filter.json)
- [Forum result-eligibility contract](../../rustok-forum/contracts/forum-search-result-eligibility.json)
- [Forum author-filter contract](../../rustok-forum/contracts/forum-search-author-filter.json)
- [Forum tag/solved contract](../../rustok-forum/contracts/forum-search-tag-solved-filter.json)
- [Forum locale/date contract](../../rustok-forum/contracts/forum-search-locale-date-filter.json)
