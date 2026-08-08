# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked after Product Storefront shadow-executor merge `de32e18022e761e3fcc80d2e8becc625f567fc5e` and continued on
`agent/index-storefront-equivalence-postgres-20260808`.

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
  runtimes;
- first current-key Product Storefront owner-vs-shadow PostgreSQL packet source.

The core PostgreSQL packet is source-only until the maintainer runs it. It uses real Product/Index migrations,
channel relation freshness, Product source/mutation/query runtimes and Product owner reads. It covers
requested/fallback/neither localized projection, third-locale search, `%`/`_`/backslash wildcard behavior,
identity de-duplication, equal-timestamp Asc/Desc Product-ID ordering, exact count and offset page boundaries.

The packet also retains an explicit raw projection gap: owner uses `Untitled product`/empty handle when no
requested/fallback translation exists, while the generic localized Index layer returns null. Final Storefront
projection must apply those public placeholders after Index page identity/order/count are fixed.

## Remaining Storefront parity/evidence blockers

- execute/review the new core PostgreSQL packet; source presence is not evidence admission;
- add a separate EAV PostgreSQL packet for scalar/localized terms and Select/Multiselect code/UUID/`Never`;
- owner title search has no explicit length bound vs Index `TextLike` 1024-byte bound;
- owner/default PostgreSQL collation vs Index deterministic `COLLATE "C"`;
- channel-less owner visibility cannot currently be represented exactly by `sales_channel_ids`;
- owner page depth exceeds Index bounded offset depth;
- final Storefront projection must map localized null title/handle to owner placeholders;
- localized Taxonomy tag names must be hydrated after Product page identity/count is fixed;
- shadow execution has no serving latency/deadline policy and must remain non-serving.

## Retained Product evidence debt

Historical PostgreSQL packets must still be mechanically actualized to routing key `4` / current 15-field
Product contract; never add a key-3 runtime alias. This is deliberately kept separate from the new current-key
Storefront packet to avoid hiding large fixture rewrites inside parity work.

The next EAV Storefront packet should run owner and shadow side-by-side for:

- nonlocalized scalar terms;
- localized requested value and requested-missing/fallback value;
- Select/Multiselect exact option code;
- direct option UUID;
- missing option code and nil UUID `Never` behavior;
- exact identity/count/page agreement under the existing Product admission/freshness runtime.

Later execution/admission still needs stale locale/readiness/restart and explicit search-bound/collation review.

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
- [x] Retain current-key core owner-vs-shadow localized PostgreSQL packet source.
- [ ] Execute/review the core Storefront PostgreSQL packet.
- [ ] Retain Product EAV owner-vs-shadow PostgreSQL packet source.
- [ ] Execute/review the EAV Storefront PostgreSQL packet.
- [ ] Resolve/admit search-length and collation parity.
- [ ] Resolve channel-less unrestricted visibility parity or keep that shape owner-native.
- [ ] Decide authoritative deep-page policy.
- [ ] Map no-localized-row nulls to owner public title/handle placeholders in final projection.
- [ ] Batch-hydrate Taxonomy tag names after the Product page is fixed.
- [ ] Actualize historical retained Product PostgreSQL packets to routing key `4`.
- [ ] Extend folded linked paths only with dedicated target-availability evidence.
- [ ] Execute/admit current replacement Product PostgreSQL evidence.
- [ ] Stage/rebuild/promote Product key `4` for a tenant.
- [ ] Move Storefront traffic only after every parity/readiness/freshness/restart/latency gate passes.

## Next source-code step

Build the separate Product Storefront EAV PostgreSQL packet on current routing key `4`. Keep it independent
from the localized core packet so a failure identifies Product term resolution/materialization rather than
locale folding or page ordering. Do not execute the packet in this implementation turn.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
