# rustok-inventory

## Known Limitations / Pending Implementation

- Native Product/Variant deletion now has a bounded, tenant-proved empty-bookkeeping guard in
  source: any reservation/shipment/return reference refuses, and stocked/reserved/incoming
  must each be zero at every original level. Missing identity never means absent stock.
  Both real Product callers share the concrete transaction and propagate exact-effect failures.
  This is **not** stock disposal, history purge, general SKU retirement or an Inventory Index
  tombstone. The additive I026 reference indexes are unexecuted; normal index creation may block
  writers, so rollout needs an operator-controlled window and real query-plan/locking evidence.
  All 31 new Rust/10 Node definitions are uncompiled/unrun. Historical migrations and receipt
  keys are preserved; arbitrary SQL/cascades/downgrades are not certified by this service guard.
  See the [owner contract](CRATE_API.md#catalog-deletion-safety-2026-11-01-source-only),
  [source plan](docs/implementation-plan.md#catalog-deletion-safety-2026-11-01-source-only) and
  [source packet](contracts/evidence/catalog-deletion-source.json). Generic Index and step 8 remain open.
- The anonymous quantity reserve/release prerequisite is corrected in source: bounded proved
  pool only, exact original locations, protection of all active claims, shared item write locks
  and exact-one conditional effects. Explicit Cart/Order/line selectors now refuse instead of
  being discarded; keyed allocations remain on the identity port. Quantity operations are
  non-idempotent and have no durable result receipt. All 27 new Rust/10 Node regression
  definitions remain uncompiled/unrun. See the [owner contract](CRATE_API.md#anonymous-quantity-reservations-2026-11-01-source-only).
- Reservation invalidation is now wired in source: the quantity-free sealed
  `inventory.reservation.changed` v1 fact uses current context and shares the counter/receipt
  transaction. Memory/module listeners and the `outbox_iggy` local fanout await explicit typed
  handlers, including Search refetch. Identity/quantity results share bounded checked signed
  availability, without per-level saturation. The additional 34 Rust/10 Node definitions are
  uncompiled/unrun. Canonical Rust digests, SQL/real relay/Iggy/restart/concurrency evidence,
  generic Index and Product step 8 remain open. Native quantity deadline metadata is admission,
  not proof of elapsed-time cancellation. See the
  [owner contract](CRATE_API.md#reservation-invalidation-facts-2026-11-01-source-only).

- Return-restock tenant/Variant/allocation and exact-replay identity is now hardened in source,
  using the existing native command/ledger and selected Order binding read. Compilation,
  SQL/locking and regressions are unrun. This does not prove historical shipment/cancellation
  unit movements or full event/Index invalidation. A separate bounded shipment/cancel/reopen
  source implementation now owns tenant/Variant/original-allocation proof and native history;
  missing historical cancelled/restore proof refuses rather than being reconstructed.
  Fresh return stock requires the saved location to remain unarchived under the existing
  storage rule; location locks precede level locks. An archived receiving location parks
  same-identity reconciliation; exact archived receipt replay still changes no state.
  No automatic reactivation/relocation or operator reactivation workflow is added.
  See the [return identity handoff](../rustok-commerce/docs/implementation-plan.md#return-restock-identity-source-evidence).

- Product step-8 source filters new identity allocations to active tenant-owned channel-visible
  locations with a 4096+1 candidate probe, preserves exact receipt replay/release, and routes
  both public port paths through the admitted wrappers. Regression sources are unrun; no Rust,
  SQL/concurrency, selected remote, mounted or deployment evidence is claimed.
- Reservation numeric outcomes remain global/descriptive, not scoped stock guarantees. The
  complete shipment/return/cancellation, event/outbox and Index convergence chain remains to
  verify; the earlier selection block itself added no publisher. The reservation-fact
  continuation is source-wired, not runtime-certified. See the
  [implementation plan](docs/implementation-plan.md#product-downstream-stock-continuation-2026-10-30-source-work).
- Identity adoption now checks full-batch physical metadata capacity and positive deadlines in
  source. The new PostgreSQL/SQLite byte projection, lock/rollback behavior and recovery flow
  are uncompiled/unexecuted; metadata-capacity refusal needs explicit owner-governed reconciliation,
  not automatic compaction or a hidden temporary-storage retry.
- Historical migrations, stored reservations and stock links are not rewritten or retired.

## Checkout reservation adoption

Inventory now owns the bounded identity-port handoff from saved Cart-line reservations to Order
lines. Commerce supplies the selected capability and owns only its separate journal/checkpoint.
Exact replay preserves allocation, quantities and owner timestamps; no stock is allocated or moved.
Known prior adoption/compaction provenance is retained without a historical rewrite. Unverified
handoff is recovered with the same IDs or parked for reconciliation, never inferred from retry
advice. See the [source-only contract and remaining gates](docs/implementation-plan.md#reservation-adoption-into-order-2026-10-31-source-work).
Tests and Rust/database/runtime verification are not claimed complete; step 8 remains open.

## Purpose

`rustok-inventory` is the default inventory submodule of the `Ecommerce` family.

## Responsibilities

- Own the inventory service, native admin stock write helpers, stock-level migrations, and normalized stock and reservation persistence.
- Keep `stock_locations`, `inventory_items`, `inventory_levels`, and `reservation_items`
  as the source of truth for ecommerce inventory runtime.
- Back admin product, variant, price, stock, and translation visibility through canonical
  Product catalog service queries and inventory-owned stock levels; variant availability is read from
  `inventory_items`/`inventory_levels` when stock levels exist, with the legacy variant
  quantity used only as a compatibility fallback.
- Provide a module-owned Leptos admin UI package in `admin/` for inventory visibility,
  low-stock triage, stock-health inspection, targeted set-quantity corrections, +/-1 adjustments, reserve/release reservation transport coverage, and availability validation.
- Treat the backorder policy value `continue` case-insensitively across service write results,
  set/adjust/reserve/check-availability guardrails, admin read-side stock state, and
  commerce checkout/storefront compatibility paths through the exported policy helper.
- Own public-channel inventory visibility/projection helpers (`normalize_public_channel_slug`,
  channel-visibility metadata parsing, channel-visible available quantity loaders, and
  `PublicChannelInventoryProjection` / `PublicChannelInventoryVariantProjectionInput`) consumed
  by the umbrella commerce storefront/checkout compatibility layer so commerce adapters do not
  duplicate backorder policy branching.
- Own the transaction-aware `BootstrapService` contract used when product creates a new variant:
  default location creation, initial item/level creation, guarded empty-Variant bookkeeping deletion, and batched
  available-quantity reads stay in the inventory module even when the caller owns the
  surrounding transaction.

## Shipment and return stock movements

`shipment_stock` consumes shipped units from the order line's reservation and restores or
reconsumes them when a shipped fulfillment is cancelled or reopened, on the caller's transaction
(replaces the former `fulfillment_inventory_consume_on_ship` trigger). `return_restock` puts
operator-selected returned units back on the shelf and owns the idempotency ledger
`inventory_return_restocks`. Return restock now requires explicit saved SKU proof, refuses
missing/ambiguous tracked allocation and correlates exact ledger replay; it does not treat an
absent reservation as untracked.

The new native shipment protocol accepts canonical context and bounded saved nullable-Variant
batches. Preparation probes immutable proof keys in a rolled-back transaction; actual movement,
`inventory_shipment_stock_proofs` and F progress/checkpoint use the caller's single transaction.
It preserves the original allocation/location, checked cumulative shipped/outstanding restored
units and terminal facts. Missing/ambiguous history, changed identity, archived location and
insufficient unreserved restored units refuse, without relocation or historical backfill.
Retained returned goods conservatively block restoration/reconsumption; outstanding restored
shipment units block fresh returns. Exact return receipt replay precedes the new dependency.
The same admitted budget includes awaited locks/reads, with checks before/between writes and
before return. Expiration rolls back; preparation never retains its probe or refreshes the lease.

Inventory introduces only additive `m20261101_000025_create_shipment_stock_proofs`, unexecuted;
nonempty down refuses. Coordinated rollout also needs F's forward-only F121 after F120;
drain old stock/provider writers before installing the two migrations/enabling matching binaries. Existing return-item PK and applied migration payloads are unchanged.
This is source work, not compilation/SQL/runtime certification or full step-8 completion. See
the [source handoff](../rustok-commerce/docs/implementation-plan.md#shipment-stock-provenance-source-evidence)
and [owning amendment](../../../DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md#shipment-identity-and-native-movement-provenance-amendment-2026-11-01).

## Interactions

- Depends on `rustok-commerce-foundation` for shared commerce DTOs, entities, and errors.
- Depends on `rustok-product` data model through variant references.
- Used by `rustok-commerce` as the umbrella/root module of the ecommerce family.
- Used by `rustok-product` only through the native `BootstrapService` exception while no
  GraphQL/REST bootstrap contract exists; the product creation transaction remains atomic.
- `apps/admin` consumes `rustok-inventory-admin` through manifest-driven composition;
  the admin package now routes Leptos UI through a private `transport/` facade and explicit native server-function adapter backed by canonical Product catalog and Inventory services, with the previous transitional commerce GraphQL adapter and pre-FFA `api.rs` facade removed, and uses native inventory-owned set/adjust/reserve/release
  quantity write endpoints plus check-availability validation for targeted stock corrections,
  +/-1 adjustments, reservation flows, and availability checks; set-quantity targets available
  quantity while preserving reserved units, while remaining non-admin write parity is split
  from the umbrella commerce surface.

## Entry points

- `InventoryModule`
- `InventoryService`
- `InventoryReservationIdentityPort` (reserve, release and owner-local Order adoption)
- public-channel inventory visibility/projection helpers exported from `services::public_channel`
- `rustok-inventory-admin`

See also `docs/README.md` and [CRATE_API.md](CRATE_API.md).

## Native stock invalidations (2026-11-01; source only)

Native shipment/restore/reconsume and new returned stock now couple the existing root
`InventoryUpdated` outbox invalidation with their local physical mutation transaction.
Preparation/replay/SKU-less emit none; equal available counters after physical shipment still
invalidate. Quantities remain descriptive global availability, not location deltas. Canonical
publication failure requires rollback; missing/foreign locations and unrepresentable v1
quantities refuse. [API and failure semantics](CRATE_API.md#native-stock-outbox-2026-11-01-source-only)
and [owner plan](docs/implementation-plan.md#native-stock-outbox-continuation-2026-11-01-source-only)
record 16 additional Rust/10 Node definitions, all unrun. Existing Search refetch is traced,
not runtime-certified. Ordinary reservation facts are now separately source-wired; their real
delivery/convergence, generic Index and Product step 8 remain open. There is no new migration/event schema or distributed atomicity claim.

## Canonical quantity writer prerequisite (2026-11-01; source only)

Native set/adjust and Product's real quantity-update hook now delegate to one Inventory-owned
bounded plan: current tenant/actor/SKU, saved positive identities, complete 4096 levels, original
location/current binding proof, checked signed arithmetic and exact effects. Quantity is global
available stock; only the existing active owner-default level's stocked counter changes.
Reservations/incoming/history/metadata/other locations are preserved. Missing state refuses;
no-op stock plans are read-only. Existing v1 stock notifications are preflighted and canonically
published on the same transaction; no negative snapshot is clamped or wire-broadened.

See the [source packet](contracts/evidence/quantity-writes-source.json) and [owner contract](CRATE_API.md#canonical-quantity-writes-2026-11-01-source-continuation).

### Known Limitations / Pending Implementation

MySQL quantity writes require active explicit-transaction instrumentation and SELECT access to
Performance Schema metadata; session defaults are never treated as proof. Missing/inactive
metadata or unsupported access refuses without changing transaction isolation. Real MySQL
metadata/isolation evidence remains unrun.

33 Rust/10 Node focused definitions remain uncompiled/unrun; direct source checks are not SQL,
concurrency, client or runtime evidence. Native stock commands remain non-idempotent and current
Inventory client redaction is unchanged. No baseline, revision/tombstone, generic Index, full
Next/Leptos recovery parity or Product step-8 readiness is claimed.

## Inventory source-history owner continuation (2026-11-01)

Explicit paged current-state baseline, positive Inventory revisions, immutable source-change
journal and retained tombstones are being wired through actual owner writers. Installation
history is never inferred and a missing owner load is not a tombstone. See the
[owner contract](CRATE_API.md#inventory-source-history-2026-11-01-source-continuation) and
[plan](docs/implementation-plan.md#inventory-source-history-continuation-2026-11-01-uncompiled).

### Known Limitations / Pending Implementation

This owner slice is uncompiled and all regression/SQL/migration/locking evidence is unrun.
At the owner-history handoff canonical versioned publication/relay was not yet wired; the
2026-11-02 continuation below adds that source slice. Selected Distribution source/event/worker,
persist-before-ACK, Index convergence and native/HTTP/GraphQL/Next/Leptos baseline-operator
parity remain pending. Native APIs are not a UI/transport cutover or authorization bypass.
Baseline construction intentionally pauses actual Inventory writes until its bounded pages
finish; there is no automatic reset/cancel/history adoption. Only PostgreSQL READ COMMITTED
and SQLite baseline operation are supported. No Index or Product step-8 readiness is claimed.

Source-history static checks: 19 of 24 direct read-only verifiers pass; five failures are
byte-identical to the hash-proven dirty entry (not certified clean-main failures). Rust formatter
and syntax-only checks cover 19 changed Rust files; no compilation or test evidence follows.
Native source APIs enforce the current budget using the core helper. Live keyset scan is not
an absence/commit-order certificate; versioned canonical convergence remains unwired.
The deletion continuation proves source parent/item/level/location bindings before the first
physical DELETE; its private prepared proof borrows the caller transaction through tombstone
CAS. Do not start a live baseline before operator preflight/recovery for unsupported/oversized
state and creator authority is available. This slice has no automatic reset, repair or abort.

## Canonical source delivery continuation (2026-11-02; source only)

Actual tracked creation/quantity/reservation/physical/deletion writes now add sealed
`inventory.source.changed` on the caller transaction, with retained change UUID and exact
positive Inventory revision. Current actors cross both Product bootstrap/deletion paths
explicitly; journal/baseline/receipt metadata is not authority. Replay/no-op/preparation/adoption
emit no revision/fact. Building seeds emit no per-SKU ready notification. Only the final page's
checkpoint/receipt/activation publishes `inventory.source.baseline_activated`, using its original
globally unique baseline UUID, including an explicitly empty baseline. Page replay emits none.

Required module delivery refuses unmatched Inventory frames before broker-only success; failures
remain retry/DLQ, not successful indexing. [Contract](CRATE_API.md#inventory-source-canonical-delivery-2026-11-02-source-continuation),
[plan](docs/implementation-plan.md#source-canonical-delivery-continuation-2026-11-02-uncompiled),
[packet](contracts/evidence/source-delivery-source.json). 26 new Rust/10 Node definitions remain
uncompiled/unrun. The previous source-history packet remains historical owner-source evidence.

### Known Limitations / Pending Implementation

At this publication handoff the selected consumer was absent. The selected-consumer continuation
below now authors that source/listener/initial enqueue/worker composition, without deployment or
runtime verification. Full native/
HTTP/GraphQL/Next/Leptos operator recovery remains pending. Already-active older baselines may
lack activation/earlier facts; reads never fabricate historical delivery. Explicit operator
reconciliation/cutover is still required. I027 must precede matching binaries; historical
migration/root/reservation payloads and release artifact are unchanged. No live baseline before
real handlers and operator preflight/recovery for unsupported/oversized state or unavailable
creator authority. No automatic reset/abort/repair. Cargo/digest/tests/SQL/backend/concurrency/
Iggy/restart/runtime and Index/Product step-8 gates remain unverified.

Canonical-delivery source checks: 20 of 26 direct read-only guards pass. Six refusals already
exist in the hash-proven dirty task entry: five outputs are byte-identical, and one Product
error-boundary exception differs only in its absolute snapshot-root stack path. These are not
clean-main comparisons. Twenty changed Rust files pass zero-change Scalar/syntax-only parsing;
four MJS syntax checks, three JSON parses, 26 Prettier checks, task-delta whitespace and 23 new
local link targets pass. All 669 entry migration Rust files, Cargo manifests/lock and 21 pinned
historical migration/root/reservation/digest files remain unchanged. Architecture metadata is
blocked by absent Cargo. None of this is compilation, tests, SQL, locking or runtime evidence.

## Selected Inventory Index consumer (2026-11-02; source only)

The [selected Distribution bridge](../rustok-distribution/docs/inventory-index.md) now authors
`inventory-stock-primary` / `rustok-inventory/stock`, real sealed Changed/activation listeners,
current authenticated native reads, durable generic persistence before Changed success and
original-baseline-UUID replay enqueue before activation success. The canonical Index runtime
registers bounded generic ModuleWork replay with exact job/checkpoint fences; no event actor,
baseline creator, receipt, journal or saved claim grants source authority. Old redelivery keeps
the currently loaded owner's immutable mutation UUID. Product/other remote consumers remain.
[Current source evidence](../rustok-inventory/contracts/evidence/source-index-consumer-source.json)
supersedes only the missing-consumer-code boundary of preceding source handoffs.

### Known Limitations / Pending Implementation

The runner still parks page failures terminally. Automatic failed-job backoff and authorized
original-job recovery remain unwired; crash/lease expiry and successful bounded yield resume
without UUID/checkpoint replacement. Lease attempts are not failure-only counters. Real credential/
policy/secret rotation, schema/digest/backend/concurrency/broker/restart/deployment evidence and
full native/HTTP/GraphQL/Next/Leptos operator recovery remain pending. No historical delivery is
fabricated; no live baseline/cutover before preflight/recovery. Tests/build/SQL/migrations/runtime
were not executed. Source/worker presence or initial enqueue is not Index or Product step-8 readiness.
