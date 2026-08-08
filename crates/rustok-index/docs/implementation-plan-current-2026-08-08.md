# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked after Product Storefront public projection merge
`f949c7c3ee25dcabbf33ff2b7627fae1ea3b9e3b` and continued on
`agent/product-storefront-tag-hydration-20260808`.

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
  channel-less owner requests remain owner-native and malformed identity fails closed;
- explicit deep-page policy: owner-valid offsets through `10_000` are shadow-eligible while deeper offsets
  remain typed owner-native without clamp or cursor rewriting;
- Product Storefront post-page public placeholder projection that keeps raw Index null evidence intact;
- Product-owned bounded post-page tag hydration keyed by already-selected Product IDs, preserving Taxonomy
  requested->fallback/canonical-key semantics **and** legacy metadata-only tag fallback;
- current-key core/EAV Storefront PostgreSQL packet source;
- historical retained Product PostgreSQL fixtures actualized to current Product routing key `4`.

## Request-shape owner-native policies

### Channel-less

Owner channel-less semantics are **metadata-unrestricted only**. Current key `4` cannot distinguish unrestricted
metadata from a restricted Product that resolves to the same complete current Channel UUID set. Channel-less
therefore remains owner-native; trusted slug/UUID scope remains shadow-eligible; malformed identity fails closed.

### Deep page

The Product owner has no generic Index-style maximum offset. The Index path is bounded to `10_000`. Valid
offsets above that bound remain typed owner-native after owner success; no clamp or cursor rewrite is used.

## Post-page Product projection layers

### Public title/handle placeholders

The raw generic `IndexQueryPage` remains Product-neutral. When requested/fallback localized rows are absent,
root `title`/`handle` remain `IndexValue::Null` in `projected` and in raw PostgreSQL evidence.

`public_projected` is derived from a clone of successful raw `projected` and maps only:

- `title: Null` -> `"Untitled product"`;
- `handle: Null` -> `""`.

It preserves item identity/order, count, page boundary, cursor, unrelated fields and `tag_ids`; missing,
duplicate or wrong-typed title/handle fields fail closed. Raw comparison still uses `projected`.

### Product-owned tag hydration

A naive `tag_ids -> Taxonomy names` adapter is insufficient. Current Product Index `tag_ids` are materialized
only from `product_tags`, while Product owner read semantics still support legacy `metadata.tags` when a Product
has no tag relations.

`ProductStorefrontTagReadPort` therefore accepts only the **already-selected Product IDs** plus fallback locale.
The embedded Product runtime wires `CatalogService` as this optional capability; external runtimes do not gain
an implicit embedded fallback.

The Product owner capability:

- accepts at most 48 unique, non-nil Product IDs;
- tenant-scopes and verifies every requested Product identity;
- reuses `CatalogService::load_product_tag_map` rather than reimplementing storage rules;
- preserves product-tag relation ordering;
- uses existing `TaxonomyService::resolve_term_names` requested->fallback resolution and canonical-key fallback;
- preserves legacy normalized `metadata.tags` when no relations exist;
- returns results in the same Product-ID order supplied by the fixed raw Index page.

`ProductStorefrontIndexShadowExecution.tag_hydration` is populated only after raw `projected` succeeds. The
executor extracts Product IDs from `projected.items`; distribution never reads Product/Taxonomy storage or
constructs `TaxonomyService`. Missing external capability or owner hydration error is retained separately and
cannot mutate/replace raw or public projected pages.

This closes the Storefront tag **source hydration boundary** without copying localized tag names into Product
Index schema and without pretending `tag_ids` represent legacy metadata-only tags.

## Remaining Storefront parity/evidence blockers

- execute/review current-key Storefront, collation and actualized retained Product PostgreSQL packets;
- admit collation parity only where the retained deployment matrix agrees;
- define serving latency/deadline/budget policy for owner-first Index + post-page Product tag hydration before
  any shadow-to-serving transition;
- preserve channel-less and deep-page owner-native routing in any future serving composition;
- complete maintainer-executed stale locale/readiness/admission/restart evidence.

## Retained Product evidence state

The retained Product PostgreSQL fixture set is source-aligned on routing key `4`: locale absence, materialized
query freshness, Product/Channel convergence, Channel identity transitions, linked-target delete/recreate,
linked-target availability equivalence and linked-target replay/redelivery. ProductVariant remains key `2`
and SalesChannel remains key `1`.

Existing Product taxonomy source evidence also retains normalized legacy metadata-tag read fallback. It is not
reinterpreted as Index tag identity evidence.

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
- [x] Batch-hydrate Product tags after raw page selection through a Product-owned capability, including legacy
      metadata-only fallback rather than relying solely on Index `tag_ids`.
- [ ] Execute/review retained Product/Storefront/collation PostgreSQL packets.
- [ ] Admit owner/default vs Index `COLLATE "C"` title-search parity for a deployment.
- [ ] Define/admit serving latency and deadline policy for eligible Storefront Index + owner hydration.
- [ ] Extend folded linked paths only with dedicated target-availability evidence.
- [ ] Execute/admit current replacement Product PostgreSQL evidence.
- [ ] Stage/rebuild/promote Product key `4` for a tenant.
- [ ] Move eligible Storefront traffic only after every parity/readiness/freshness/restart/latency gate passes;
      channel-less and deep-page shapes remain owner-native under the current contracts.

## Next source-code step

Define a non-serving Storefront Index serving-budget contract before any traffic-switch adapter. It must bound
Index execution plus Product post-page owner hydration, preserve owner success/fail-closed behavior, and make
an exceeded/missing deadline a reason to keep the request owner-native rather than introducing unbounded tail
latency. Do not switch mounted Storefront traffic in that slice.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
