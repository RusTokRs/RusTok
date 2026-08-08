# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked after Product-owned Storefront filter resolution merge `215389fcac0e086d7015453d7712fc341d6b722f` and continued on
`agent/index-storefront-owner-terms-20260808`.

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
- Product-owned public Storefront attribute-filter -> neutral canonical term resolution;
- pure crate-local Storefront shadow builder that consumes only `ProductResolvedAttributeFilter` and
  translates `Term/And/Or/Not/Never` into Index root predicates.

`Never` maps to a bind-free false predicate by negating the current Product schema's required/non-null `id`
invariant. The builder validates owner/resolver filter count and code identity before translating terms, so
arbitrary consumer-built `FilterExpr` values can no longer cross the Product ownership boundary.

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
option code and UUID inputs, missing option `Never`, channel membership, equal timestamp Asc/Desc ties,
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
- [x] Wire Product term expressions into the shadow builder.
- [ ] Compose non-serving Product-owner + Index shadow executor.
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

Compose a non-serving **shadow executor/equivalence boundary**. It must retrieve the host-selected Product
schema read port, call `resolve_storefront_attribute_filters`, feed those owner-owned results to the shadow
builder, execute through `execute_localized_query`, and retain owner-vs-Index evidence without changing
mounted Storefront behavior. Taxonomy hydration remains after Product page selection.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
