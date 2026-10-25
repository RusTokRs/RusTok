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
- Serve storefront Search, admin preview, and host global-search capabilities. Generic storefront FTS and suggestions exclude Forum documents at the SQL boundary (including totals, facets and pinned query rules) because those surfaces have no Forum audience owner; admin Search is unaffected. The explicit Forum-only storefront path preflights the Forum owner's public-discovery policy before scanning raw indexed candidates; its query rules may reorder only the already authorized page and never inject a pinned document outside owner eligibility.
- Provide generic `SearchAudiencePolicyPort` and `SearchAudienceRegistry` SPI for domain-owned real-time audience discovery preflights and batch candidate authorization across dynamic access-controlled source modules.
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
- `SearchAudiencePolicyPort` / `SearchAudienceRegistry`
- `SearchCandidateDescriptor` / `resolve_audience_eligible_candidates`
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

To preserve module isolation and prevent cross-module leakage, the historical specialized Forum search execution pipeline (`forum_storefront_execution.rs`, `storefront_result_eligibility.rs`) has been superseded by the generic **Audience Security SPI** (`audience_security.rs`):
- `execute_forum_storefront_search` and `StorefrontSearchResultEligibilityPort` are marked `#[deprecated]`.
- Domain modules implement `SearchAudiencePolicyPort` directly and register in `SearchAudienceRegistry`.
- Storefront consumers query through the unified `storefront_search` GraphQL entrypoint; candidate post-authorization is dispatched across all modules registered in `SearchAudienceRegistry`.
- Unconstrained source modules (e.g. `blog`, `content`, `products`) pass through automatically as public candidates without requiring dedicated authorization policies.
- Full physical removal of legacy `forum_*.rs` files from `rustok-search` is scheduled for Phase 4 after external verification scripts and contract JSONs transition to the generic SPI.

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

- `npm run verify:search:fba`
- `npm run verify:search:ui-boundary`
- `node scripts/verify/verify-search-canonical-url-contract.mjs`
- `node scripts/verify/verify-search-canonical-url-contract.test.mjs`
- `node scripts/verify/verify-search-blog-projection.mjs`
- `node scripts/verify/verify-search-blog-projection.test.mjs`
- `node scripts/verify/verify-forum-search-exact-category-filter.mjs`
- `node scripts/verify/verify-forum-search-storefront-scope.mjs`
- `node scripts/verify/verify-forum-search-result-eligibility.mjs`
- `node scripts/verify/verify-forum-search-author-filter.mjs`
- `node scripts/verify/verify-forum-search-tag-solved-filter.mjs`
- `node scripts/verify/verify-forum-search-locale-date-filter.mjs`
- `cargo test -p rustok-search engine::tests::canonical_url`
- `cargo test -p rustok-search category_filter_preserves_product_and_adds_exact_forum_scope -- --nocapture`
- `cargo xtask module validate search`

## Known Limitations / Pending Implementation

The items below are verified gaps. They are not hidden behind compatibility wrappers.
Each one has a follow-up task in
[`docs/implementation-plan.md`](./docs/implementation-plan.md).

- The Forum-only Search filters are transport-only. `forumStorefrontSearch` accepts
  exact author, tag, solved-state, locale, inclusive published-date and trusted
  current-channel arguments, and both storefront transports can send each family as an
  additive operation through the single Forum-only execution owner, but no storefront
  surface renders a control for them yet. The shared `SearchPreviewInput`/`SearchPreviewFilters`
  model deliberately does not carry these fields, so a caller that has not landed a
  filter keeps the base `forumStorefrontSearch(input: $input)` operation. Owner plan:
  `FORUM-23B2F1` to `FORUM-23B2F4`.
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
