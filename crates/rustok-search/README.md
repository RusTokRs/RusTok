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
- Is composed by `apps/server`, `apps/admin`, and `apps/storefront` through module
  contracts rather than host-owned Search logic.
- Keeps external engine integrations behind Search-owned connector crates.
- Is a whole-module remote extraction pilot: PostgreSQL and future
  Meilisearch/Typesense/Algolia engines remain internal `SearchEngine`
  implementations, while consumers use only Search ports.

## Entry points

- `SearchModule`
- `SearchEngine` / `SearchEngineKind`
- `SearchConnectorDescriptor`
- `PgSearchEngine`
- `SearchQueryPort`
- `SearchSuggestionPort`
- `canonical_search_result_url`
- `graphql::SearchQueryRoot` / `graphql::SearchMutationRoot`

## Capability matrix

| Capability | Primary surface | Consumer | Boundary |
|---|---|---|---|
| Public search | `storefrontSearch` | Storefronts | Tenant-scoped, published/public documents only |
| Suggestions | `storefrontSearchSuggestions` | Storefronts | Same public visibility boundary as search |
| Click tracking | `trackSearchClick` | Storefronts/admin | Best-effort analytics write using canonical href |
| Admin preview | `searchPreview` / `searchSettingsPreview` | Admin packages | Authenticated tenant-scoped control plane |
| Host quick search | `adminGlobalSearch` | Admin shell/KBar | Permission-filtered canonical results |
| Diagnostics | `searchDiagnostics` | Operators | Tenant-scoped lag and consistency inspection |
| Analytics | `searchAnalytics` | Operators | Query, CTR, abandonment, and latency analysis |
| Dictionaries | dictionary queries/mutations | Operators | Tenant-owned synonyms, stop words, and pin rules |
| Settings/rebuild | `triggerSearchRebuild` and settings mutations | Operators | Permission-gated and event-published |

## Validation and error policy

- Invalid engines, ranking profiles, presets, filters, dictionary values, UUIDs,
  and malformed route payloads are rejected before execution.
- Admin/control-plane surfaces require authenticated tenant-scoped authority.
- Public storefront surfaces remain read-only.
- Storage and connector failures are operational failures, not caller validation
  errors.
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

## Current architecture

- Search queries are read-only and never bootstrap indexes on request paths.
- `SearchIngestionHandler` updates `search_documents` asynchronously from domain
  events and handles targeted/full rebuilds and module toggles.
- Rebuilds are transactional so consumers do not observe partial tenant indexes.
- Blog projection follows the active PostgreSQL `search_path` and removes stale
  documents before source lookup.
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
- `cargo test -p rustok-search engine::tests::canonical_url`
- `cargo test -p rustok-search category_filter_preserves_product_and_adds_exact_forum_scope -- --nocapture`
- `cargo xtask module validate search`

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../docs/index.md)
