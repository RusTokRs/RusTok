# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked after Product-owned term translation merge `9a684becf802904baec3e860eb2178c689a0253c` and continued on
`agent/index-storefront-shadow-executor-20260808`.

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
- pure Storefront shadow builder consuming only `ProductResolvedAttributeFilter`;
- non-serving owner-first `ProductStorefrontIndexShadowExecutor` over host-selected Product and Index
  runtimes.

The shadow executor first calls the authoritative Product read port. Only after owner success does it resolve
Product EAV metadata, build the localized Index query and call `execute_localized_query`. Projected failures
remain data in the shadow result and cannot replace the successful owner result.

For channel-scoped comparison the caller must supply a trusted current slug/UUID pair; the executor checks
presence/non-empty/non-nil only and does not independently prove their correspondence. Built-in comparison
covers ordered Product IDs, exact count and `has_more` only. Full field/search/tag equivalence is still
retained PostgreSQL evidence debt.

## Remaining Storefront parity blockers

- owner title search has no explicit length bound vs Index `TextLike` 1024-byte bound;
- owner/default PostgreSQL collation vs Index deterministic `COLLATE "C"`;
- channel-less owner visibility cannot currently be represented exactly by `sales_channel_ids`;
- owner page depth exceeds Index bounded offset depth;
- localized Taxonomy tag names must be hydrated after Product page identity/count is fixed;
- shadow execution has no serving latency/deadline policy and must remain non-serving.

## Retained Product evidence debt

Historical PostgreSQL packets must be actualized to routing key `4` / current 15-field Product contract;
never add a key-3 runtime alias.

The next localized Storefront PostgreSQL packet must run owner and shadow execution side-by-side and cover:

- requested/fallback/neither localized projection;
- third-locale/all-translations title search and wildcard semantics;
- duplicate locale matches yielding one identity/count;
- scalar and localized EAV terms;
- Select/Multiselect option code, UUID and missing-option `Never` behavior;
- trusted public-channel membership;
- equal timestamp Asc/Desc Product-ID ties;
- pagination and exact count;
- stale locale exclusion, readiness/admission and replay/restart behavior;
- explicit search-bound and collation evidence.

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
- [x] Compose non-serving Product-owner + Index shadow executor.
- [ ] Retain owner-vs-Index localized PostgreSQL equivalence packet.
- [ ] Resolve/admit search-length and collation parity.
- [ ] Resolve channel-less unrestricted visibility parity or keep that shape owner-native.
- [ ] Decide authoritative deep-page policy.
- [ ] Batch-hydrate Taxonomy tag names after the Product page is fixed.
- [ ] Actualize retained Product PostgreSQL packets to routing key `4`.
- [ ] Extend folded linked paths only with dedicated target-availability evidence.
- [ ] Execute/admit current replacement Product PostgreSQL evidence.
- [ ] Stage/rebuild/promote Product key `4` for a tenant.
- [ ] Move Storefront traffic only after every parity/readiness/freshness/restart/latency gate passes.

## Next source-code step

Build the retained Product Storefront localized PostgreSQL **owner-vs-shadow equivalence packet/harness** on
current routing key `4`. It must exercise the actual Product owner list and the non-serving shadow executor,
record identity/order/count and projected field evidence, and keep unresolved search/channel/deep-page cases
explicitly fail-closed. Taxonomy hydration remains a post-page evidence step.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
