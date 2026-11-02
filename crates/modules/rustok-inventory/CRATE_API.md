# rustok-inventory / CRATE_API

## Public Modules

- `entities`: SeaORM entity models for inventory items, inventory levels, stock locations, and reservations.
- `migrations`: SeaORM migration definitions for inventory tables.
- `module`: Core `InventoryModule` definition and platform registration.
- `ports`: Canonical availability/reservation contracts and admitted wrappers, explicitly re-exported at crate root. Persistence implementation is private (`ports_impl`).
- `return_restock`: Return restock logic and idempotency handling.
- `services`: Domain services for stock adjustments, reservations, and public channel inventory projections.
- `shipment_stock`: Shipment stock movement ports and consumption logic.

## Platform Capability Bindings

- `InventoryModule` implements `rustok_core::module::RusToKModule` and `MigrationSource`.
- Declared as `ModuleKind::Optional`.
- Self-check returns `HealthStatus::Healthy`.
- Slug: `inventory`.
- Dependencies: `product`.
- Permissions: `INVENTORY_CREATE`, `INVENTORY_READ`, `INVENTORY_UPDATE`, `INVENTORY_DELETE`, `INVENTORY_LIST`, `INVENTORY_MANAGE`.

## Domain Collaborations

- Default inventory submodule of the ecommerce family.
- Owns inventory stock levels, stock locations, and reservation records.
- Provides `InventoryReservationPort` and `InventoryReservationIdentityPort` for checkout reservation lifecycle.
- Provides `BootstrapService` for transactional initial stock creation during product/variant creation.
- Implements `StockLocationTranslationTargetProvider` for localized stock location titles.
- Leptos admin package in `admin/` (`rustok-inventory-admin`).

## Minimum Contract Set

### Input DTOs/Commands

- `InventoryIdentityReservationAdoptionRequest` / `InventoryIdentityReservationAdoptionItem`:
  nonempty batch up to 1024 saved reservation/external/Variant/quantity/location and Cart-/Order-line
  bindings, correlated by Cart, checkout operation and Order. See the adoption contract below.
- `InventoryAvailabilityRequest`: variant_id, requested_quantity, optional channel_slug. Request/context
  channel selectors must agree after normalization; quantity zero is a valid stock probe.
- `InventoryReservationRequest`: variant_id, quantity, optional cart_id and line_item_id.
- `InventoryIdentityReservationRequest`: reservation_id, external_id, variant_id, quantity,
  optional line_item_id and metadata. Location is selected by the owner, not supplied by a client.
- `InventoryReservationReleaseRequest`: variant_id, quantity, optional order_id and line_item_id.
- `InventoryIdentityReservationReleaseRequest`: reservation_id, external_id.
- `InitialInventory`: variant_id, quantity, optional location_id.
- `ReturnRestockLine`: return_item_id, return_id, order_line_item_id, explicit saved
  variant_id (optional only for a proved SKU-less Order line), positive quantity. Location
  is resolved from Inventory's saved allocation/receipt, never supplied as a replacement.

### Domain Invariants

- Denies float arithmetic; stock quantities are exact signed/unsigned integers.
- Positive identity-reserve quantities and exact integer arithmetic; quantity-based commands
  preserve their existing non-negative semantics. Backorders may exceed on-hand stock;
  signed available quantities can be negative and are not channel-eligibility proofs.
- Idempotent reservations: `(reservation_id, external_id)` pairs uniquely identify durable reservations; re-reserving with identical parameters is replay-safe.
- Case-insensitive backorder policy: `continue` policy permits reservations beyond on-hand quantity.
- Tenant isolation enforced across all database queries, levels, and reservations.

### Identity allocation and recovery (2026-10-30; source only)

- Initial allocation probes 4096+1 candidate levels and refuses overflow. It retains only active
  tenant-owned locations visible for the trusted channel, even for `continue` backorders.
- Exact durable replay precedes new selection and preserves the original allocation. Release
  is independent of current visibility/archival. `allocated_channel_slug` is owner-controlled
  metadata on new rows only, not a new identity dimension or a historical rewrite.
- Crate-root and `ports` factories/concrete wrapper paths use the same admitted diagnostic
  boundary; raw persistence construction is private. Request and error payloads are not log facts.
- The quantity-based port remains distinct from durable identity allocation. Its numeric write
  outcomes are descriptive global state, not scoped reservations or current-channel proofs.
- Source regressions and static guards are not SQL, concurrency, mounted/native or remote
  certification. See [remaining work](docs/implementation-plan.md).

### Events / Outbox Side Effects

- Inventory service physical-stock writes retain the root stock event contract. Fresh positive
  identity and anonymous reserve/release append `InventoryReservationEvent::Changed` on the
  same counter/receipt transaction; replay/zero/adoption append none. See the source-only
  reservation-fact contract below; runtime projection/delivery evidence is still open.
- Durable reservation rows retain identity/replay/release state; do not infer receipts for
  quantity-only operations.

### Errors / Failure Codes

- `CommerceError::NotFound`: stock location, inventory item, or level not found.
- `CommerceError::Validation`: nil UUID, negative quantity, or invalid reservation parameters.
- `CommerceError::InsufficientStock`: requested quantity exceeds available stock when backorder is disabled.
- `CommerceError::Conflict`: reservation external_id conflict with mismatched parameters.
- `CommerceError::Database`: underlying storage failure.

## Reservation adoption into Order (2026-10-31; source continuation)

`InventoryReservationIdentityPort::adopt_inventory_by_identity` accepts a nonempty batch of at
most 1024 `InventoryIdentityReservationAdoptionItem` bindings in one
`InventoryIdentityReservationAdoptionRequest`. Cart/checkout-operation/Order IDs bind the batch;
each row carries original reservation/external identity, stable Variant UUID, exact quantity,
Cart-line/Order-line UUIDs and the **expected existing** location. These are trusted internal
owner commands, not buyer DTOs or arbitrary stock-location selectors.

The owner checks all rows before writes under deterministic item and reservation locks. Exact
replay preserves original allocation, quantities and timestamps, including archived/hidden locations.
Source provenance, current line binding, tenant, external identity, quantity and location must all
agree; inactive/partially consumed/contradictory rows refuse. No allocation, level movement or new
receipt occurs. The response echoes complete correlated bindings only after commit.
Reserved levels must unambiguously cover the complete batch with checked sums; on-hand stock is
not refreshed as an eligibility gate for an existing backorder allocation.

Current allocations keep `source=inventory_identity_port`; known previously adopted
`source=checkout_operation` is accepted only with exact target provenance. The existing compaction
migration retains immutable reservation/external/Variant/Cart-line identity but omits Cart and
operation labels. Only its explicit `metadata_truncated=true` permits those missing/null labels;
contradictory retained labels refuse. Commerce still verifies its immutable journal. No missing
historical label is guessed, filled in or used as a new business source of truth.

Before writing any changed row, the owner measures the complete preserved metadata plus target
fields in one PostgreSQL/SQLite native byte projection against the existing 32 KiB physical limit.
`inventory.adoption_metadata_too_large` is a nonretryable conflict requiring explicit owner-governed
reconciliation, never automatic truncation or endless temporary-unavailable retry. Missing size
proof and unsupported backends refuse explicitly. Exact replay does not resize or rewrite saved
metadata. Both public factory/concrete paths use the same admitted wrapper and canonical positive
deadline semantics; the private persistent implementation is not a public alternative.

Adoption failure policy:

- malformed/nil/duplicate/ambiguous/oversized binding commands refuse before storage;
- `inventory.adoption_identity_conflict`, `inventory.reservation_not_found` and
  `inventory.adoption_ledger_inconsistent` are explicit saved-state refusals, not repair requests;
  non-null terminal disposition/supersession/release/disposal markers also refuse even if quantity
  and deletion flags appear active. Missing/null retained markers are valid; none are cleared;
- `inventory.adoption_metadata_too_large`, `inventory.adoption_metadata_size_unverified` and
  `inventory.adoption_backend_unsupported` are nonretryable and never discard saved payloads;
- transient `inventory.database_unavailable` and consuming timeout retain same-identity recovery.
  A false hint is not a no-effect proof; Commerce parks an unverified handoff rather than automatically
  releasing its allocation. The owner response is correlated command output, not a durable receipt.

Commerce owns its journal/checkpoint separately. An unknown outcome must inspect/recover the same
identity; retryability is not proof of failure or permission to rotate keys. This is owner-local
atomicity, not a cross-owner transaction or runtime certification. Historical schemas/state remain
untouched; full reservation event/outbox and shipment/return convergence are still open.

## Native returned-stock identity (2026-10-31; source continuation)

`return_restock::restock_returned_items(db, PortContext, lines)` is the existing native
Inventory command, now using canonical write/deadline/idempotency admission and `PortError`.
It accepts 1..=1024 distinct return items of one return. Its caller asserts completed
Order-owned return/SKU/quantity facts; Inventory does not read private Order tables.
Commerce enforces the positive call budget around the future.

Every Variant is tenant-proved before tenant-less items are locked in sorted Variant
order. A fresh tracked line needs exactly one item/Order-line allocation, no known terminal
release/disposal/supersession facts, a tenant-owned saved location and exactly one level.
Hidden saved locations do not require fresh publication/channel visibility. Fresh writes require
an unarchived saved location under the existing inactive-location storage invariant; distinct
receiving locations lock in UUID order before any level locks or receipt writes. An archived
receiving location returns `inventory.return_restock_location_archived` as nonretryable
`InvariantViolation`, requiring same-identity reconciliation without implicit reactivation or
relocation. This adds no operator reactivation workflow. Exact replay may still prove an archived
saved location without changing stock. Missing tracked inventory is never SKU-less.
Levels lock in stable item/location order. The existing ledger primary-key fence, checked
grouped addition and stock writes share one native transaction; other counters, reservations
and allocation metadata do not change. No new schema/receipt or historical rewrite exists.

Exact ledger replay compares tenant/return/item/Order-line/item identity and quantity and
proves the saved location's tenant without reselecting a reservation or rewriting timestamps.
The ledger records item/location, so missing item/location identity requires reconciliation,
not invented Variant provenance. A changed quantity is not an incremental top-up.

Operator inspection is still the restock authority. This identity slice does not establish
historical shipment/cancellation movement provenance, certify lifecycle overlap arithmetic,
or add complete event/Index invalidation. Native Inventory atomicity is not atomicity with
Order completion or Commerce. Compilation, regression/SQL/locking and remote execution are
unproven; see the [owner plan](docs/implementation-plan.md#return-restock-identity-continuation-2026-10-31-source-only).

## Native shipment provenance (2026-11-01; source only)

`shipment_stock` is the Inventory-owned native implementation, not a peer-facing Fulfillment
port or an independent journal interpreter. It exports:

- `ShipmentStockBatch { order_id, fulfillment_id, lines }` and `ShipmentStockLine` with F-item/
  Order-line, explicit nullable saved Variant, planned/shipped/delivered and movement quantities;
- `ShipmentStockAction::{Ship, Restore, Reconsume}` and `MAX_SHIPMENT_STOCK_ITEMS = 1024`;
- `validate_shipment_stock_preparation(db, &PortContext, &batch, action)`, whose short transaction
  always rolls back, including tentative global proof-key probes, without changing reservations
  or physical stock;
- `consume_shipped_reservation`, `restore_unshipped_stock`, `reconsume_restored_stock`, each
  accepting `(&DatabaseTransaction, &PortContext, &ShipmentStockBatch)`, never committing the
  caller's transaction and requiring rollback on any error.

Canonical write admission requires a positive deadline and immutable idempotency identity,
with a 30-second ceiling. Elapsed awaited lock/read time is rechecked before proof/stock writes,
between batch writes and before returning; expiry requires rollback, including preparation probes.
These are phase gates, not database statement timeouts. Tenant/Variant proof precedes tenant-less item reads. Sorted line
serialization and item/allocation/location/level checks preserve the original allocation,
location and all five non-null terminal markers (including false/zero values). No global oldest
fallback, repricing/publication refresh, SKU-less downgrade or historical backfill exists.

Private `inventory_shipment_stock_proofs` stores immutable tenant/Order/F/item/line/nullable
Variant/planned/original-allocation facts, cumulative shipped and outstanding restored units.
Only native new effects create history, atomically with checked level/reservation movement and
F's owner transaction. Global F-item collisions refuse without adopting foreign proofs;
quantity-zero retained reservations are audit, not evidence of a prior shipment by themselves.
Restore/reconsume require exact native history; overflow, archived original location, terminal
allocation, missing/ambiguous tracking or insufficient unreserved restored units refuse without
relocation/reactivation. Explicit SKU-less requires no retained allocation/returned-stock fact.

Retained return-restock facts block restore/reconsume on that line, and outstanding restored
shipment units block a new return restock. Both use the same I item serialization; exact return
receipt replay remains read-only and precedes the new dependency. This conservative exclusion
does not establish unpaid-return eligibility or incremental inspection top-ups.

The additive `m20261101_000025_create_shipment_stock_proofs` is registered but unexecuted.
Rollout pairs I025 with F's forward-only provider-admission correction F121 after F120;
drain old stock/provider writers before migration installation and matching binary enablement.
This does not add an Inventory dependency on Fulfillment or rewrite either owner's history.
Nonempty down refuses; no applied migration, global return-item PK or existing return receipt
is rewritten. PostgreSQL advisory/SQLite write-fence and rollback semantics, compilation,
regressions, carriers, mounted/remote behavior and complete event/outbox/Index convergence
remain unverified. See the [owning amendment](../../../DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md#shipment-identity-and-native-movement-provenance-amendment-2026-11-01)
and [source handoff](../rustok-commerce/docs/implementation-plan.md#shipment-stock-provenance-source-evidence).

## Native stock outbox (2026-11-01; source only)

Private `stock_events` constructs the existing registered `InventoryUpdated` root event and
calls `TransactionalEventBus::publish_root_in_tx` on the physical mutation transaction.
Shipment, cancellation restoration and reopening reconsumption use F's caller transaction;
`restock_returned_items` uses its existing I transaction. Required publication failure rolls
back local level/reservation/proof/receipt/counter writes; it does not roll back a carrier,
Order, financial effect or a separate Commerce checkpoint. C and F do not publish substitute
Inventory events. There is no new public capability, event schema, migration or receipt.

Each physically changed `(inventory_item_id, location_id)` emits one invalidation. A shipment
which decreases stocked and reserved equally still emits, even with equal available counters.
The v1 old/new quantities retain **global Variant availability**: descriptive source snapshots
around the whole batch, not location movement deltas, exclusive causal history, a stock lease
or a reservation guarantee. Several changed locations for the same Variant share snapshots.
Consumers refetch source state; they must not sum these payloads as movement accounting.

The plan retains Product IDs from already tenant-proved saved Variant rows, without fresh
Catalog eligibility/price reads. Aggregate snapshot result rows are bounded to at most 1024
proved items. Missing/foreign contributing location ownership or global availability outside
`0..=i32::MAX` refuses without clamping, relocation or silently dropping an event. This v1
representability restriction does not redefine backorder policy. Unrelated archived locations
are not silently excluded from the established global quantity semantics.

Preparation checks representability but never writes the outbox. Exact return receipt replay,
explicit SKU-less and no physical counter change have no new event-storage dependency. Only
actually inserted fresh return receipts contribute to new movement events. A human actor is
the current `PortContext` user's non-nil UUID; service/system actors use `None`. Stock events
contain no provider result, reason, role or claims. Unconditional root/typed outbox publication
requires one inserted row; zero-row suppression is not durable success. Exact write-once
conflict admission remains the separate canonical Outbox contract.

Typed failures: `inventory.stock_event_scope_unverified` and
`inventory.stock_event_quantity_unrepresentable` are nonretryable `InvariantViolation`;
`inventory.stock_event_actor_invalid` is validation; unsupported backend and required outbox
storage failures use `inventory.stock_event_backend_unsupported` /
`inventory.stock_event_persistence_unavailable`. Retain the original mutation identity and
reconcile owner state; never acknowledge event-less stock or rotate an immutable key.

Search already handles `InventoryUpdated` by Product upsert and reads live stock in its
projector; no consumer change or delta replay was added. This source trace does not certify
relay delivery, consumer durability/restart, generic Index or reservation-write convergence.
The [source evidence](contracts/evidence/native-stock-outbox-source.json) records 16 Rust and
10 Node definitions, all unrun. Compilation, SQL/locking/rollback and runtime remain unverified;
[owner plan](docs/implementation-plan.md#native-stock-outbox-continuation-2026-11-01-source-only).

## Anonymous quantity reservations (2026-11-01; source only)

`InventoryService::reserve` and `release_reservation_quantity` retain the native operator
quantity DTOs, but release now addresses only the positively proved anonymous compatibility
pool. The `InventoryReservationPort` adapter is the same implementation, not another ledger.

- New checkout consumers use `InventoryReservationIdentityPort`. Quantity commands refuse
  nonempty Cart/Order/line selectors with nonretryable
  `inventory.reservation_identity_required`, including selector-bearing zero requests. UUID,
  quantity, tenant and ordinary port admission validation retain their prior ordering.
- Eligible saved rows have native source `legacy_inventory_service`, exact saved Variant,
  no external/line binding, no checkout/adoption identity labels, and no terminal history.
  Missing/compacted/unverified metadata and positive counters alone never grant release authority.
  All five nonnull disposition/supersession/release/disposal markers refuse, including false/zero.
  Metadata, descriptions, IDs, original locations and creation timestamps are never repaired.
- An explicit anonymous pool FIFO uses `(created_at, id)`; it is not a missing identity fallback.
  The 4096+1 probe materializes only active unkeyed rows. New anonymous reserves stop before the
  pool exceeds 4096, and refuse contradictory native history. Existing oversized pools require
  owner-governed reconciliation, not a guessed external ID or automatic compaction.
- Every reduction is grouped at its saved `(item, location)`. Tenant-proved saved locations lock
  before exact levels; missing/foreign/duplicate/insufficient levels refuse, without warehouse
  fallback, relocation or current channel/publication reselection. All active claims at touched
  locations are aggregated in SQL, including arbitrary numbers of keyed rows outside the candidate
  pool. Their exact total must fit a reserved counter and remain covered before/after release.
  MySQL uses current locking claim reads rather than a stale repeatable-read snapshot; its
  signed aggregate cast is admitted only for groups within i32 range. PostgreSQL, SQLite and
  MySQL execution is not certified by these source checks.
- The complete plan precedes writes. Row and level updates condition on saved identity/quantity
  and must each affect exactly one row; ignored/drifted updates and unrepresentable final DTOs
  roll back the complete existing transaction. Only reserved counters change, never stocked or
  incoming quantity. A fully released anonymous row becomes quantity zero/soft-deleted, without
  rewriting metadata or turning it into a durable identity receipt.
- Existing archived-level constraints remain authoritative. A planned nonzero result at an
  archived original location refuses; clearing a historically nonzero level entirely to zero
  is compatible with I021. No automatic reactivation/relocation or archived-history repair occurs.
- Native compatibility and identity/adoption/shipment writers use the same item row lock.
  PostgreSQL row/location/level locks and SQLite conflicting-writer behavior still require runtime
  proof. Zero without identity selectors is read-only and creates no stock state or publication
  dependency. Availability uses a 4096+1 level read and checked i64 intermediates with signed i32
  response admission: legitimate negative `continue` availability survives; no saturation/clamp.
- A required transport key/retry hint is not a compatibility command receipt. Repeating a successful
  quantity command may perform another mutation; do not use it for automatic checkout recovery.
  Native permission/context admission and global descriptive results are unchanged. These results
  remain neither channel eligibility nor allocation/provenance guarantees for another consumer.

The operator UI English/Russian labels explicitly name anonymous reservations. No alternate
Next.js inventory quantity mutation or GraphQL publisher is added. That anonymous-pool prerequisite introduced no new public capability, event schema/digest,
receipt, migration, history rewrite or Product refresh impersonation. The subsequent reservation
fact and typed-delivery source continuation is documented below; its runtime delivery remains
unverified. The v1 physical-stock helper cannot be reused to prohibit valid backorders.

Evidence: [source record](contracts/evidence/quantity-release-source.json), 27 Rust/10 Node
regression definitions **uncompiled/unrun**, and [remaining gates](docs/implementation-plan.md#anonymous-quantity-reservation-release-2026-11-01-source-only).
The separate prior physical-stock outbox record remains unchanged; step 8 is not complete.

## Reservation invalidation facts (2026-11-01; source only)

`InventoryService::reserve(&PortContext, variant_id, quantity)` and
`release_reservation_quantity(&PortContext, variant_id, quantity)` now require the **current**
command context instead of a bare tenant UUID. Existing quantity ports forward it unchanged;
authenticated native admin supplies current tenant/user, correlation and available request
locale/channel, refusing a conflicting request tenant. Missing optional presentation context
is `und`/no channel, not inferred identity. The native two-second deadline and diagnostic key
are admission metadata, not an idempotency receipt or verified wall-clock cancellation.

- Fresh positive identity and anonymous reserve/release prove the tenant's original Product /
  stable Variant UUID, perform admitted exact counter/receipt effects, check the signed result,
  and persist one canonical sealed `InventoryReservationEvent::Changed` before the same commit.
  Its event type is `inventory.reservation.changed`, schema version 1; the payload contains only
  non-nil `product_id` and `variant_id`. Current human actor UUID remains envelope metadata;
  malformed/nil human identity refuses, while Service/System uses no fabricated human actor.
- The canonical `TransactionalEventBus::publish_contract_direct_in_tx` helper writes on the
  existing owner transaction. Publication/insert-count failure aborts local effects, including
  receipt changes. There is no host/Commerce publisher, separate queue, raw `sys_events` pipeline,
  Product refresh/revision impersonation or reservation fallback to root `InventoryUpdated`.
- Exact identity replay, released replay, zero quantity and binding-only adoption emit nothing
  and acquire no event-publication dependency. Anonymous quantity commands remain non-idempotent
  even when their transport requires a key. Stored actor-like metadata never becomes the actor.
- Both identity and quantity availability use the same bounded checked signed evaluator
  (4096+1 probe, wide intermediates, checked final i32). Negative `continue` balances are valid;
  an unrepresentable result or inconsistent/oversized level layout refuses atomically rather
  than clamping, saturating, omitting a required fact or banning backorders.
- The existing Core module dispatcher now offers acknowledged sealed opt-in. Memory awaits all
  matching explicit handlers; absent/closed listeners, capacity refusal and handler failure do
  not acknowledge the local fact. Under `outbox_iggy`, the configured primary accepts the same
  envelope first, then required local delivery must succeed. A local failure can republish that
  **same identity** remotely on Outbox retry: at-least-once, not distributed exactly-once.
- Artifact subscriptions validate and durably enqueue the sealed envelope with its original
  source identity before forwarding. Search explicitly consumes this Inventory family and
  refetches current persisted state, not a payload quantity/delta or Product revision. Search's
  PostgreSQL product upsert/rebuild/delete paths share a tenant product-scope advisory lock and
  READ COMMITTED transactions before fresh reads. This remains an existing direct-table projection,
  not an Inventory owner-read port, generic Index source or channel/backorder sellability proof.

No migration, anonymous receipt, public capability or root event wire change is introduced.
The sealed schema **is** new; its generated digest artifact is deliberately unchanged until
`cargo run --locked -p rustok-events --example event_contract_digests -- --write` can run.
Existing pending Checkout-family admission is not superseded. Cargo/rustc are absent.

Evidence: [reservation source packet](contracts/evidence/reservation-events-source.json) and
[owner plan](docs/implementation-plan.md#reservation-fact-delivery-continuation-2026-11-01-source-only).
The **34 Rust/10 Node definitions are uncompiled/unrun**. Source guards, syntax and format checks
are not SQL, actual Iggy receive/ACK, relay/restart/lag, artifact-worker or Search concurrency
proof. Generic Index's Inventory-owned source/revision/consumer and Product step 8 remain open.

## Catalog deletion safety (2026-11-01; source only)

`BootstrapService::delete_records_for_variants_in_tx` is the existing native Product lifecycle
collaboration, not a new public stock-disposal API. Its admitted inputs are the caller's
`DatabaseTransaction`, current nonnil tenant and at most 256 distinct nonnil Variant UUIDs.
It proves all tenant identities and shares native item locks before reading Inventory history.
Every requested Variant needs a positive Inventory item identity; a missing item is not empty
stock proof. An empty request alone is read-only.

Any saved reservation, shipment-stock proof (Variant or item reference) or returned-stock
receipt blocks deletion, even when counters are zero, a row is soft-deleted, or saved labels
are contradictory. Opaque existence checks follow tenant-proved parent identities and never
return foreign history payloads. No history is erased or reinterpreted as SKU-less state.
A complete plan admits at most 4096 levels for the entire batch; every original location must
belong to the current tenant, and every stocked/reserved/incoming counter must individually
be zero. Archived empty locations remain archived. Locations lock before current levels; exact
keys/bindings and exact-one conditional deletes are mandatory. PostgreSQL READ COMMITTED is
required; non-current snapshots refuse. Caller errors require rollback of the whole enclosing
transaction. The helper never commits independently.

`CatalogDeletionError` distinguishes storage failure from finite `CatalogDeletionBlocker`
refusals. Product maps blockers without database text into its normal native/GraphQL error
boundaries; stock/history requires Inventory reconciliation or Product archival, not automatic
cleanup/retry. Additive history-reference indexes change no ledger identity or historical row.
No stock/reservation event is emitted for removing proven empty bookkeeping; Product retains
its existing lifecycle event transaction. This is not Inventory → generic Index convergence,
and adds no revision/tombstone or authorization/replay receipt. Regression, migration, SQL,
locking and compiler evidence remain unrun; see the [owner plan](docs/implementation-plan.md#catalog-deletion-safety-2026-11-01-source-only).

## Canonical quantity writes (2026-11-01; source continuation)

Native set/adjust and Product's existing transaction-sharing quantity update must use one
Inventory-owned bounded plan. Quantity means global available units, not an arbitrary level's
stocked count. The concrete caller transaction and current nonnil tenant/actor/Variant precede
positive Variant/product-parent/item identity. Parent tenant proof is structural, not publication
eligibility; canonical tenant ownership is immutable and its read must not invert Product locks.
Missing or ambiguous items/levels refuse; no legacy scalar,
implicit identity creation, warehouse creation or first-level fallback supplies proof.

At most 4096 complete levels (plus one overflow sentinel) are discovered. Original tenant-owned
locations, including the frozen current owner-default candidate, lock in UUID order before a
current level reread. Exact bindings must agree. Current owner-default selection is the existing
code=`default`, otherwise oldest active tenant location policy; ties are UUID ordered. The
selected active target must already have exactly one level; never reselect after locking,
reopen a location or alter another warehouse. No-op plans require proven current state but no
active default, timestamps, outbox dependency, new level or stock event.

Checked signed i64 planning and exact i32 response/storage admission replace saturation,
unary i32 negation, unchecked sums and SQL addition. Stocked/reserved/incoming counters must
be individually nonnegative; only target stocked changes, preserving every reservation,
receipt, incoming counter, threshold, metadata and other location. Deny policy forbids negative
availability; continue policy never permits negative physical stocked units. Existing v1
`InventoryUpdated` cannot encode negative global quantities: a real change with unrepresentable
old/new snapshots refuses explicitly before effects, without changing the event schema or the
separate UUID-only reservation fact. Signed availability remains valid for a proven no-op.

PostgreSQL and MySQL quantity writes require verified READ COMMITTED; SQLite uses its
transaction snapshot/write-conflict behavior. PostgreSQL reads the active transaction setting.
MySQL must read the active explicit transaction for `CONNECTION_ID()` from
`performance_schema.events_transactions_current` joined to `performance_schema.threads`.
The session default is not proof: a one-transaction isolation override or a later session
setting change can disagree with the already-open transaction. Current-transaction
instrumentation and SELECT access are therefore required on MySQL; missing/inactive metadata
refuses, and query/decoding failures propagate without a session-default fallback or silently
changing the caller's transaction. Actual metadata access, current rereads and backend
concurrency remain maintainer-run gates, not inferred certification.

The Product response/public-channel quantity projections share checked wide aggregation instead
of order-dependent per-level saturation. Channel/location visibility remains a projection policy,
not a stock absence proof or a new Index source. Their existing read materialization contracts
are not promoted to authoritative bounded Index reads by this writer prerequisite.

Exact conditional target update, canonical root outbox publication and any existing adjust-only
low-stock notification share the caller transaction; any ignored effect or publication failure
requires full rollback. Product forwards current tenant/actor and propagates typed owner failure
through its finite public policy and existing receipt finalization. Native stock commands remain
non-idempotent; retry advice is not outcome or permission to replay an adjustment. No helper
commit, stock disposal, historical rewrite, baseline, source revision or Index cutover is added.
Compilation, regression/SQL/locking/client/runtime evidence remains unrun; see the owner plan.

## Inventory source history (2026-11-01; source continuation)

Inventory owns a new, explicit **current-state-only** baseline and positive per-tenant/SKU
source revisions. No migration or normal write initializes an installation, assigns historical
versions, infers historical deletes, or borrows Product revisions. Unknown history remains
unknown. Baseline absence means uninitialized, never an empty authoritative source.

A permission-checked native owner command starts a durable baseline identity. Bounded pages
(up to 128 Variants and 4096 complete levels total, plus sentinels) copy positive tenant/parent/
item/original-location identity and checked counters. Page progress and immutable source-change
rows commit together; only the final complete page activates owner tracking. Caller-selected
smaller pages handle dense SKUs; overflow refuses the entire page without partial progress.
The original baseline identity is retained; every identical page request replays its saved
original progress, stale new or changed page requests refuse, and no reset/key rotation is inferred.

While building, actual stock/creation/deletion writes refuse and roll back. PostgreSQL uses
nonblocking shared writer/exclusive baseline transaction advisory gates, including writers
which already hold Product/Variant/item locks: neither path waits for the other's advisory gate,
so baseline does not invert existing owner lock order. PostgreSQL requires READ COMMITTED.
SQLite uses transaction write/snapshot exclusion; initialization is unsupported on MySQL.
Before explicit initialization, existing domain writes remain untracked and are not certified
Index inputs. Once active, missing/mismatched revision state refuses rather than silently
recreating a baseline. New SKU creation initializes version 1 only for an actual new owner
identity. Real quantity/reservation/physical effects advance once per SKU per owner plan even
when net availability is equal. Replay, zero, preparation and adoption-only metadata do not.

The owner record is a transactionally maintained snapshot of global stocked/reserved/incoming
counters and immutable SKU/item/parent identity; it deliberately excludes Product publication,
SKU copy, policy and location/channel visibility. Stable level-binding proof prevents implicit
repair from becoming tracked identity. Checked i64 aggregates preserve signed availability.
An exact history-preserving delete retains a higher-version tombstone without foreign-key
cascade. All records and immutable changes retain their baseline identity and original UUIDs.
A missing targeted load is unverified, not zero, authoritative absence or a tombstone.

The journal is **not** a second publisher, delivery receipt, global commit-order cursor or
Index readiness claim. Its stable change UUID and committed source version are intended for
the selected canonical delivery continuation. Per-SKU revision paging is ordered by that
SKU's serialized commits; no unsafe global sequence watermark is exposed. No source code writes
Index tables. At this owner-history handoff canonical versioned publication/relay was unwired;
the 2026-11-02 continuation below adds that source slice. Selected Distribution conversion,
source/event/worker registration, persist-before-ACK and native/HTTP/GraphQL/Next/Leptos
operator parity remain deliberately unwired and must precede any Index cutover. Existing stock
and UUID-only reservation notification contracts remain unchanged. Source compilation, tests,
SQL/migration/lock/runtime evidence are unrun; no readiness status is promoted.

Baseline start/page commands are creator-actor bound and permission-checked under the current
context. They use the canonical core timeout helper for acquisition/work/commit and await error
rollback. Commit timeout remains unknown: recover the original baseline/page identity rather
than reconstructing an actor, rotating keys or assuming no effect. Existing error envelopes
conservatively keep source provenance failures nonterminal; new actionable client recovery is
not implemented by this owner slice. Apply empty-table I027 before matching binaries.
Native status/load/scan/per-SKU changes also enforce the current read budget through that core
helper. Scan is live bounded keyset enumeration, not an installation-wide repeatable snapshot,
absence certificate or commit-order watermark. Empty history/scan does not replace targeted
`load` and cannot authorize a deletion; concurrent creation behind a scan cursor requires the
still-pending versioned canonical convergence path. Source versions remain native u64 bounded
by the owner i64 store; transport/client encoding and conformance are not claimed by this API.
Deletion captures every original source parent/item/level/location binding and retained head
**before the first physical DELETE**. Its private prepared proof is consumed only after all
exact physical effects, with the same caller transaction and version/head CAS. Even an empty
replacement level is unverified; it is not silently retired as the original identity.

## Inventory source canonical delivery (2026-11-02; source continuation)

Two additive sealed Inventory facts use the existing canonical `sys_events`/OutboxRelay path:
`inventory.source.changed` carries only the exact baseline UUID, Variant UUID and positive
committed Inventory source version; `inventory.source.baseline_activated` carries the explicit
baseline UUID. Neither widens `InventoryUpdated` v1 or the UUID-only reservation fact, borrows
Product revisions, embeds generic Index DTOs, or claims an owner receipt/availability lease.

Every real tracked creation/quantity/reservation/physical/deletion effect writes source state,
its retained change row, and the changed fact on the **same caller DatabaseTransaction**.
The retained change UUID is the exact write-once canonical envelope UUID. The current caller
supplies the actor; Bootstrap and both Product deletion paths must forward it explicitly.
No source/journal reader or relay reconstructs authority from baseline/receipt metadata.
Replay, no-op, preparation and adoption do not create another version or source fact.

Building pages retain staged version-1 records without publishing per-SKU ready events. The
final page writes one baseline-activation fact, using the globally unique baseline UUID as its
canonical envelope identity, in the checkpoint/receipt/activation transaction. This includes an
explicit empty baseline. Original page replay publishes nothing. The selected Distribution
consumer must durably arrange the initial bounded source replay before acknowledging activation;
this source delivery slice does not pretend that consumer/rebuild wiring already exists.

The existing acknowledged module-listener path is mandatory for Inventory reservation and
source facts in memory and Iggy fan-out profiles. A missing/inactive/mismatched handler must
refuse before broker-only success; handler failure retains the unchanged outbox identity.
Other explicitly transport-consumed sealed families keep their existing remote cursor policy.
Local completion and a broker consumer cursor are separate acknowledgements. A future selected
Index consumer must perform authoritative load, minimum-version/stale fencing and durable
mutation persistence before its handler/transport acknowledgement.

No old migration is changed and no delivery history is inferred. A baseline already activated
under an earlier binary may have no activation event or earlier changed facts: status/journal
observation never manufactures them. Explicit operator reconciliation/cutover, selected
Distribution generic source/event/rebuild/worker composition, digest admission and full native/
HTTP/GraphQL/Next/Leptos recovery remain open. Do not start a live baseline or promote Index/
step-8 readiness from source checks. No tests, SQL, migrations or runtime are executed here.

## Selected Inventory Index consumption (2026-11-02; source continuation)

The selected Distribution bridge publishes the nonlocalized `rustok-inventory/stock` schema, keyed
by the original Variant UUID. Inventory remains the owner of baseline identity, positive
per-SKU versions, original parent/item bindings, signed aggregate availability and retained
tombstones. The bridge uses only `InventorySourceHistory` native reads, never private tables,
Product revisions, journal-generated publication or copied event quantities. This projection
is global stock information, not channel visibility, public availability or a reservation lease.

Every read requires a current Auth-owned worker service context. Deployment binds an exact
tenant and source consumer to a canonical secret reference for a genuine OAuth service access
token. Authentication revalidates the signed token, current active app, tenant, grant, allowed
scopes, current granted permissions and module availability. Current effective permissions
are forwarded, not invented. Missing credentials, revoked permissions, wrong principal kind
or unavailable policy refuse the read. Event actor, baseline creator, journal and receipts
are never worker authority. The source refuses missing targeted records and retains tombstones.

The source factory contributes the real sealed listener through the existing Index factory
catalog and module event-listener lifecycle. Changed deliveries verify the exact owner change
identity, load at least the published source version, preserve the loaded owner's immutable
change identity and wait for generic Index inbox/entity/link persistence before handler ACK.
Transport delivery UUID and the current loaded mutation UUID are distinct when the owner has
already advanced; rebinding a changing payload to an old event UUID is forbidden.

Activation verifies the current exact active baseline, registers the tenant schema through the
Index owner and durably enqueues bounded initial replay using the original baseline/event UUID
as the job UUID before ACK. Duplicate requests retain that job and checkpoint. The generic
module-work scheduler discovers pending or expired replay ownership and resumes bounded pages
with attempt-fenced checkpoints and current authority on each source invocation. Terminal or
identity conflicts refuse; they do not silently create a replacement job or baseline.

Scan remains live keyset enumeration of current records plus retained tombstones, not a
repeatable snapshot, absence certificate or global commit watermark. The bridge publishes no
absence provider. Writes committed behind the scan cursor converge through canonical changed
facts. Applied historical migrations and event digest remain immutable. Old active baselines
without canonical activation/change history still need explicit operator reconciliation.

This is source implementation, not deployment evidence. Real credentials, persisted tenant
schema readiness, worker enablement, compilation, regression/SQL/concurrency/restart evidence,
digest admission, full native/HTTP/GraphQL/Next/Leptos operator recovery and Index/step-8 cutover
are separate gates; none is promoted by registration or lexical checks.

The current selected consumer source is documented in the
[Distribution contract](../rustok-distribution/docs/inventory-index.md) and
[source-only evidence](contracts/evidence/source-index-consumer-source.json).
Actual code now registers the source/listener and generic bounded worker; runtime verification
is still absent. Initial enqueue is not readiness. Crash/yield resumes retain original identity;
page failures currently park and require pending original-job recovery, never replacement/reset.
No historical delivery is inferred, and live cutover/operator-client recovery remains gated.
