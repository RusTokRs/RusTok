# rustok-search implementation plan

## 2026-10-25 Phase 4 — Forum storefront legacy retirement

The specialized Forum storefront pipeline is gone; the generic declared-corpus path is the
only storefront Search path. No compatibility bridge remains.

- [x] Deleted `forum_storefront_execution.rs`, `forum_storefront_execution_public.rs`,
      `storefront_result_eligibility.rs`, `forum_document_filters.rs`,
      `forum_current_channel_filter.rs` and `graphql/forum_storefront.rs` from this crate,
      plus `transport/forum_graphql_adapter.rs` and
      `transport/forum_native_server_adapter.rs` from `rustok-search-storefront`. Module
      declarations, `pub use` re-exports, GraphQL schema wiring and storefront transport
      selection were pruned in the same change.
- [x] Removed `ExecuteForumStorefrontSearch`/`execute_forum_storefront_search`, the
      `forumStorefrontSearch` GraphQL operation, `StorefrontSearchResultEligibilityPort`
      (type, shared alias, resolver, candidate kind) and
      `MAX_FORUM_SEARCH_RESULT_CANDIDATES`. §14 of `AGENTS.md` forbids keeping a stub, and
      a `#[deprecated]` bridge over the same SPI is a stub with a comment.
- [x] `StorefrontSearchCategoryScopePort` lost its Forum name: it now declares
      `source_module()` and `expand_category_scope()`, and
      `resolve_storefront_search_category_ids` consults it only when the request scope is
      exactly that single module. A missing capability, or a scope naming any other
      module, keeps the caller's exact `category_ids` instead of failing the request —
      category scope is an enrichment, not an authorization decision.
- [x] The host Forum policy
      (`apps/server/src/services/forum_search_result_eligibility.rs`) implements
      `SearchAudiencePolicyPort` and nothing else; it owns status, category and
      topic/reply authorization and converts neutral `SearchCandidateDescriptor`s by
      entity type.
- [x] The five `apps/server/tests/forum_versioned_invalidation_*` integration tests were
      migrated to `execute_audience_eligible_search` with a locally composed
      `SearchAudienceRegistry`, not deleted and not re-pointed at a bridge.
- [x] `execute_audience_eligible_search` now takes its transport boundary as
      `StorefrontSearchAudienceContext` (registry, optional owner category-scope
      capability, auth, request context, transport). A caller can no longer forget one
      security input by miscounting positional arguments, and a future capability is a
      field instead of a sixth parameter.
- [x] Category scope became a real part of the generic path instead of a legacy-only
      feature: the executor consults it after the discovery preflight, uses the locale
      the request actually runs in (explicit query locale, else the host-resolved request
      locale) and never invents a default. An owner that resolves a requested root to no
      visible category short-circuits to an empty result — previously the empty
      `category_ids` fell through to "no category predicate", i.e. the whole corpus.
      `audience_security.rs` regression test:
      `category_scope_resolved_to_nothing_returns_empty_result`.
- [x] `FORUM_SEARCH_SOURCE_MODULE` moved out of `rustok-search` to its owner
      (`rustok_forum::FORUM_MODULE_SLUG`), so the engine crate exports no per-module named
      constant.
- [x] Nine `scripts/verify/verify-forum-search-*.mjs` guards that pinned the deleted
      surface were removed together with their contract JSONs; four contracts that
      described only the deleted pipeline were deleted as well; the survivors point at the
      generic owner and `verify-search-audience-classes.mjs`, and that verifier now fails
      when any retired file or symbol reappears, when a Forum Search contract names a
      missing path or a retired symbol, or when the category-scope capability loses its
      `source_module` gate.
- [x] Guards that pinned the same legacy markers were moved to the durable spelling:
      `verify-forum-anonymous-discovery`, the three LINK-FORUM-03 proofs, the two
      versioned-invalidation proofs (`deletion_acl_ordering`, `normal_delivery`), the
      rebuild scope-preservation guard (now pinning one projector instead of the retired
      `projector_legacy` name) and the projection-reconciliation status guard.

## 2026-10-25 Declared Search audience corpus

The audience mechanism was audited end to end and rebuilt around declared audience
classes ([ADR 2026-10-25](../../../../DECISIONS/2026-10-25-search-audience-declared-corpus.md)):

- [x] `SearchAudienceRegistry` declares each source module exactly once as
      `SearchAudienceClass::Public` or `SearchAudienceClass::Gated`; duplicate
      declarations are `conflict` errors and `with_policy` (panic + silent overwrite)
      is removed.
- [x] Undeclared modules are denied: they never pass through as public candidates and
      `public_discovery_allowed` answers `false` for them.
- [x] GraphQL `storefrontSearch` / `storefrontSearchSuggestions` and the native SSR
      adapter fail closed when the host did not compose the registry; `apps/server`
      propagates declaration and registration failures instead of `let _ =`.
- [x] The GraphQL schema now receives the registry as request data
      (`apps/server/src/graphql/schema.rs`). Search is a required module, so the manifest
      `runtime_data_factory` hook is not wired for it; without the explicit attachment the
      storefront fields resolved no registry at all and failed closed on every call.
- [x] `apps/server` owns the corpus inventory in `search_public_source_modules()`
      (`blog`, `commerce`, `docs`, `pages`, `content` plus
      `rustok_content::CONTENT_NODE_KINDS`) and registers Forum as gated in the same
      composition step. `content` is the projector's fallback bucket for an empty stored
      kind.
- [x] Content node kinds are a single closed list: `projector_core` writes
      `source_module = node.kind`, so `CONTENT_NODE_KINDS` is both the write allowlist and
      the public corpus declaration. Before this, the projector emitted `post`/`page`/
      `article`/`custom`/`block`/`menu`/`menu_item` while the host declared `content`, and
      every published CMS document would have become invisible in public Search.
- [x] `SearchAudiencePolicyPort::public_discovery_allowed` is a required method; no
      permissive default remains.
- [x] Candidate authorization is batched (`MAX_SEARCH_AUDIENCE_CANDIDATES` per call) and
      the scan is bounded by `MAX_SEARCH_AUDIENCE_SCAN_DOCUMENTS`; the former
      100-candidate *request* rejection is gone and public modules are never bounded.
- [x] One scoped SQL query (`source_module NOT IN (gated) OR document_id IN (authorized)`)
      produces totals, facets, ordering and pagination; `build_authorized_facets`, the
      `forum_visible_status` fallback and the duplicated facet vocabulary are removed.
- [x] `PortError` keeps its taxonomy across the SPI boundary: `NotFound`/`Forbidden`
      exclude a module, `Unavailable`/`Timeout` fail the request, `InvariantViolation`
      is internal.
- [x] Storefront suggestions take `StorefrontSuggestionAudience`, apply the declared
      module scope, the anonymous discovery preflight, and per-candidate authorization.
- [x] `exclude_forum`, `source_module <> 'forum'` and the exclusion special case are
      removed from `pg_engine`; the engine receives only `SearchAudienceDocumentScope`.
- [x] Query rules cannot inject a pinned document while a gated module is in scope, and
      pinned documents pass payload channel visibility for every entity type.
- [x] `npm run verify:search:audience` guards the declared corpus, the fail-closed
      boundaries, the neutral engine scope and suggestion parity.

Follow-up completed on 2026-10-25: physical removal of the legacy Forum storefront
files and of `graphql/forum_storefront.rs` (see the Phase 4 entry above).

Open follow-up:

- [ ] Decide whether Blog indexing keeps the SQL projector in this crate or switches to
      the Blog-owned `SearchProjectionSource`; today the module-side source is composed
      but not consulted by `SearchIngestionHandler`, and the two implementations must
      stay semantically identical.

## 2026-10-16 Audience Security SPI and Projection Source Decoupling

The Search module generalizes cross-module authorization and ingestion boundaries:
1. **Audience Security SPI (`audience_security.rs`):**
   - Introduced `SearchAudiencePolicyPort` and `SearchAudienceRegistry` replacing hard-coded Forum audience couplings with a generic port-based registry.
   - Domain modules implement `SearchAudiencePolicyPort` directly (`ServerForumSearchResultEligibilityPort`), registered directly in `apps/server/src/services/mod.rs`.
   - Wired `SearchAudienceRegistry` into GraphQL `storefrontSearch` and native SSR `storefront_search_native`, providing real-time multi-module candidate post-authorization.
   - Historical Forum endpoints (`execute_forum_storefront_search` and `StorefrontSearchResultEligibilityPort`) were preserved at first as `#[deprecated]` adapter bridges over the generic SPI, satisfying ADR `2026-10-16-generic-search-audience-security-spi.md` (§3) and the then-current contract verification suites. They were physically removed on 2026-10-25 (Phase 4) once every caller, verifier and contract moved to the generic SPI.
2. **Search Projection Sources (`SearchProjectionSourceRegistry`):**
   - Generalizes projection ingestion from domain modules via `SearchProjectionSource` and `SearchProjectionSourceFactory`.
   - `SearchIngestionHandler::with_projection_sources` accepts both `blog_source` and `forum_source` registered via module runtime extensions.

## 2026-09-24 Blog projection destructive-operation guard

The Blog Search projector now validates the complete Blog/Taxonomy source-table contract before
deleting any existing Blog search documents during full or targeted upsert. It also propagates `document_key` decode failures while refreshing canonical article bodies; a malformed cursor cannot silently stop the batch scan and commit only a prefix of the rebuilt projection. A missing source table,
a missing availability row, or a database decode failure is an error, so the surrounding transaction
rolls back instead of committing an empty replacement. This keeps temporary schema availability
failures from destroying the last known-good Search projection. The Blog projection verifier and
focused fixture enforce the guard ordering and reject silent availability decode fallbacks.

## FFA/FBA status

- FFA status: `phase_b_ready`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`

## Contract and error policy completion

- [x] Expand capability matrix and contract tests
- [x] Finalize search-facing error catalog and validation policy

## Current state

`rustok-search` owns normalized search documents, PostgreSQL FTS, catalog, Blog,
and Forum projection ingestion, analytics, dictionaries, query rules,
rebuild/diagnostics, and module-owned admin/storefront surfaces. It remains
separate from `rustok-index`; consumers depend on published Search contracts
rather than index runtime types. The FFA split is `phase_b_ready` with focused
core, transport, and UI packages.

Canonical result navigation has a single owner policy:
`canonical_search_result_url` in `crates/modules/rustok-search/src/engine.rs`. It derives
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
`crates/modules/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`,
guarded by `scripts/verify/verify-search-blog-projection.mjs` and focused fixture
`scripts/verify/verify-search-blog-projection.test.mjs`. Exact commands
`verify:search:blog-projection` and `test:verify:search:blog-projection` run after
the canonical URL leaf in the Search FBA verify/test chains. The aggregate guard
locks their paths, commands, order, test targets, and `executable_no_run` status;
PostgreSQL execution remains maintainer-owned.

Forum public category, topic, and approved-reply projections support exact
category filtering through the existing bounded `category_ids` query field.
Complete Forum category-subtree filtering is not part of that field: an exact id matches
only the documents Forum projects with that identifier, and subtree expansion happens in
the Forum-owned scope port below.
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

> Historical note: the dedicated Forum-only path described in this section was removed
> on 2026-10-25 (Phase 4). The durable design is the declared-corpus path at the top of
> this file; the owner port that survives is `SearchAudiencePolicyPort`, and category
> scope survives as the module-neutral `StorefrontSearchCategoryScopePort`.

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

`FORUM-23B2G1` adds a durable Forum inbox ingest sequence issued by PostgreSQL.
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
from trusted `RequestContext`; caller-provided `channel_id` is now only a compatibility
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
retain the previous non-storefront path. Product channel visibility remains blocked
on that evidence: the source boundary is delivered, the maintainer-executed PostgreSQL
and transport proof is not.

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
  `crates/modules/rustok-search/contracts/search-fba-registry.json`.
- Static provider evidence:
  `crates/modules/rustok-search/contracts/evidence/search-contract-test-static-matrix.json`.
- Executable provider fallback evidence:
  `crates/modules/rustok-search/contracts/evidence/search-runtime-fallback-smoke.json`.
- Executable provider contract evidence:
  `crates/modules/rustok-search/contracts/evidence/search-runtime-contract-smoke.json`.
- Provider invocation evidence:
  `crates/modules/rustok-search/contracts/evidence/search-runtime-invocation-trace.json`.
- Canonical URL status: `source_verified_no_compile`.
- Canonical URL evidence:
  `crates/modules/rustok-search/contracts/evidence/search-canonical-url-contract.json`.
- Canonical URL guardrail:
  `scripts/verify/verify-search-canonical-url-contract.mjs`.
- Canonical URL focused fixture:
  `scripts/verify/verify-search-canonical-url-contract.test.mjs`.
- Canonical URL leaf commands: `verify:search:canonical-url` and
  `test:verify:search:canonical-url`; both are locked into the Search FBA package
  chains by `scripts/verify/verify-search-fba.mjs`.
- Blog projection evidence:
  `crates/modules/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`.
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
  `crates/modules/rustok-forum/contracts/forum-search-exact-category-filter.json`.
- Exact Forum category filter guardrail:
  `scripts/verify/verify-search-audience-classes.mjs`.
- Forum category-subtree owner status: `source_complete_execution_pending`.
- Forum richer-audience subtree contract:
  `crates/modules/rustok-forum/contracts/forum-search-category-audience-scope.json`.
- Forum-only storefront Search composition status:
  `source_complete_execution_pending`.
- Forum-only storefront Search contract and guardrail:
  `crates/modules/rustok-forum/contracts/forum-search-storefront-scope.json` and
  `scripts/verify/verify-search-audience-classes.mjs`.
- Forum topic/reply result eligibility status:
  `source_complete_execution_pending` under `FORUM-23B2D`.
- Forum result eligibility contract and guardrail:
  `crates/modules/rustok-forum/contracts/forum-search-result-eligibility.json` and
  `scripts/verify/verify-search-audience-classes.mjs`.
- Trusted storefront channel authority status:
  `source_complete_execution_pending` under `FORUM-23B2E1`.
- Trusted channel contract and guardrail:
  `crates/modules/rustok-forum/contracts/forum-search-trusted-channel-authority.json` and
  `scripts/verify/verify-search-audience-classes.mjs`.
- Product channel visibility status:
  `source_complete_execution_pending` under `FORUM-23B2E2`.
- Product channel visibility contract and guardrail:
  `crates/modules/rustok-forum/contracts/forum-search-product-channel-visibility.json` and
  `scripts/verify/verify-search-audience-classes.mjs`.
- Exact Forum author filter status:
  `source_complete_execution_pending` under `FORUM-23B2F1`.
- Exact Forum author filter contract and guardrail:
  `crates/modules/rustok-forum/contracts/forum-search-author-filter.json` and
  `scripts/verify/verify-search-audience-classes.mjs`.
- Exact Forum tag and solved filter status:
  `source_complete_execution_pending` under `FORUM-23B2F2`.
- Exact Forum tag and solved contract and guardrail:
  `crates/modules/rustok-forum/contracts/forum-search-tag-solved-filter.json` and
  `scripts/verify/verify-search-audience-classes.mjs`.
- Exact Forum locale and date filter status:
  `source_complete_execution_pending` under `FORUM-23B2F3`.
- Exact Forum locale/date contract and guardrail:
  `crates/modules/rustok-forum/contracts/forum-search-locale-date-filter.json` and
  `scripts/verify/verify-search-audience-classes.mjs`.
- Trusted current-channel Forum filter status:
  `source_complete_execution_pending` under `FORUM-23B2F4`.
- Trusted current-channel contract and guardrail:
  `crates/modules/rustok-forum/contracts/forum-search-current-channel-filter.json` and
  `scripts/verify/verify-search-audience-classes.mjs`.
- Durable Forum inbox ingest-sequence status:
  `source_complete_execution_pending` under `FORUM-23B2G1`.
- Durable ingest-sequence contract and guardrail:
  `crates/modules/rustok-forum/contracts/forum-search-durable-ingest-sequence.json` and
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
[Media and Search Extraction Boundaries](../../../../DECISIONS/2026-07-16-media-search-extraction-boundaries.md).

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
2. **Add owner-issued Forum projection revisions.** Carry owner-issued monotonic aggregate
   revisions in versioned invalidation events and reconcile them with the delivered
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
- `cargo test -p rustok-search category_filter -- --nocapture`
- `cargo test -p rustok-search storefront_category_scope -- --nocapture`
- `cargo test -p rustok-search audience_security -- --nocapture`
- `cargo test -p rustok-search storefront_product_channel_visibility -- --nocapture`
- `cargo test -p rustok-search product_channel_visibility_legacy_projection_is_detected -- --nocapture`
- `cargo test -p rustok-search product_channel_reconciliation -- --nocapture`
- `cargo test -p rustok-server --features mod-forum forum_search_category_scope -- --nocapture`
- `cargo test -p rustok-server --features mod-forum forum_search_result_eligibility -- --nocapture`
- `node scripts/verify/verify-search-audience-classes.mjs`
- `node scripts/verify/verify-search-audience-classes.mjs`
- `node scripts/verify/verify-search-audience-classes.mjs`
- `node scripts/verify/verify-search-audience-classes.mjs`
- `node scripts/verify/verify-search-audience-classes.mjs`
- `node scripts/verify/verify-search-audience-classes.mjs`
- `node scripts/verify/verify-search-audience-classes.mjs`
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


## Blog projection tenant and ownership boundary

The Blog Search projector binds every owner relation to the source post tenant, including author enrichment: `users.id = blog_posts.author_id` is insufficient by itself, so the projection also requires `users.tenant_id = blog_posts.tenant_id`. This keeps the read model aligned with the platform tenant invariant even when malformed cross-tenant owner references exist. The PostgreSQL projection harness includes a negative cross-tenant author case, while runtime execution remains maintainer-owned.


## Search projector source-of-truth boundary

The public `SearchProjector` facade delegates to the canonical `projector_core.rs` implementation. The previous `projector_legacy.rs` filename was retired because this code is current implementation, not compatibility or legacy behavior. `ensure_bootstrap` also fails closed when the database count row cannot be decoded: a missing row is the only condition that yields zero and can trigger bootstrap rebuild; database/schema decoding errors propagate instead of being converted into an empty index signal.


## 2026-09-24 Search bootstrap failure boundary

Both layers of the SearchProjector bootstrap path now propagate database count-row decoding errors. A missing count row yields no row and therefore zero; a present row that cannot be decoded is an infrastructure/schema error and must not be converted into an empty-index signal or trigger a destructive rebuild. The Search FBA verifier enforces this in the canonical projector core and facade.


## 2026-09-24 Search GraphQL error-boundary hardening

A fresh cross-surface error audit found that Search GraphQL adapters exposed `rustok_core::Error::to_string()` directly for database, external-service, serialization, cache, scripting, and internal failures. That leaked persistence/driver details through the GraphQL boundary and violated the module's own operational-error policy.

The Search GraphQL surface now has one owner-local error adapter in `graphql/mod.rs`. Caller-safe validation, not-found, authentication, and permission failures retain typed GraphQL codes; infrastructure and internal failures are mapped to the generic `INTERNAL_ERROR` message while detailed errors remain in server-side telemetry/logging. Query, mutation, and Forum Search reconciliation paths use the same adapter, eliminating transport-specific error drift. Regression tests cover redaction and typed public codes.

Runtime, build, gatekeeper, and automated tests remain maintainer-owned and unrun by the agent.
