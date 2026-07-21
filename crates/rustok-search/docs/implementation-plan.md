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
storefront transport facade now normalizes navigation after selecting native or
GraphQL transport: existing backend URLs remain authoritative, while a Blog
result without a URL receives `/modules/blog?slug=...` only for a bounded safe
slug. Invalid or missing slugs stay non-navigable.

## FFA/FBA status

- FFA status: `phase_b_ready`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Structural shape: `core_transport_ui`
- `SearchQueryPort` and `SearchSuggestionPort` are provider contracts in
  `crates/rustok-search/contracts/search-fba-registry.json`.
- Evidence: `crates/rustok-search/contracts/evidence/search-contract-test-static-matrix.json`,
  `crates/rustok-search/contracts/evidence/search-runtime-fallback-smoke.json`,
  `crates/rustok-search/contracts/evidence/search-runtime-contract-smoke.json`,
  and `crates/rustok-search/contracts/evidence/search-runtime-invocation-trace.json`.
  These are source-locked/no-compile evidence; live provider invocation is
  required for promotion.
- Guardrails: `scripts/verify/verify-search-fba.mjs`,
  `scripts/verify/verify-search-ui-boundary.mjs`, and
  `scripts/verify/verify-search-blog-navigation.mjs`.
- Blog result navigation parity is transport-neutral in the Rust storefront:
  the same post-processing runs after native or GraphQL selection, preserves
  owner/backend URLs, and fails closed on malformed indexed payloads.

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

## Next results

1. **Execute live provider contract evidence.** Run queries and suggestions
   against a real PostgreSQL provider under deadline, fallback, error, locale,
   tenant, channel, and catalog-filter conditions. Done when invocation traces
   are backed by runtime results and justify any status promotion.
2. **Harden search operations.** Deliver ingestion/rebuild retry and DLQ
   behavior together with production-grade diagnostics and analytics views,
   including recovery visibility for lagging or inconsistent documents. Done
   when operator actions have bounded retry/failure semantics and observable
   outcomes instead of source-only evidence.
3. **Promote canonical URL derivation to the shared Search contract.** Move the
   Blog route fallback from Rust storefront post-processing into the shared
   result projection when the large GraphQL type file can be changed through an
   atomic patch. Keep the storefront fallback during compatibility rollout and
   remove it only after all consumers read the canonical backend URL.
4. **Stage external engines as adapters.** Add Meilisearch, Typesense, or
   Algolia only behind dedicated connector crates with schema-sync, health,
   fallback, and data-consistency contracts. Done when a selected connector
   cannot bypass `SearchQueryPort`/`SearchSuggestionPort` or replace the
   PostgreSQL baseline implicitly.

## Verification

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
