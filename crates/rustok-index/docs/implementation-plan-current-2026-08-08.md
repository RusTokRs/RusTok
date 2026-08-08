# Current `rustok-index` implementation plan — 2026-08-08

Status overlay continued from `main@500c6f647b5f09f617cb907a43093e4f954b3fed` (#3220) on
`agent/index-localized-identity-order-20260808`.

`implementation-plan.md` remains historical architecture context. This file is the current execution
cursor.

## Recheck result

The Product/Index Storefront sequence now includes:

- #3199: one current 15-field Product contract on internal routing key `4`;
- #3200: owner all-translations title search and requested -> fallback projection mismatch documented;
- #3204: generic localized-entity identity-fold architecture selected;
- #3208: explicit localized query validation and dedicated cursor identity;
- #3210: root-only PostgreSQL localized identity-fold page/count compiler and decoder;
- #3212: canonical PostgreSQL localized runtime with persisted readiness, generic admission and one
  read-only repeatable-read page/count snapshot;
- #3220: generic bounded scalar String `TextLike` with PostgreSQL/reference wildcard semantics.

The adapter recheck after #3220 found one additional query-order mismatch before Product translation can be
claimed: owner descending Storefront order uses both timestamp DESC and Product ID DESC, while the
localized compiler still used a fixed root entity-ID ASC tie-break. This slice closes that localized-only
gap without changing ordinary exact-locale `IndexQuery` semantics.

## Current primary owner gate

`M6 - execute and admit concrete repair PostgreSQL evidence`

The repair implementation/harness/admission source remains complete. Maintainer execution/admission is
still required and is not claimed by source inspection.

## Current Product/Storefront source state

Current Product facts remain:

- one current Product schema, routing key `4`; lower keys are historical only;
- 15 Product fields including Storefront scalars, `attribute_terms`, `variant_ids` and
  `sales_channel_ids`;
- Product replay IDs are schema-scoped;
- EAV writes advance the canonical Product owner clock;
- Product root freshness and ordinary linked-target availability remain fail-closed;
- Product channel visibility convergence materializes resolved current channel UUID membership under a
  freshness witness.

## Localized query/runtime state

Source-complete capabilities now include:

- `LocalizedEntityQuery` requested/fallback roles, `any_locale_filter` and
  `localized_projection_fields`;
- localized cursor wire version `3`, separate from ordinary wire version `2`;
- root-only identity fold over physical locale rows;
- requested -> fallback -> null projection;
- group-before-order/page/count semantics;
- generic owner admission on all participating physical row roles;
- persisted schema readiness plus one `REPEATABLE READ, READ ONLY` page/count snapshot;
- generic bounded scalar String `TextLike` for ordinary and folded filters;
- explicit localized `identity_order_direction` for the final root entity-ID tie-break.

`identity_order_direction` defaults to `Asc`, validates only `Asc|Desc`, is bound into localized cursor and
plan fingerprints, and changes only the localized final identity term:

- Asc: `entity_id ASC`, continuation `entity_id > cursor`;
- Desc: `entity_id DESC`, continuation `entity_id < cursor`.

Ordinary `IndexQuery` and its existing ascending entity-ID tie-break remain unchanged.

## Product adapter blockers discovered during source recheck

The next Product Storefront adapter remains fail-closed on three explicit parity gaps:

1. **Search length** — owner search has no explicit length bound; `TextLike` is capped at 1024 UTF-8
   bytes. The adapter must not silently truncate or reject owner-valid search without a reviewed owner/API
   bound.
2. **Search collation** — owner title `LIKE` uses deployment/default collation while Index String scalar
   SQL uses deterministic `COLLATE "C"`; retained PostgreSQL evidence must establish the admitted contract.
3. **Channel-less visibility** — owner list requests without a public channel admit only metadata-
   unrestricted Products. Current `sales_channel_ids` represents unrestricted as membership in all current
   channels and therefore cannot distinguish unrestricted from a restricted Product that currently
   contains every channel. A channel-less authoritative adapter must fail closed until that distinction is
   materialized or supplied by an owner capability.

For a request carrying a trusted current public channel ID, `Contains(sales_channel_ids, channel_id)` is the
intended root membership predicate under the existing channel resolver/freshness contract.

## Retained Product evidence debt

Historical PostgreSQL packets still need mechanical actualization to routing key `4` / the current
15-field Product contract. Do not add a runtime alias for key `3`.

The localized Storefront equivalence packet must additionally cover:

- requested present / fallback / neither requested nor fallback;
- any-locale title matches, including third-locale matches;
- `%`, `_` and escaped wildcard behavior;
- duplicate locale matches yielding one Product identity and count;
- equal-timestamp ordering under both ascending and descending Product-ID tie-breaks;
- cursor continuation across those ties;
- EAV terms, category and public channel visibility;
- stale locale exclusion/readiness/admission/restart;
- search-bound/collation behavior;
- current Product routing key promotion.

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
- [x] Localized identity/fallback architecture.
- [x] Localized query/cursor contract.
- [x] Localized PostgreSQL compiler/decoder.
- [x] Localized production runtime with readiness/admission/snapshot semantics.
- [x] Generic scalar String `TextLike`.
- [x] Explicit localized entity-ID tie-break direction matching owner Asc/Desc ordering.
- [ ] Implement Product Storefront Index shadow/evidence adapter.
- [ ] Resolve search-length/collation parity.
- [ ] Resolve channel-less unrestricted visibility parity.
- [ ] Resolve Product attribute option-code metadata through an owner capability and emit canonical
  `attribute_terms`.
- [ ] Batch-hydrate Taxonomy tag names after the Product page is fixed.
- [ ] Actualize retained Product PostgreSQL packets to routing key `4`.
- [ ] Retain owner-vs-Index localized PostgreSQL equivalence packet.
- [ ] Extend folded linked paths only with dedicated target-availability evidence.
- [ ] Execute/admit current replacement Product PostgreSQL evidence.
- [ ] Stage/rebuild/promote Product key `4` for a tenant.
- [ ] Move Storefront traffic only after every parity/readiness/freshness/restart gate passes.

## Next source-code step

Build the Product Storefront **shadow/evidence adapter**, not a traffic switch. It should translate only
inputs whose parity is already representable and fail closed for unresolved search/channel cases. The
adapter must map owner sort direction to both timestamp ordering and localized identity tie-break direction,
resolve Product EAV metadata through Product-owned capabilities, and leave Taxonomy hydration after page
selection.

In parallel, actualize historical Product PostgreSQL packets to routing key `4` / current 15-field source.

## Maintainer verification after this slice

The implementation agent has not run these commands. Maintainer verification should include:

```bash
node scripts/verify/verify-index-localized-identity-order.mjs
node scripts/verify/verify-index-localized-query-contract.mjs
node scripts/verify/verify-index-localized-query-postgres-fold.mjs
node scripts/verify/verify-index-localized-query-runtime.mjs
node scripts/verify/verify-index-text-like-filter.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
