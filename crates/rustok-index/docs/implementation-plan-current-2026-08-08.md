# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked after Product Storefront tag hydration merge
`5a37e78d50b37785a2a5c119689887f441a94cf9` and continued on
`agent/product-storefront-serving-budget-policy-20260808`.

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
  requested->fallback/canonical-key semantics and legacy metadata-only tag fallback;
- explicit **post-owner serving-budget policy** that requires host-measured remaining budget, positive bounded
  Index/tag phases, safety margin and selected tag-hydration capability before a future serving path is eligible;
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

`public_projected` is derived from a clone of successful raw `projected` and maps only `title: Null` to
`"Untitled product"` and `handle: Null` to `""`. It preserves item identity/order, count, page boundary,
cursor, unrelated fields and `tag_ids`; raw comparison still uses `projected`.

### Product-owned tag hydration

`ProductStorefrontTagReadPort` accepts only already-selected Product IDs plus fallback locale. The embedded
Product runtime wires `CatalogService` as this optional capability; external runtimes do not gain an implicit
embedded fallback.

It preserves relation ordering, Taxonomy requested->fallback/canonical-key resolution and legacy normalized
`metadata.tags` when relation-backed tags are absent. `tag_hydration` runs only after raw `projected` succeeds
and cannot mutate or replace the raw/public page.

## Post-owner serving-budget policy

`PortContext.deadline_ms` is the original duration budget, not an automatically decreasing remaining deadline.
A future serving router must provide a monotonic host-measured `remaining_ms` at the handoff **after** the
authoritative Product owner call.

`ProductStorefrontIndexServingBudget` contains host-selected:

- positive `index_execution_ms`;
- positive `tag_hydration_ms`;
- `safety_margin_ms`;
- checked `required_ms` sum.

No production SLO numbers are hard-coded by this slice. `classify_product_storefront_index_serving_budget`
keeps the request owner-native when deadline semantics, configured budget, measured remaining time or tag
capability are missing/inconsistent, or when remaining time is below the required bounded phases. Only an
internally consistent observation with sufficient budget returns `Eligible`.

This is a **policy only**. It does not run timers and is deliberately not called by the current shadow executor
or mounted Storefront. Real phase timeout enforcement remains the next source step.

## Remaining Storefront parity/evidence blockers

- execute/review current-key Storefront, collation and actualized retained Product PostgreSQL packets;
- admit collation parity only where the retained deployment matrix agrees;
- add non-serving enforcement of admitted Index/tag-hydration phase timeouts and retain timeout behavior;
- preserve channel-less and deep-page owner-native routing in any future serving composition;
- complete maintainer-executed stale locale/readiness/admission/restart evidence.

## Retained Product evidence state

The retained Product PostgreSQL fixture set is source-aligned on routing key `4`: locale absence, materialized
query freshness, Product/Channel convergence, Channel identity transitions, linked-target delete/recreate,
linked-target availability equivalence and linked-target replay/redelivery. ProductVariant remains key `2`
and SalesChannel remains key `1`.

Existing Product taxonomy source evidence retains normalized legacy metadata-tag read fallback. It is not
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
- [x] Define post-owner serving-budget eligibility using host-measured remaining time and explicit bounded
      Index/tag phases without choosing unevidenced global SLO values.
- [ ] Enforce admitted Index/tag phase timeouts in a non-serving execution adapter.
- [ ] Execute/review retained Product/Storefront/collation PostgreSQL packets.
- [ ] Admit owner/default vs Index `COLLATE "C"` title-search parity for a deployment.
- [ ] Extend folded linked paths only with dedicated target-availability evidence.
- [ ] Execute/admit current replacement Product PostgreSQL evidence.
- [ ] Stage/rebuild/promote Product key `4` for a tenant.
- [ ] Move eligible Storefront traffic only after every parity/readiness/freshness/restart/latency gate passes;
      channel-less and deep-page shapes remain owner-native under the current contracts.

## Next source-code step

Add a **non-serving budgeted execution adapter** that accepts only an `Eligible` budget decision and actually
applies the admitted Index and Product-tag phase timeouts. Timeout/unavailable/error outcomes must remain
separate from the already-successful Product owner result. Do not mount that adapter into Storefront traffic.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
