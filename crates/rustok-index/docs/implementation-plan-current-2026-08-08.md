# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked after real Product Storefront deep-page policy merge
`ac399264f12dd71439861e60a5df86ac9ab496bb` and continued on
`agent/product-storefront-placeholder-projection-v2-20260808`.

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
- generic String `TextLike` plus the Product-owned 1022-byte effective Storefront search bound;
- retained owner/default-vs-Index-`C` PostgreSQL title-search collation packet source;
- localized Product-ID tie-break direction matching owner Asc/Desc ordering;
- Product-owned Storefront attribute-filter -> neutral canonical term resolution;
- pure Storefront shadow builder and owner-first non-serving shadow executor;
- explicit current-key channel-scope policy: trusted non-empty slug + non-nil UUID is shadow-eligible, while
  channel-less owner requests remain owner-native and partial/invalid channel identity fails closed;
- explicit deep-page policy: owner-valid offsets through `10_000` are shadow-eligible while deeper offsets
  remain typed owner-native without clamp or cursor rewriting;
- Product Storefront **post-page public projection**: raw generic Index rows remain intact for equivalence
  evidence, while a derived page maps root `title: null` to `"Untitled product"` and root `handle: null` to
  `""` only after page identity/order/count/cursor are fixed;
- current-key core/EAV Storefront PostgreSQL packet source;
- historical retained Product PostgreSQL fixtures actualized to current Product routing key `4`.

## Request-shape owner-native policies

### Channel-less

Owner channel-less semantics are **metadata-unrestricted only**. Current key `4` stores unrestricted visibility
as membership in all current Channel UUIDs, which is indistinguishable from a restricted Product that currently
resolves to the same complete set. Therefore:

- absent/blank slug + absent UUID => `OwnerNativeChannelLess`;
- trusted non-empty slug + non-nil UUID => channel-scoped shadow eligible;
- malformed/partial identity => fail closed.

No visibility sentinel or key-5 approximation is used.

### Deep page

The Product owner accepts valid `page/per_page` without the generic Index offset ceiling. The Index path is
bounded to offset `10_000`. Therefore:

- offset `<= 10_000` => shadow eligible;
- offset `> 10_000` => typed `OwnerNativeDeepPage` / `DeepPageOwnerNative` after owner success;
- invalid pagination/overflow => fail closed as invalid pagination.

No clamp or cursor rewrite is used.

## Post-page public placeholder projection

The generic localized Index decoder deliberately remains Product-neutral. When neither requested nor fallback
translation row exists, raw root `title`/`handle` remain `IndexValue::Null`. Existing PostgreSQL parity evidence
continues to retain that raw state.

`project_product_storefront_index_page` is a distribution-owned Product adapter applied only to a clone of a
successful raw `IndexQueryPage`. It:

- maps root `title: Null` -> `String("Untitled product")`;
- maps root `handle: Null` -> `String("")`;
- preserves already-present strings;
- fails closed on missing, duplicate, or non-string/non-null root title/handle projections;
- preserves item identity/order, exact count, page boundary, cursor, unrelated projected fields and `tag_ids`.

`ProductStorefrontIndexShadowExecution.projected` remains the raw generic Index page. New
`public_projected` is derived after raw execution. Identity/count/page comparison still uses raw `projected`, so
owner public placeholders cannot affect filtering, sorting, identity folding, exact count, pagination, or cursor
construction.

This closes the no-localized-row public placeholder **source adapter** gap. It does not yet produce final
Storefront tags: `tag_ids` remain IDs until the separate Taxonomy hydration slice.

## Remaining Storefront parity/evidence blockers

- execute/review current-key Storefront, collation and actualized retained Product PostgreSQL packets;
- admit collation parity only where the retained deployment matrix agrees;
- batch-hydrate localized Taxonomy tag names only after Product page identity/count is fixed;
- define serving latency/deadline policy before any shadow-to-serving transition;
- preserve channel-less and deep-page owner-native routing in any future serving composition;
- complete maintainer-executed stale locale/readiness/admission/restart evidence.

## Retained Product evidence state

The retained Product PostgreSQL fixture set is source-aligned on routing key `4`: locale absence, materialized
query freshness, Product/Channel convergence, Channel identity transitions, linked-target delete/recreate,
linked-target availability equivalence and linked-target replay/redelivery. ProductVariant remains key `2`
and SalesChannel remains key `1`.

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
- [x] Generic scalar String `TextLike` and Product-owned compatible search bound.
- [x] Retain owner/default-vs-Index-`C` PostgreSQL title-search collation packet source.
- [x] Explicit localized entity-ID tie-break direction matching owner Asc/Desc ordering.
- [x] Product Storefront shadow query builder and Product-owned EAV resolution.
- [x] Compose non-serving Product-owner + Index shadow executor.
- [x] Retain current-key core/EAV owner-vs-shadow PostgreSQL packet source.
- [x] Actualize historical retained Product PostgreSQL packets to routing key `4`.
- [x] Keep channel-less Storefront owner-native on current key `4`; distinguish malformed channel identity.
- [x] Keep owner-valid offsets above `10_000` owner-native without clamp/rewrite.
- [x] Map raw no-localized-row title/handle nulls to Product public placeholders in a post-page derived layer.
- [ ] Execute/review retained Product/Storefront/collation PostgreSQL packets.
- [ ] Admit owner/default vs Index `COLLATE "C"` title-search parity for a deployment.
- [ ] Batch-hydrate Taxonomy tag names after the Product page is fixed.
- [ ] Extend folded linked paths only with dedicated target-availability evidence.
- [ ] Execute/admit current replacement Product PostgreSQL evidence.
- [ ] Stage/rebuild/promote Product key `4` for a tenant.
- [ ] Move eligible Storefront traffic only after every parity/readiness/freshness/restart/latency gate passes;
      channel-less and deep-page shapes remain owner-native under the current contracts.

## Next source-code step

Add a post-page Product Storefront Taxonomy tag hydration boundary. The Index page already carries `tag_ids`;
resolve their requested-locale -> fallback-locale names in one bounded/batched owner capability **after** page
identity/order/exact-count are fixed. Hydration failure must not alter the raw Index page or be interpreted as a
different Product page. Do not add Taxonomy tag names to Product Index schema merely to avoid owner hydration.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
