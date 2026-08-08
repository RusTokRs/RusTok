# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked after core Product Storefront PostgreSQL packet merge `6e635d890503443622b41ebfe203c5d5682e7333` and continued on
`agent/index-storefront-eav-equivalence-postgres-20260808`.

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
- non-serving owner-first `ProductStorefrontIndexShadowExecutor`;
- current-key core Product Storefront owner-vs-shadow PostgreSQL packet source;
- separate current-key Product Storefront EAV owner-vs-shadow PostgreSQL packet source.

The core packet retains localized projection/search, identity de-duplication, equal-timestamp ordering,
page/count and public-channel scenarios. The EAV packet isolates Product term resolution/materialization and
retains integer, localized requested/fallback, Select/Multiselect option code, direct option UUID and
missing/nil option `Never` scenarios. Both remain source-only until maintainer execution.

The core packet also records a public projection gap: owner uses `Untitled product`/empty handle when no
requested/fallback translation exists, while generic localized Index returns null. Final Storefront
projection must apply public placeholders only after Product page identity/order/count are fixed.

## Remaining Storefront parity/evidence blockers

- execute/review both current-key Storefront PostgreSQL packets; source presence is not evidence admission;
- owner title search has no explicit length bound vs Index `TextLike` 1024-byte bound;
- owner/default PostgreSQL collation vs Index deterministic `COLLATE "C"`;
- channel-less owner visibility cannot currently be represented exactly by `sales_channel_ids`;
- owner page depth exceeds Index bounded offset depth;
- map localized null title/handle to owner public placeholders in final Storefront projection;
- localized Taxonomy tag names must be hydrated after Product page identity/count is fixed;
- shadow execution has no serving latency/deadline policy and must remain non-serving;
- stale locale/readiness/admission/restart evidence still requires maintainer execution/extension.

## Retained Product evidence debt

Historical PostgreSQL packets must still be mechanically actualized to routing key `4` / current 15-field
Product contract; never add a key-3 runtime alias. Keep those large fixture rewrites separate from the new
Storefront packets.

After the current packets are executed, use observed evidence to decide the next correction/admission slice:
search collation/bounds, placeholder projection, channel-less visibility, deep-page policy or restart/readiness.

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
- [x] Retain Product EAV owner-vs-shadow PostgreSQL packet source.
- [ ] Execute/review the core Storefront PostgreSQL packet.
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

Mechanically actualize historical retained Product PostgreSQL packets to current routing key `4` / 15-field
fixtures without adding runtime compatibility. Keep that mechanical debt separate from Storefront parity.
Do not execute tests or PostgreSQL packets in this implementation turn.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
