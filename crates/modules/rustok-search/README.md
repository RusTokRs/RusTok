# rustok-search

## Purpose

`rustok-search` owns normalized search, projection ingestion, ranking, result
navigation, dictionaries, analytics, diagnostics, and provider contracts for
RusToK.

## Responsibilities

- Provide `SearchModule` metadata for the runtime registry.
- Keep PostgreSQL as the default search engine and own connector selection.
- Own module-local `search_documents`, settings, dictionaries, query rules,
  analytics, diagnostics, and rebuild behavior.
- Execute PostgreSQL FTS with `tsvector`, `websearch_to_tsquery`, `ts_rank_cd`,
  highlights, typo-tolerant fallback, filters, sorting, and facets.
- Interpret the bounded `category_ids` query field across owner projections:
  normalized product-category relations remain authoritative for products, while
  public Forum category/topic/reply documents use their exact projected category
  identity. Owner subtree expansion remains outside the Search engine.
- Consume product-owned high-load projections from `rustok-index` without
  exposing index runtime types to Search consumers.
- Consume Blog lifecycle and scoped rebuild events into Search-owned documents.
- Own canonical application URLs for normalized `SearchResultItem` values through
  `canonical_search_result_url`.
- Serve storefront Search, admin preview, and host global-search capabilities.
  GraphQL storefront Search enforces the no-tenant-override surface policy
  before performing dictionary, settings or index reads, rejecting explicit
  `tenant_id` even when it equals the current tenant. The tenant comes only
  from the trusted host request.
  Storefront Search and suggestions read only the *declared* Search corpus: the host
  declares each source module as public or gated, gated modules answer a real-time
  discovery preflight and per-candidate authorization, and modules that were never
  declared are denied. Admin Search is unaffected.
- Provide the generic `SearchAudiencePolicyPort` and `SearchAudienceRegistry` SPI for
  domain-owned real-time audience discovery preflights and batch candidate
  authorization across dynamic access-controlled source modules, without any
  module-specific code inside the generic storefront engine.
- Publish transport-neutral `SearchQueryPort` and `SearchSuggestionPort` provider
  boundaries.

## Result navigation ownership

`canonical_search_result_url` is the only application-route policy for Search
results. It derives product, content, and Blog routes before transport
serialization.

Blog navigation requires the canonical `source_module=blog` /
`entity_type=blog_post` pair and a bounded ASCII slug from the Blog projector.
Missing, malformed, spoofed, traversal, whitespace, and overlong slugs fail
closed. Content source-module values are validated before they can enter a query
string.

GraphQL Search, storefront native Search, Search admin preview, and admin global
search all delegate to this function. The storefront transport facade returns the
selected payload unchanged. There is no post-transport navigation enrichment,
transport-local Blog route builder, or compatibility URL implementation.

## Interactions

- Depends on `rustok-core` for module and event contracts.
- Uses `rustok-api` provider contexts and errors at the FBA boundary.
- May ingest domain tables or neutral read models but keeps its own search storage
  and runtime.
- Content document tags are projected from `nodes.metadata.tags`; Search does not
  read the removed taggable join model.
- Forum category documents carry only the ids Forum projected.
  Search does not resolve descendants or copy Forum tree policy: a category root is
  expanded by the Forum-owned storefront scope service before Search execution, and
  topic/reply eligibility is resolved by Forum owners on current state.
- Is composed by `apps/server`, `apps/admin`, and `apps/storefront` through module
  contracts rather than host-owned Search logic.
- Keeps external engine integrations behind Search-owned connector crates.
- Is a whole-module remote extraction pilot: PostgreSQL and future
  Meilisearch/Typesense/Algolia engines remain internal `SearchEngine`
  implementations, while consumers use only Search ports.

## Entry points

- `projector_core.rs` is the canonical implementation behind the public `SearchProjector` facade; no retired `legacy` projector source remains.
- `SearchModule`
- `SearchEngine` / `SearchEngineKind`
- `SearchConnectorDescriptor`
- `PgSearchEngine`
- `SearchQueryPort`
- `SearchSuggestionPort`
- `SearchAudiencePolicyPort` / `SearchAudienceRegistry` / `SearchAudienceClass`
- `SearchCandidateDescriptor` / `resolve_audience_eligible_candidates`
- `execute_audience_eligible_search` / `StorefrontSearchAudienceContext`
- `SearchAudienceDocumentScope`
- `StorefrontSuggestionAudience`
- `canonical_search_result_url`
- `graphql::SearchQueryRoot` / `graphql::SearchMutationRoot`

## Capability matrix

| Capability | Primary surface | Consumer | Boundary |
|---|---|---|---|
| Public search | `storefrontSearch` | Storefronts | Tenant-scoped, published/public documents only |
| Audience security | `resolve_audience_eligible_candidates` | Storefronts | Multi-module real-time candidate post-authorization and preflight discovery |
| Suggestions | `storefrontSearchSuggestions` | Storefronts | Same public visibility boundary as search |
| Click tracking | `trackSearchClick` | Storefronts/admin | Best-effort analytics write using canonical href |
| Admin preview | `searchPreview` / `searchSettingsPreview` | Admin packages | Authenticated tenant-scoped control plane |
| Host quick search | `adminGlobalSearch` | Admin shell/KBar | Permission-filtered canonical results |
| Diagnostics | `searchDiagnostics` | Operators | Tenant-scoped lag and consistency inspection |
| Analytics | `searchAnalytics` | Operators | Query, CTR, abandonment, and latency analysis |
| Dictionaries | dictionary queries/mutations | Operators | Tenant-owned synonyms, stop words, and pin rules |
| Settings/rebuild | `triggerSearchRebuild` and settings mutations | Operators | Permission-gated and event-published |

## Error catalog and validation policy

- Invalid engines, ranking profiles, presets, filters, dictionary values, UUIDs,
  and malformed route payloads are rejected before execution.
- Admin/control-plane surfaces require authenticated tenant-scoped authority.
- Public storefront surfaces remain read-only.
- Storage and connector failures are operational failures, not caller validation
  errors. GraphQL maps them to a generic `INTERNAL_ERROR` response while retaining
  detailed diagnostics only in server-side telemetry/logging; validation, not-found,
  authentication and permission failures keep their typed public codes.
- Ranking profiles use stable identifiers: `balanced`, `exact`, `fresh`,
  `catalog`, and `content`.
- Filter/preset keys use bounded normalized ASCII identifiers.
- Query normalization trims input, removes configured stop words, and expands
  tenant-owned synonyms before FTS execution.

## FBA provider boundary

- `SearchQueryPort` and `SearchSuggestionPort` define `search.query.v1`.
- The in-process provider is `PgSearchEngine`; provider calls use shared read
  deadline semantics, locale fallback, and `PortError` mapping.
- Provider registry:
  `contracts/search-fba-registry.json`.
- Static evidence:
  `contracts/evidence/search-contract-test-static-matrix.json`.
- Runtime fallback evidence:
  `contracts/evidence/search-runtime-fallback-smoke.json`.
- Runtime contract evidence:
  `contracts/evidence/search-runtime-contract-smoke.json`.
- Invocation evidence:
  `contracts/evidence/search-runtime-invocation-trace.json`.
- Canonical URL evidence:
  `contracts/evidence/search-canonical-url-contract.json`.
- `npm run verify:search:fba` checks provider contracts and current-only canonical
  URL ownership without compiling the workspace.

## Audience Security and Legacy Retirement Path

The historical specialized Forum search execution pipeline
(`forum_storefront_execution.rs`, `storefront_result_eligibility.rs`) is retired in
favor of the generic **Audience Security SPI** (`audience_security.rs`), governed by
[ADR 2026-10-25](../../../DECISIONS/2026-10-25-search-audience-declared-corpus.md).

### Declared audience classes

Every source module that writes `search_documents` must be declared exactly once:

- `SearchAudienceRegistry::declare_public_modules([...])` declares public discovery
  modules (Blog, Content, Commerce, Docs, Pages in the shipped host).
- `SearchAudienceRegistry::register(policy)` declares a gated module and registers its
  `SearchAudiencePolicyPort`.

Declaring a module twice is a `conflict` error. A module with **no** declared class is
never publicly discoverable: candidates from it are denied with an operator warning, and
`public_discovery_allowed` answers `false`. `apps/server` owns the inventory in
`search_public_source_modules()` — `blog`, `commerce`, `docs`, `pages`, the
Content-owned `CONTENT_NODE_KINDS` (the Content projector keys its documents by node
kind) and the `content` fallback bucket the projector uses for a damaged empty kind —
and fails host composition when a declaration or the Forum policy registration is
rejected.

### Execution model

1. The request scope is resolved against the declared corpus; undeclared modules are
   denied and unused classes never reach SQL.
2. Anonymous requests run the per-module public discovery preflight; a closed or
   disabled module is excluded instead of failing the request (authenticated callers are
   authorized per candidate, because discovery gating is a public-surface concept).
3. Gated candidates are scanned in `AUDIENCE_CANDIDATE_SCAN_PAGE_SIZE` pages and
   authorized in batches of at most `MAX_SEARCH_AUDIENCE_CANDIDATES`. A scan above
   `MAX_SEARCH_AUDIENCE_SCAN_DOCUMENTS` is a documented `Validation` error; public
   modules are never bounded by it.
4. The authorized ids are applied inside the SQL visibility filter
   (`(source_module NOT IN (gated) OR document_id IN (authorized))`), so totals, facets,
   ordering and pagination are computed by PostgreSQL over the authorized row set. There
   is no second facet vocabulary and no in-memory pagination.
5. Storefront query rules may reorder the authorized page; they may inject an extra
   pinned document only when no gated module is in scope, and injected documents are
   checked for payload channel visibility for every entity type.

### SPI error contract

- `Ok(false)`, `NotFound`, `Forbidden` — module unavailable to this caller: its
  documents are excluded, the request succeeds.
- `Unavailable`, `Timeout` — operational failure: the request fails without partial
  results.
- `Validation`, `Conflict` — caller error; `InvariantViolation` — internal error.

### Boundaries and fail-closed defaults

- `apps/server` attaches the composed registry to the GraphQL schema data
  (`graphql_runtime_inputs.shared_get::<SearchAudienceRegistry>()`); schema construction
  fails when the host did not compose it. Search is a required module, so the manifest
  `runtime_data_factory` hook is not wired for it and the attachment is explicit.
- GraphQL `storefrontSearch` / `storefrontSearchSuggestions` and the native SSR adapter
  return an explicit error when the request data has no `SearchAudienceRegistry`. They
  never fall back to an empty registry.
- Storefront suggestions use the same corpus: declared module scope, public discovery
  preflight, and per-candidate authorization for gated modules.
- `SearchAudiencePolicyPort::public_discovery_allowed` has no permissive default; every
  gated policy states it.
- The generic engine receives only `SearchAudienceDocumentScope`; `exclude_forum` and
  `source_module <> 'forum'` no longer exist in `pg_engine`.

### Retired legacy surface (Phase 4, complete)

Nothing of the specialized Forum storefront pipeline remains, not even as a
`#[deprecated]` bridge:

- deleted from `rustok-search`: `forum_storefront_execution.rs`,
  `forum_storefront_execution_public.rs`, `storefront_result_eligibility.rs`,
  `forum_document_filters.rs`, `forum_current_channel_filter.rs` and
  `graphql/forum_storefront.rs`;
- deleted from `rustok-search-storefront`: `transport/forum_graphql_adapter.rs` and
  `transport/forum_native_server_adapter.rs`;
- deleted the matching `forumStorefrontSearch` operation, the
  `execute_forum_storefront_search` entry point and `StorefrontSearchResultEligibilityPort`
  in every form (type, shared alias, resolver function, candidate kind).

The generic path is the only path: one `execute_audience_eligible_search` executor over
`storefrontSearch`, one `SearchAudiencePolicyPort` implementation per gated module
(`apps/server/src/services/forum_search_result_eligibility.rs` for Forum), and one
`StorefrontSearchCategoryScopePort` capability that the owner module implements and
declares with `source_module()`. `npm run verify:search:audience` fails when any retired
file or symbol reappears.

## Current architecture

- Search queries are read-only and never bootstrap indexes on request paths.
- `SearchIngestionHandler` updates `search_documents` asynchronously from domain
  events and handles targeted/full rebuilds and module toggles.
- Rebuilds are transactional so consumers do not observe partial tenant indexes.
- Blog projection follows the active PostgreSQL `search_path` and removes stale
  documents before source lookup; author enrichment is tenant-scoped by both user
  identity and source-post tenant, so malformed cross-tenant author references do
  not populate Search metadata.
- Product filters use normalized category, channel, and attribute projections.
- Exact Forum category filtering reuses `category_ids`: category documents match
  their document identifier and topic/reply documents match the public
  `facets.category_id` value. Search does not resolve descendants or copy Forum
  tree policy.
- GraphQL and native Leptos transports expose the same normalized query/result
  fields.
- The Search admin native adapter is split into focused API, read, write,
  normalization, pipeline, mapping, and support parts. Its mapping part delegates
  result URLs to the Search owner policy.
- Admin global search admits canonical Blog result types only under
  `BLOG_POSTS_READ` and fails closed for unknown source/entity pairs.
- Storefront transport selection does not mutate returned result URLs.
- Dictionaries and exact-query pin rules are tenant-owned.
- Diagnostics expose lagging, missing, orphaned, and inconsistent documents.
- Analytics expose volume, zero-result rate, latency, CTR, abandonment, and query
  intelligence.
- External engines cannot bypass Search ports, owner URL mapping, or connector
  credentials isolation.

## Verification

- `npm run verify:search:audience`
- `npm run verify:search:fba`
- `npm run verify:search:ui-boundary`
- `node scripts/verify/verify-search-canonical-url-contract.mjs`
- `node scripts/verify/verify-search-canonical-url-contract.test.mjs`
- `node scripts/verify/verify-search-blog-projection.mjs`
- `node scripts/verify/verify-search-blog-projection.test.mjs`
- `node scripts/verify/verify-search-audience-classes.mjs`
- `cargo test -p rustok-search engine::tests::canonical_url`
- `cargo test -p rustok-search category_filter -- --nocapture`
- `cargo xtask module validate search`

## Known Limitations / Pending Implementation

The items below are verified gaps. They are not hidden behind compatibility wrappers.
Each one has a follow-up task in
[`docs/implementation-plan.md`](./docs/implementation-plan.md).

- The Forum-only Search filters are owner-side. `storefrontSearch` accepts no Forum
  argument: author, tag, solved-state, locale, inclusive published-date and trusted
  current-channel narrowing are owned by the Forum read surface
  (`forumStorefrontAudienceTopics` / `forumTopics`) and by the host
  `SearchAudiencePolicyPort`, so Search never learns a Forum field. No storefront
  surface renders additional filter controls yet. Owner plan: `FORUM-23B2F1` to
  `FORUM-23B2F4`, now scoped to the generic surface.
- Storefront query-text suggestions remain disabled because the aggregate query logs
  cannot be channel-authorized; admin and global query suggestions are unchanged.
  Owner plan: `FORUM-23B2E1`/`FORUM-23B2E2`.
- Runtime evidence for the Forum projection slices is maintainer-owned and not retained:
  PostgreSQL ordering, restart, retry, Iggy delivery and multi-process contention proofs
  stay `source_complete_execution_pending`, and `scripts/verify/verify-forum-search-*.mjs`
  guards assert the source contracts only.

## Docs

- [CRATE_API.md](CRATE_API.md)
- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../../docs/index.md)
