# rustok-search implementation plan

## Current state

`rustok-search` owns search documents, PostgreSQL FTS baseline, catalog
projection search, analytics, dictionaries, query rules, rebuild/diagnostics,
and module-owned admin/storefront surfaces. It remains separate from
`rustok-index`; product catalog projections are consumed through the published
boundary rather than by importing index runtime types. The FFA split is
`phase_b_ready`; no further UI extraction is planned without a new functional
surface.

## FFA/FBA status

- FFA status: `phase_b_ready`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- `SearchQueryPort` and `SearchSuggestionPort` are provider contracts in
  `crates/rustok-search/contracts/search-fba-registry.json`.
- Evidence: `crates/rustok-search/contracts/evidence/search-contract-test-static-matrix.json`,
  `crates/rustok-search/contracts/evidence/search-runtime-fallback-smoke.json`,
  `crates/rustok-search/contracts/evidence/search-runtime-contract-smoke.json`,
  and `crates/rustok-search/contracts/evidence/search-runtime-invocation-trace.json`.
  These are source-locked/no-compile evidence; live provider invocation is
  required for promotion.
- Guardrails: `scripts/verify/verify-search-fba.mjs` and
  `scripts/verify/verify-search-ui-boundary.mjs`.

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
3. **Stage external engines as adapters.** Add Meilisearch, Typesense, or
   Algolia only behind dedicated connector crates with schema-sync, health,
   fallback, and data-consistency contracts. Done when a selected connector
   cannot bypass `SearchQueryPort`/`SearchSuggestionPort` or replace the
   PostgreSQL baseline implicitly.

## Verification

- `npm run verify:search:fba`
- `npm run verify:search:ui-boundary`
- `cargo xtask module validate search`
- Targeted ingestion, ranking, catalog-filter, diagnostics, and live provider
  contract tests.

## References

- [Crate README](../README.md)
- [Search documentation](./README.md)
- [Search FBA registry](../contracts/search-fba-registry.json)
