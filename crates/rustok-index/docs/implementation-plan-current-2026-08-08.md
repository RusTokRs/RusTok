# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked after Product Storefront shadow adapter merge `4e9e032e11848db91ad4f837937c0de8ca3a7eaf` and continued on
`agent/product-storefront-filter-terms-20260808`.

`implementation-plan.md` remains historical architecture context. This file is the current execution cursor.

## Current primary owner gate

`M6 - execute and admit concrete repair PostgreSQL evidence`

Repair implementation/harness/admission source remains complete. Maintainer execution/admission is still
required and is not claimed by source inspection.

## M7 Product Storefront source state

Source-complete:

- one current 15-field Product schema on routing key `4`; lower keys historical only;
- schema-scoped Product replay IDs, Product owner clock and canonical typed `attribute_terms`;
- Product channel relation/freshness materialization;
- localized entity identity fold, scoped cursor v3 and requested -> fallback projection;
- localized PostgreSQL compiler/decoder/runtime with persisted readiness, generic admission and one
  `REPEATABLE READ, READ ONLY` page/count snapshot;
- bounded generic String `TextLike` for all-translations title matching;
- localized Product-ID tie-break direction matching owner Asc/Desc ordering;
- pure crate-local Product Storefront shadow query builder;
- Product-owned public Storefront attribute-filter -> canonical term resolution capability.

The Product-owned resolver now centralizes typed filter parsing used by the owner SQL path and exposes
neutral `ProductAttributeTermExpr` values through optional `ProductCatalogSchemaReadPort` capability.
Select/Multiselect option codes resolve to current active Product option UUIDs; UUID inputs keep owner
identity semantics; missing option code/nil UUID becomes `Never` and therefore preserves owner empty-result
behavior. Distribution Rust term helpers delegate canonical term identity to Product.

## Remaining Storefront parity blockers

- owner title search has no explicit length bound vs Index `TextLike` 1024-byte bound;
- owner/default PostgreSQL collation vs Index deterministic `COLLATE "C"`;
- channel-less owner visibility cannot currently be represented exactly by `sales_channel_ids`;
- owner page depth exceeds Index bounded offset depth;
- localized Taxonomy tag names must be hydrated after Product page identity/count is fixed.

## Retained Product evidence debt

Historical PostgreSQL packets must be actualized to routing key `4` / current 15-field Product contract;
never add a key-3 runtime alias. Localized Storefront retained equivalence must cover requested/fallback/
third-locale projection and search, wildcard behavior, scalar and localized EAV terms, Select/Multiselect
option code and UUID inputs, missing option behavior, channel membership, equal timestamp Asc/Desc ties,
pagination/count, stale locale exclusion, readiness/admission and restart.

## M5 incremental ingestion

- [x] Source replay registry and bounded failures.
- [x] Inbox deduplication and monotonic source versions.
- [x] Mutation event orchestration and exact source refresh.
- [x] Product locale/ProductVariant refresh ledgers and durable relay.
- [ ] Execute canonical event-contract digest admission on reviewed `main`.
- [ ] Add canonical Product Index typed event family only after digest admission.
- [ ] Retain commit/ack crash-redelivery evidence for that route.

## M6 replay/reconciliation/repair

- [x] Bounded replay, durable jobs/leases/checkpoints and cancellation.
- [x] Reconciliation, drift diagnosis and targeted repair source.
- [x] Real-migration repair PostgreSQL harness and retained-evidence admission tooling.
- [ ] Execute and admit the concrete repair PostgreSQL packet.
- [ ] Complete remaining multi-host/restart/shutdown/command-transport evidence.

## M7 Product Storefront graph

- [x] Current Product/ProductVariant/SalesChannel sources and graph freshness.
- [x] One current 15-field Product contract and schema-safe replacement mechanism.
- [x] Canonical typed EAV terms and Product owner clock.
- [x] Localized identity/fallback architecture and query/cursor contract.
- [x] Localized PostgreSQL compiler/decoder/runtime.
- [x] Generic scalar String `TextLike`.
- [x] Explicit localized entity-ID tie-break direction matching owner Asc/Desc ordering.
- [x] Product Storefront Index shadow/evidence query builder.
- [x] Product-owned Storefront attribute-filter resolution to neutral canonical term expressions.
- [ ] Wire Product term expressions into the shadow builder/executor.
- [ ] Retain owner-vs-Index localized PostgreSQL equivalence packet.
- [ ] Resolve/admit search-length and collation parity.
- [ ] Resolve channel-less unrestricted visibility parity or keep that shape owner-native.
- [ ] Decide authoritative deep-page policy.
- [ ] Batch-hydrate Taxonomy tag names after the Product page is fixed.
- [ ] Actualize retained Product PostgreSQL packets to routing key `4`.
- [ ] Extend folded linked paths only with dedicated target-availability evidence.
- [ ] Execute/admit current replacement Product PostgreSQL evidence.
- [ ] Stage/rebuild/promote Product key `4` for a tenant.
- [ ] Move Storefront traffic only after every parity/readiness/freshness/restart gate passes.

## Next source-code step

Build a non-serving **shadow executor/equivalence boundary**. It should retrieve the host-selected Product
schema read port, resolve owner EAV inputs there, translate `ProductAttributeTermExpr` into Index root
predicates, execute through `execute_localized_query`, and retain owner-vs-Index evidence without changing
mounted Storefront behavior. Taxonomy hydration stays after Product page selection.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
