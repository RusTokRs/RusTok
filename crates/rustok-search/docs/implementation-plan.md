# rustok-search implementation plan

## Current state

`rustok-search` owns normalized search documents, PostgreSQL FTS, catalog and Blog
projection ingestion, analytics, dictionaries, query rules, rebuild/diagnostics,
and module-owned admin/storefront surfaces. It remains separate from
`rustok-index`; consumers depend on published Search contracts rather than index
runtime types. The FFA split is `phase_b_ready` with focused core, transport, and
UI packages.

Canonical result navigation has a single owner policy:
`canonical_search_result_url` in `crates/rustok-search/src/engine.rs`. It derives
product, content, and Blog URLs from normalized `SearchResultItem` values before
transport serialization. Blog navigation requires the canonical
`source_module=blog` / `entity_type=blog_post` pair and a bounded ASCII slug from
the owner-projected payload. Missing, malformed, spoofed, traversal, whitespace,
and overlong values fail closed. Content source-module values are bounded before
they enter a query string.

GraphQL Search, storefront native Search, Search admin preview, and admin global
search all delegate to this single owner policy. The storefront transport facade
returns the selected transport payload unchanged: there is no transport fallback,
no post-processing navigation module, and no local Blog route builder. The Search
admin native adapter is split into focused include-parts for API facade, read
handlers, write handlers, normalization, execution pipeline, mapping, and support.
Only the mapping part converts normalized results to admin DTOs, and it delegates
URL resolution to `canonical_search_result_url`.

Blog ingestion has two executable, unrun harness layers. A routing target locks
Blog lifecycle, module-toggle, and targeted/full reindex events. An env-gated
PostgreSQL target creates an isolated schema, runs Search migrations, projects
real Blog source rows through `SearchIngestionHandler`, verifies lifecycle and
payload replacement, checks tenant-scoped full rebuild, targeted missing-post
cleanup, and module-disabled cleanup followed by enable-time rebuild. Source-table
availability resolves through the active PostgreSQL `search_path` instead of
hard-coding `public`.

## FFA/FBA status

- FFA status: `phase_b_ready`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Provider contracts: `SearchQueryPort` and `SearchSuggestionPort` in
  `crates/rustok-search/contracts/search-fba-registry.json`.
- Canonical URL status: `source_verified_no_compile`.
- Canonical URL evidence:
  `crates/rustok-search/contracts/evidence/search-canonical-url-contract.json`.
- Canonical URL guardrail:
  `scripts/verify/verify-search-canonical-url-contract.mjs`.
- Blog projection evidence:
  `crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`.
- Blog projection harness status: `executable_no_run`; execution remains user-owned
  and requires `RUSTOK_SEARCH_TEST_DATABASE_URL` or PostgreSQL `DATABASE_URL`.
- GraphQL and all native/admin mappings use the same Search-owned URL function.
- The removed storefront `transport/navigation.rs` path is forbidden by the
  canonical URL guardrail.
- Transport-local `derive_search_result_url`, `derive_admin_search_result_url`,
  `enrich_search_result_urls`, and Blog route constants are forbidden.
- Blog projection table discovery and reads use the active `search_path`.
- `TenantModuleToggled(blog, false)` deletes only the current tenant Blog search
  scope; enabling the module rebuilds it from retained owner rows.
- Targeted Blog reindex deletes stale documents before source lookup, so a missing
  owner post cannot leave obsolete search data behind.

## Deployment and connector boundary

Search is a whole-module extraction boundary. Remote deployment contains the
complete `rustok-search` owner, including `SearchEngine`, ranking, dictionaries,
query rules, analytics, URL policy, and PostgreSQL baseline. Storefront and admin
consumers call normalized Search contracts and never construct application routes.

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
3. Added `canonical_search_result_url` with product, content, and Blog routing,
   bounded slug validation, source/entity ownership, and query-injection guards.
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

## Next results

1. **Execute canonical URL evidence.** Run core URL-policy tests, GraphQL
   storefront Search, native storefront Search, Search admin preview, and admin
   global search against projected product, content, and Blog documents. Retain
   proof that malformed Blog payloads remain non-navigable everywhere.
2. **Verify click analytics.** Confirm every Search surface records the canonical
   href without reconstructing routes in analytics code.
3. **Execute live Blog projection evidence.** Run routing and PostgreSQL harnesses
   and retain migration/`pg_trgm`, event-delivery, targeted missing-post cleanup,
   module-disable cleanup, and category reindex results.
4. **Execute live provider evidence.** Run query and suggestion providers under
   deadline, error, locale, tenant, channel, ranking, and catalog-filter conditions.
5. **Harden operations.** Add bounded ingestion/rebuild retry and DLQ behavior with
   observable lag, consistency, and recovery outcomes.
6. **Add external engines only as adapters.** Meilisearch, Typesense, or Algolia
   connectors must not bypass Search ports, owner URL mapping, or PostgreSQL
   baseline selection.

## Verification

- `cargo test -p rustok-search engine::tests::canonical_url`
- `node scripts/verify/verify-search-canonical-url-contract.mjs`
- `node scripts/verify/verify-search-canonical-url-contract.test.mjs`
- `cargo test -p rustok-search --test blog_ingestion_contract_test`
- `RUSTOK_SEARCH_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-search --test blog_projection_postgres_test`
- `node scripts/verify/verify-search-blog-projection.mjs`
- `node scripts/verify/verify-search-blog-projection.test.mjs`
- `npm run verify:search:fba`
- `npm run verify:search:ui-boundary`
- `cargo xtask module validate search`

## References

- [Crate README](../README.md)
- [Search documentation](./README.md)
- [Search FBA registry](../contracts/search-fba-registry.json)
