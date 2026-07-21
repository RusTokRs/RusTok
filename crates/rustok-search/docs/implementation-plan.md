# rustok-search implementation plan

## Current state

`rustok-search` owns search documents, PostgreSQL FTS baseline, catalog
projection search, analytics, dictionaries, query rules, rebuild/diagnostics,
and module-owned admin/storefront surfaces. It remains separate from
`rustok-index`; product catalog projections are consumed through the published
boundary rather than by importing index runtime types. The FFA split is
`phase_b_ready`; no further UI extraction is planned without a new functional
surface.

The Blog projector stores a canonical slug in each `blog_post` payload. The Rust
storefront transport facade normalizes navigation after selecting native or
GraphQL transport: existing backend URLs remain authoritative, while a Blog
result without a URL receives `/modules/blog?slug=...` only for a bounded safe
slug. Invalid or missing slugs stay non-navigable.

Blog ingestion now has two executable, unrun harness layers. A routing target
locks all Blog lifecycle and targeted/full reindex events. An env-gated
PostgreSQL target creates an isolated schema, runs Search migrations, projects
real Blog source rows through `SearchIngestionHandler`, verifies lifecycle and
payload replacement, and checks tenant-scoped full rebuild behavior. Source-table
availability now resolves through the active PostgreSQL `search_path` instead of
hard-coding `public`, matching the projector's unqualified SQL.

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
  and `crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`.
- Blog projection harness status: `executable_no_run`; execution remains owned by
  the user and requires `RUSTOK_SEARCH_TEST_DATABASE_URL` or PostgreSQL
  `DATABASE_URL`.
- Guardrails: `scripts/verify/verify-search-fba.mjs`,
  `scripts/verify/verify-search-ui-boundary.mjs`, and
  `scripts/verify/verify-search-blog-navigation.mjs`.
- Blog result navigation parity is transport-neutral in the Rust storefront:
  the same post-processing runs after native or GraphQL selection, preserves
  owner/backend URLs, and fails closed on malformed indexed payloads.
- Blog projection table discovery and all subsequent reads use one schema
  resolution policy through the connection `search_path`; schema-isolated tests
  and deployments no longer silently check `public` while querying another
  schema.

## Deployment and connector boundary

Search is the second whole-module extraction pilot. The remote deployment
contains the complete `rustok-search` owner, including `SearchEngine`, ranking,
dictionaries, query rules, analytics, fallback policy, and the PostgreSQL
baseline. Storefront and admin consumers call only `SearchQueryPort` and
`SearchSuggestionPort` over the selected transport.

Meilisearch, Typesense, and Algolia remain connector implementations inside
this search service. They receive canonical `SearchQuery`/document inputs and
return normalized `SearchResult`/suggestion DTOs. Consumers never select a
connector, access engine credentials, or depend on engine-specific schemas.

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
   reindex, full-scope reindex, and unrelated-target rejection.
4. Added an isolated-schema PostgreSQL Blog projection harness covering create,
   publish, archive, delete, projected payload, stale-document replacement, and
   cross-tenant rebuild isolation.
5. Removed the Blog projector's `public` schema assumption so table discovery
   follows the same active `search_path` as its source and destination SQL.

## Next results

1. **Execute live Blog projection evidence.** Run the new routing and PostgreSQL
   targets, retain migration/`pg_trgm` capability evidence, and add targeted
   reindex missing-post plus module-disabled cleanup cases.
2. **Execute live provider contract evidence.** Run queries and suggestions
   against a real PostgreSQL provider under deadline, fallback, error, locale,
   tenant, channel, and catalog-filter conditions. Done when invocation traces
   are backed by runtime results and justify any status promotion.
3. **Harden search operations.** Deliver ingestion/rebuild retry and DLQ
   behavior together with production-grade diagnostics and analytics views,
   including recovery visibility for lagging or inconsistent documents. Done
   when operator actions have bounded retry/failure semantics and observable
   outcomes instead of source-only evidence.
4. **Promote canonical URL derivation to the shared Search contract.** Move the
   Blog route fallback from Rust storefront post-processing into the shared
   result projection when the large GraphQL type file can be changed through an
   atomic patch. Keep the storefront fallback during compatibility rollout and
   remove it only after all consumers read the canonical backend URL.
5. **Stage external engines as adapters.** Add Meilisearch, Typesense, or
   Algolia only behind dedicated connector crates with schema-sync, health,
   fallback, and data-consistency contracts. Done when a selected connector
   cannot bypass `SearchQueryPort`/`SearchSuggestionPort` or replace the
   PostgreSQL baseline implicitly.

## Verification

- `cargo test -p rustok-search --test blog_ingestion_contract_test`
- `RUSTOK_SEARCH_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-search --test blog_projection_postgres_test`
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
