# rustok-search implementation plan

## Current state

`rustok-search` owns search documents, PostgreSQL FTS baseline, catalog
projection search, analytics, dictionaries, query rules, rebuild/diagnostics,
and module-owned admin/storefront surfaces. It remains separate from
`rustok-index`; product catalog projections are consumed through the published
boundary rather than by importing index runtime types. The FFA split is
`phase_b_ready`; no further UI extraction is planned without a new functional
surface.

Canonical result navigation belongs to the normalized Search contract.
`canonical_search_result_url` derives product, content, and Blog URLs before
GraphQL or storefront-native serialization. Blog navigation requires the
canonical `source_module=blog` / `entity_type=blog_post` pair and a bounded safe
slug from the owner-projected payload; malformed or spoofed results fail closed.
Content kind values are bounded before they enter a query string. The Rust
storefront post-transport Blog enrichment remains temporarily as an idempotent
compatibility fallback and never overwrites a backend URL. The admin native
mapper is the last transport-local URL switch and is explicitly prevented from
adding a second Blog policy before its final cutover.

Blog ingestion has two executable, unrun harness layers. A routing target locks
all Blog lifecycle, module-toggle, and targeted/full reindex events. An env-gated
PostgreSQL target creates an isolated schema, runs Search migrations, projects
real Blog source rows through `SearchIngestionHandler`, verifies lifecycle and
payload replacement, checks tenant-scoped full rebuild, targeted missing-post
cleanup, and module-disabled cleanup followed by enable-time rebuild. Source-table
availability resolves through the active PostgreSQL `search_path` instead of
hard-coding `public`, matching the projector's unqualified SQL. A focused source
guardrail and negative fixtures lock this schema and lifecycle contract.

## FFA/FBA status

- FFA status: `phase_b_ready`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Structural shape: `core_transport_ui`.
- `SearchQueryPort` and `SearchSuggestionPort` are provider contracts in
  `crates/rustok-search/contracts/search-fba-registry.json`.
- Evidence: `crates/rustok-search/contracts/evidence/search-contract-test-static-matrix.json`,
  `crates/rustok-search/contracts/evidence/search-runtime-fallback-smoke.json`,
  `crates/rustok-search/contracts/evidence/search-runtime-contract-smoke.json`,
  `crates/rustok-search/contracts/evidence/search-runtime-invocation-trace.json`,
  `crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`,
  and `crates/rustok-search/contracts/evidence/search-canonical-url-contract.json`.
- Blog projection harness status: `executable_no_run`; execution remains owned by
  the user and requires `RUSTOK_SEARCH_TEST_DATABASE_URL` or PostgreSQL
  `DATABASE_URL`.
- Canonical URL status: `source_verified_no_compile`; core, GraphQL, and
  storefront-native result projections delegate to the shared policy. Only the
  admin native mapper and eventual compatibility-fallback removal remain.
- Guardrails: `scripts/verify/verify-search-fba.mjs`,
  `scripts/verify/verify-search-ui-boundary.mjs`,
  `scripts/verify/verify-search-blog-navigation.mjs`,
  `scripts/verify/verify-search-blog-projection.mjs`, and
  `scripts/verify/verify-search-canonical-url-contract.mjs`.
- GraphQL Search results delegate URL derivation to
  `canonical_search_result_url`; GraphQL no longer owns a transport-local route
  switch.
- Storefront native Search results delegate to the same owner policy before the
  shared DTO crosses the server-function boundary.
- The storefront compatibility fallback runs after native/GraphQL selection,
  preserves backend URLs, validates the same Blog slug shape, and fills only
  missing legacy URLs.
- The admin native mapper still has a product/content compatibility switch but
  the canonical URL guardrail forbids it from defining Blog route behavior.
- Blog projection table discovery and all subsequent reads use one schema
  resolution policy through the connection `search_path`; schema-isolated tests
  and deployments no longer silently check `public` while querying another
  schema.
- The PostgreSQL fixture places its unique schema before `public`, keeping test
  tables isolated while retaining access to shared extensions such as `pg_trgm`.
- `TenantModuleToggled(blog, false)` deletes only the current tenant Blog search
  scope; `TenantModuleToggled(blog, true)` rebuilds it from retained owner rows.
- Targeted Blog reindex deletes a stale document before source lookup, so a
  missing owner post remains absent instead of preserving obsolete search data.

## Deployment and connector boundary

Search is the second whole-module extraction pilot. The remote deployment
contains the complete `rustok-search` owner, including `SearchEngine`, ranking,
dictionaries, query rules, analytics, fallback policy, canonical URL policy,
and the PostgreSQL baseline. Storefront and admin consumers call only
`SearchQueryPort` and `SearchSuggestionPort` over the selected transport.

Meilisearch, Typesense, and Algolia remain connector implementations inside
this search service. They receive canonical `SearchQuery`/document inputs and
return normalized `SearchResult`/suggestion DTOs. Consumers never select a
connector, access engine credentials, or depend on engine-specific schemas.
Connector results must pass through the Search-owned URL policy before transport
serialization; connectors do not construct application routes.

`rustok-index` remains a separate ingestion/read-model owner. Its canonical
read models may enrich Search only through `IndexReadModelPort`; current
query-time SQL reads of `index_product_categories` and
`index_product_attribute_values` must be replaced by search-owned denormalized
fields populated during ingestion before database isolation. Search continues
to own `SearchIngestionHandler` and consumes domain events through a replayable
event transport. Index-service extraction waits for replay, duplicate, lag,
rebuild, and recovery evidence. See [ADR: Media and Search
Extraction Boundaries](../../../DECISIONS/2026-07-16-media-search-extraction-boundaries.md).

External connector crates implement the internal `SearchEngine` query contract
and a planned search-owned document writer contract for schema sync, upsert,
delete, rebuild, and health. They are linked into the whole Search service;
only normalized Search ports are exposed remotely.

## Completed implementation slices

1. Added Rust storefront Blog result navigation after transport selection,
   deriving `/modules/blog?slug=...` from the indexed owner payload without
   overwriting backend-provided URLs.
2. Added strict source/entity checks, bounded ASCII slug validation, malformed
   payload fail-closed behavior, unit coverage, and a focused source guardrail.
3. Added a Blog ingestion routing contract for all lifecycle events, targeted
   reindex, full-scope reindex, module toggles, and unrelated-target rejection.
4. Added an isolated-schema PostgreSQL Blog projection harness covering create,
   publish, archive, delete, projected payload, stale-document replacement, and
   cross-tenant rebuild isolation.
5. Removed the Blog projector's `public` schema assumption so table discovery
   follows the same active `search_path` as its source and destination SQL.
6. Added a focused Blog projection verifier with canonical, hard-coded-public,
   and missing-PostgreSQL-harness fixtures.
7. Added production module-toggle handling: disable deletes the tenant Blog
   scope, enable rebuilds it, and both use dedicated operation labels/metrics.
8. Added PostgreSQL targeted missing-post cleanup and module disable/enable
   lifecycle cases, then locked them in evidence and verifier fixtures.
9. Added the Search-owned `canonical_search_result_url` policy with Blog
   source/entity ownership, bounded slug validation, content-kind injection
   protection, and product/content compatibility tests.
10. Exported the canonical URL policy and migrated GraphQL result projection to
    it, removing the GraphQL-local URL switch.
11. Migrated storefront native result projection to the same core policy while
    retaining an idempotent post-transport compatibility fallback.
12. Added machine-readable canonical URL evidence plus canonical and negative
    source-verifier fixtures for GraphQL, storefront native, Blog ownership, and
    admin-policy duplication.

## Next results

1. **Finish admin native URL cutover.** Migrate the final admin result mapper to
   `canonical_search_result_url`; afterward all backend Search result surfaces
   share one policy.
2. **Execute canonical URL evidence.** Run the core URL-policy tests, GraphQL
   storefront search against projected Blog documents, native backend URL
   behavior, compatibility idempotence, and click-href analytics evidence.
3. **Retire compatibility fallback.** Remove storefront post-processing only
   after all consumers prove they receive and preserve backend URLs.
4. **Execute live Blog projection evidence.** Run the routing, PostgreSQL, and
   source-verifier targets and retain migration/`pg_trgm`, event-delivery,
   targeted missing-post cleanup, and module-disabled cleanup evidence.
5. **Execute live provider contract evidence.** Run queries and suggestions
   against a real PostgreSQL provider under deadline, fallback, error, locale,
   tenant, channel, and catalog-filter conditions. Done when invocation traces
   are backed by runtime results and justify any status promotion.
6. **Harden search operations.** Deliver ingestion/rebuild retry and DLQ
   behavior together with production-grade diagnostics and analytics views,
   including recovery visibility for lagging or inconsistent documents. Done
   when operator actions have bounded retry/failure semantics and observable
   outcomes instead of source-only evidence.
7. **Stage external engines as adapters.** Add Meilisearch, Typesense, or
   Algolia only behind dedicated connector crates with schema-sync, health,
   fallback, and data-consistency contracts. Done when a selected connector
   cannot bypass `SearchQueryPort`/`SearchSuggestionPort` or replace the
   PostgreSQL baseline implicitly.
8. **Stop indexing serialized richtext JSON.** During the atomic
   [Richtext cutover](../../../docs/modules/rich-text-implementation-plan.md),
   replace raw `body` insertion in Blog/shared-content projectors with the one
   `rustok-content::richtext` plain-text projection. Split SQL
   `INSERT ... SELECT` into a typed Rust projection or consume owner-extracted
   text through an event; do not add a shadow text source without evidence.
   **Done when:** Search/Index fixtures contain prose rather than JSON syntax,
   locale/profile failures are observable, and no Search-local tree walker
   exists.

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
- `node scripts/verify/verify-search-blog-navigation.mjs`
- `node scripts/verify/verify-search-blog-navigation.test.mjs`
- `cargo xtask module validate search`
- Targeted ingestion, ranking, catalog-filter, diagnostics, navigation, and live
  provider contract tests.

## References

- [Crate README](../README.md)
- [Search documentation](./README.md)
- [Search FBA registry](../contracts/search-fba-registry.json)
