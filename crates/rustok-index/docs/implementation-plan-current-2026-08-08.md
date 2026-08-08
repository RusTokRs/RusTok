# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked after retained Product PostgreSQL key-4 actualization merge
`07117d8c88608625a941de501464002aa835897d` and continued on
`agent/product-storefront-search-bound-20260808`.

`implementation-plan.md` remains historical architecture context. This file is the current execution cursor.

## Current primary owner gate

`M6 - execute and admit concrete repair PostgreSQL evidence`

Repair implementation/harness/admission source remains complete. Maintainer execution/admission is still
required and is not claimed by source inspection.

## M7 Product Storefront source state

Source-complete:

- one current 15-field Product schema on routing key `4`; lower keys are historical storage identities only;
- schema-scoped Product replay IDs, Product owner clock and canonical typed `attribute_terms`;
- Product channel relation/freshness materialization;
- localized entity identity fold, cursor v3 and requested -> fallback projection;
- localized PostgreSQL compiler/decoder/runtime with readiness/admission and one repeatable-read page/count snapshot;
- generic String `TextLike` with a 1024-byte pattern bound;
- Product-owned Storefront title-search input bound of 1022 effective UTF-8 bytes, leaving exactly two bytes
  for the owner `%{search}%` pattern without truncation;
- localized Product-ID tie-break direction matching owner Asc/Desc ordering;
- Product-owned public Storefront attribute-filter -> neutral canonical term resolution;
- pure Storefront shadow builder consuming only Product-owned filter terms and the Product-owned search bound;
- non-serving owner-first `ProductStorefrontIndexShadowExecutor`;
- current-key core and EAV Product Storefront owner-vs-shadow PostgreSQL packet source;
- historical retained Product PostgreSQL fixtures actualized to current Product routing key `4`.

The Product search bound is enforced both by `StorefrontProductListQuery::try_new*` and immediately before
`CatalogService::list_published_products_with_query` constructs owner SQL. Public struct fields therefore
cannot bypass the owner contract. Distribution imports the same Product constant; it no longer owns a second
Product-specific search length value.

Core/EAV Storefront packets and historical Product packets remain source-only until maintainer execution.
The core packet also retains the no-requested/fallback-translation projection gap: owner emits
`Untitled product`/empty handle while generic localized Index returns null.

## Remaining Storefront parity/evidence blockers

- execute/review current-key Storefront and actualized retained Product PostgreSQL packets;
- owner/default PostgreSQL title `LIKE` collation vs Index deterministic `COLLATE "C"`;
- channel-less owner visibility cannot currently be represented exactly by `sales_channel_ids`;
- owner page depth exceeds Index bounded offset depth;
- map localized null title/handle to owner public placeholders in final projection;
- hydrate localized Taxonomy tag names only after Product page identity/count is fixed;
- define serving latency/deadline policy before any shadow-to-serving transition;
- complete maintainer-executed stale locale/readiness/admission/restart evidence.

## Retained Product evidence state

The retained Product PostgreSQL fixture set is source-aligned on routing key `4`: locale absence, materialized
query freshness, Product/Channel convergence, Channel identity transitions, linked-target delete/recreate,
linked-target availability equivalence and linked-target replay/redelivery. ProductVariant remains key `2`
and SalesChannel remains key `1`.

`verify-index-product-postgres-key4-fixtures.mjs` locks that boundary. Execution/admission remains separate.

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
- [x] Product-owned 1022-byte Storefront search input bound compatible with the 1024-byte TextLike pattern.
- [x] Explicit localized entity-ID tie-break direction matching owner Asc/Desc ordering.
- [x] Product Storefront Index shadow/evidence query builder.
- [x] Product-owned Storefront attribute-filter resolution.
- [x] Compose non-serving Product-owner + Index shadow executor.
- [x] Retain current-key core and EAV owner-vs-shadow PostgreSQL packet source.
- [x] Actualize historical retained Product PostgreSQL packets to routing key `4`.
- [ ] Execute/review the retained Product/Storefront PostgreSQL packets.
- [ ] Resolve/admit owner/default vs Index `COLLATE "C"` title-search collation parity.
- [ ] Resolve channel-less unrestricted visibility parity or keep that shape owner-native.
- [ ] Decide authoritative deep-page policy.
- [ ] Map no-localized-row nulls to owner public title/handle placeholders in final projection.
- [ ] Batch-hydrate Taxonomy tag names after the Product page is fixed.
- [ ] Extend folded linked paths only with dedicated target-availability evidence.
- [ ] Execute/admit current replacement Product PostgreSQL evidence.
- [ ] Stage/rebuild/promote Product key `4` for a tenant.
- [ ] Move Storefront traffic only after every parity/readiness/freshness/restart/latency gate passes.

## Next source-code step

Retain explicit PostgreSQL collation equivalence evidence for the owner all-translations `pt.title LIKE $1`
query versus Index `TextLike` compiled with deterministic `COLLATE "C"`. The packet must include ASCII,
non-ASCII/case-sensitive and wildcard/escape cases and must not weaken Index deterministic collation merely to
match one deployment's database default.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
