# Shipment stock movement is a fulfillment-owned port; returned goods are restocked only by operator decision

- Date: 2026-10-21
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-fulfillment (shipment truth, port contract); rustok-inventory (stock and ledger); rustok-commerce (composition, return completion)
- Extends: [Order shipped/delivered status follows fulfillment truth; stock consumption stays trigger-owned for now](./2026-10-21-order-shipment-status-projection.md)
- Supersedes: None
- Superseded by: None

## Context

ECOM-INV-01. Stock consumption on shipment is the `fulfillment_inventory_consume_on_ship` database
trigger (`m20260713_000007`). On growth of `fulfillment_items.shipped_quantity` it reads
`order_line_items` and reads and writes `reservation_items` and `inventory_levels`: a cross-module
business rule inside a trigger, and a second writer of tables inventory owns (AGENTS §5/§7). It also
leaves two behavioral gaps: cancelling a fulfillment that already shipped never returns stock, and
returned goods never return to stock at all. The earlier ADR (shipment projection) deferred the move
because it could not be validated; this change makes the move with a fail-closed default so a missed
wiring fails loudly instead of silently skipping stock.

## Decision

1. **Port contract in the consumer.** `rustok-fulfillment` declares `ShipmentStockPort`
   (`consume_shipped`, `restore_unshipped`, `reconsume_restored`), each taking the caller's
   `&DatabaseTransaction` and `ShipmentStockRequest { tenant_id, order_line_item_id, quantity }`.
   The module stays a leaf: it depends on neither order nor inventory.
2. **Calls inside the fulfillment transaction.** `ship` calls `consume_shipped` for every item whose
   `shipped_quantity` grows. `cancel` of a `shipped` fulfillment calls `restore_unshipped` with
   `shipped − delivered` per item (delivered units are with the customer). `reopen` of a cancelled
   fulfillment calls `reconsume_restored` with the same quantity. `reopen`/`reship` of a delivered
   fulfillment change only `delivered_quantity` and move no stock. A port error rolls the whole
   operation back.
3. **Inventory owns the arithmetic.** `rustok_inventory::shipment_stock` reproduces the trigger's
   ledger rules on the passed transaction: active reservation of the line, quantity ≤ remaining
   reservation, level `reserved`/`stocked` ≥ quantity, reservation soft-deleted at zero. Restore adds
   to `stocked` at the location of the line's (possibly consumed) reservation; reconsume requires the
   units to still be available.
4. **Composition in commerce.** `rustok_commerce::InventoryShipmentStockPort` skips lines without a
   variant (they never have a reservation) and delegates to inventory. Commerce builds every
   ship-capable fulfillment service through `fulfillment_service(db)` /
   `fulfillment_admin_command_runtime(db, registry)`.
5. **Fail closed.** `FulfillmentService::new` installs `UnconfiguredShipmentStockPort`, which fails
   every movement. `NoopShipmentStockPort` is the explicit opt-out for deployments without stock
   tracking and for fixtures. Creating and reading fulfillments does not need a port.
6. **Trigger removed.** `m20261021_000001_drop_fulfillment_shipping_stock_trigger` drops the trigger
   and function (PostgreSQL, SQLite); `down()` re-runs `m20260713_000007`. The order-delivery trigger
   (`m20260713_000006`) is unchanged.
7. **Return restock is opt-in per completion.** Nothing returns to stock by default. The operator
   passes `metadata.restock` to complete-return: `"all"` or `[{return_item_id, quantity}]`.
   `ReturnCompletionOrchestrationService` validates the shape at admission, then after the owner
   completes the return (and also when a retry finds it already completed) calls
   `rustok_inventory::return_restock::restock_returned_items`. Inventory owns the ledger
   `inventory_return_restocks` (primary key `return_item_id`, written in the same transaction as the
   stock increase), so retries, replays and concurrent completions never add stock twice. Stock goes
   to the item and location of the order line's reservation; lines without a reservation are skipped.

## Sources of truth and ownership

Shipment counters: `rustok-fulfillment`. Reservations, levels and the restock ledger:
`rustok-inventory`. Which lines are stock-tracked and the decision of which return items to restock:
`rustok-commerce` (composition) and the operator (decision). The port contract is owned by its
consumer, fulfillment.

## Invariants

### Allowed states

Per tracked order line: `shipped_quantity` consumed from the reservation exactly once; a cancelled
shipped fulfillment has returned its undelivered units; a return item is restocked at most once.

### Forbidden states

Shipment counters advancing without the matching stock movement; stock added twice for one return
item; a ship through a fulfillment service without a stock port.

## Non-goals

Automatic restock policies (by reason or product), partial inspection workflows, re-reserving units
for another order, restock on return paths that bypass `ReturnCompletionOrchestrationService`,
stock movement on provider reconciliation outside the fulfillment service.

## Data, transaction, and concurrency boundary

All movements run in the caller's transaction; PostgreSQL locks the reservation and level rows
(`FOR UPDATE`), SQLite relies on its single-writer transactions. The restock re-checks the ledger
after taking the level lock.

## Context dimensions

Tenant is carried in the request and recorded in the ledger. No channel or locale dependence.

## Events and projections

None added. Stock levels keep their existing read projections.

## Failure semantics

Port rejection or storage error: the fulfillment operation fails and rolls back. Restock rejection:
the completion journal records `reconciliation_required` (ledger inconsistent); restock storage
error: retryable. The return itself stays completed; a retry re-enters and applies the ledger
idempotently.

## Migration and cutover

Run migrations before routing traffic to the new binary. The new binary against a database that
still carries the trigger would consume stock twice (trigger and port); the old binary against a
migrated database would ship without consuming stock. `rustok-inventory` adds
`m20261021_000024_create_inventory_return_restocks`. Hosts that build their own
`FulfillmentAdminCommandRuntime` or `FulfillmentService` and ship through it must call
`with_shipment_stock_port`.

## Alternatives considered

Fulfillment depending on inventory (breaks the leaf module and digital flows); no-op default
(silent loss of stock movement); process-global port registry (hidden global state); event-driven
consumption (needs a ledger and eventual consistency for a synchronous invariant); a new journal
stage for restock (schema and trigger changes for no gain over the inventory ledger).

## Verification

Integration tests in `rustok-commerce/tests/order_inventory_reservation_test.rs` (ship consumes,
cancel restores, reopen reconsumes, unwired service fails closed and rolls back, restock is
idempotent and skips untracked lines) and unit tests for the restock decision in
`return_restock.rs`. **Not compiled or run when written.**

## Consequences

Stock movement is typed Rust, testable on SQLite and PostgreSQL, and one owner per concern.
Deployments must wire the port. Operators decide restocking explicitly.

Known gap: a shipped fulfillment cancelled *before* this change never returned its stock. Reopening
it afterwards calls `reconsume_restored` and either fails (units no longer available) or deducts stock
that was never restored. Such legacy rows need an operator review before reopening.

## Return-restock identity amendment (2026-10-31)

This amendment tightens decision 7, its absence-as-untracked rule, and the restock portion of the
transaction/context sections above. It does not change the native caller-transaction
`ShipmentStockPort`, certify ship/cancel/reopen, or infer that historical cancellations restored
stock. The original decision is retained above as history.

### Problem and owner contract

An Order-line UUID alone is not tenant or SKU proof. Choosing the oldest reservation can restore
an unrelated allocation; accepting a return-item primary key without comparing its saved facts can
misreport a changed command as replay. A tracked line without an allocation must never become a
successful no-stock operation.

Order owns the immutable Order-line/Variant/quantity binding. A mandatory bounded batch read on the
existing `OrderReadPort` supplies only those saved facts, not fresh Catalog eligibility or Pricing.
Commerce uses its already-selected Order read capability, validates the whole response and the
operator's selection before resolution side effects, and freezes that plan through owner completion.
The completed return must still match the admitted tenant, Order, return and item facts. No embedded
fallback, provider switch, SKU substitution or price refresh is permitted.

Inventory's existing native `return_restock::restock_returned_items` entry point accepts canonical
`PortContext` and explicit saved Variant bindings. A nonempty batch is bounded to 1024 distinct
return items of one return; quantities and identities are validated before database work. Only an
explicit SKU-less Order binding may yield `untracked`. Missing tracked inventory, absent or ambiguous
reservations/levels, foreign locations and terminal release/disposal/supersession facts refuse.
Saved hidden locations remain valid without current publication/channel eligibility checks.
An archived location is valid for exact replay only: the existing Inventory inactive-location
storage guard forbids nonzero quantities there. Fresh movement requires the saved location to
remain unarchived under a location-row lock. Restock never selects a replacement location.

### Persistence, concurrency and replay

The existing `inventory_return_restocks` ledger remains the sole restock receipt; no new ledger,
receipt, schema or historical rewrite is introduced. Inventory proves every Variant's tenant before
loading its tenant-less item, locks items in stable Variant order, and scopes reservation reads by
that proved item and Order-line. Fresh movement requires exactly one historical allocation, not the
first row. Inventory locks distinct receiving locations in sorted UUID order after item locks,
proves their tenants and rejects archived locations before locking levels or admitting receipts.
It then locks all affected levels in stable item/location order. Read-only replay location proof
cannot substitute for this fresh-write preflight. Ledger admission and checked stock additions
commit together for the entire batch; reserved/incoming quantities, reservations and allocation metadata do not change.

Exact replay compares the ledger's tenant, return, return item, Order-line, item and quantity against
the request and the tenant-proved Variant/item, and proves its saved location's tenant. It neither
reselects a reservation nor rewrites a level, receipt or timestamp. Since historical receipts contain
item/location rather than Variant, missing or contradictory item/location ownership refuses instead
of inventing historical identity. The primary-key fence also protects races across conflicting SKU
claims without reading another tenant's receipt. A changed quantity is a conflict, not an incremental
restock; partial-inspection/top-up workflows remain out of scope.

### Failure and verification limits

Commerce enforces a positive bounded call budget and validates complete Inventory outcome
correlation before its separate checkpoint. Timeout/storage failure does not prove no effect; only
same-identity recovery is allowed, with the original operator decision and mutation identities.
Identity/history inconsistencies require reconciliation, not automatic release, relocation, key
rotation or compensation. Stock and receipt atomicity is native Inventory atomicity, not distributed
atomicity with Order completion or the Commerce journal.

Regression definitions and read-only guards must cover tenant/Variant proof, allocation ambiguity,
terminal history, exact replay, batch rollback, outcome correlation and selected-provider use.
Runtime/SQL/locking, compilation, remote convergence, complete stock event/projection invalidation,
and the separate shipment/cancellation/reopen tail remain unverified or unfinished; source evidence
must not promote module readiness.

### Archived-location storage correction (2026-10-31)

Source review found that the initial identity continuation allowed fresh writes to archived
locations even though `m20260713_000021_bound_reservation_metadata_and_inactive_locations`
rejects nonzero stock there on PostgreSQL and SQLite. That would surface an invariant refusal
as a retryable storage failure. The correction preserves that potentially applied migration:
`inventory.return_restock_location_archived` requires reconciliation using the original saved
location and mutation identities, without automatic reactivation, relocation or compensation.
It does not claim an operator reactivation workflow exists. Exact replay still verifies retained
location ownership without requiring an active location or changing any stock/timestamps.
The PostgreSQL receiving-location row lock serializes fresh stock addition against concurrent
archival. SQLite retains its native transaction and existing storage guard, not `FOR UPDATE`.
Actual PostgreSQL/SQLite timing and regression sources remain unexecuted.

### Physical-history scope limit

The operator's completed-return inspection remains authority for returned goods. This identity
amendment does not infer that a deleted/zero reservation proves historical shipment consumption,
or introduce shipment/cancel movement receipts. Historical shipped/restored unit provenance,
unpaid-order return eligibility, partial-inspection top-ups and overlapping cancellation/return
stock arithmetic need their own owner transition/policy work; they are not certified by these
identity checks. In particular, the existing shipment adapter/helper tenant and oldest-allocation
gaps and legacy cancelled reopening remain open, rather than being hidden as completed here.

## Shipment identity and native movement provenance amendment (2026-11-01)

This amendment supersedes the original line-only shipment request and oldest-reservation
selection in decisions 1–4. Fulfillment remains the owner of lifecycle/progress and its
consumer port; Inventory owns physical movements/provenance; Commerce composes the selected
Order capability. No peer dependency is added to Fulfillment.

### Preparation and transaction boundary

The consumer port prepares a price-free, bounded (1024 items) stock plan from a tenant-scoped
Fulfillment snapshot and the already-selected mandatory Order saved-binding batch. Commerce
correlates tenant, Order, requested line set, explicit nullable Variant and saved quantities;
there is no foreign-table Order lookup or native/remote fallback. Preparation and peer I/O
occur before the Fulfillment write transaction (and before fresh carrier execution). The
in-memory prepared plan is not authorization, a durable receipt, a distributed transaction
or proof of external outcome. Recovery re-reads retained immutable owner facts and preserves
existing journal/mutation identities; it does not reconstruct missing physical history.

A fresh header-only shipment with no typed items is unknown stock scope, not an explicit
SKU-less binding. It must still consult the selected stock capability before a carrier;
the native Commerce adapter refuses the incomplete scope. Only an explicitly installed
Noop may admit that no-stock deployment/test intent. Pending cancellation, delivery and
undoing delivery remain movement-free and must not acquire a stock dependency.

Under its parent lock, Fulfillment correlates the complete frozen identity/progress snapshot
and supplies one movement batch before changing counters. Inventory receives canonical context,
Order/Fulfillment/item/line identities, saved optional Variant, quantities and progress. All
item, provenance, allocation, location and level locks use deterministic batch ordering. Native
stock/provenance changes and Fulfillment counters/provider commit share the caller's transaction;
Inventory never commits it. Missing/ambiguous/terminal allocation, conflicting proof, overflow,
closed receiving location or unavailable restored units refuse without choosing another warehouse.

### Admission, replay identity and nullable binding serialization

A fresh carrier claim re-correlates the frozen Fulfillment snapshot while holding its parent
lock. Unresolved sibling provider operations (including legacy `provider_error`, whose outcome is not
proved) fence both new carrier claims and local progress changes. Only a freshly claimed,
known-not-invoked attempt may return to `pending` when its remaining budget expires before
invocation. Registry availability/capability preflight also precedes claim: an adapter that
cannot be invoked leaves Pending, rather than inventing an unknown external outcome. Recompute budget after claim; never turn an ambiguous carrier error into retry.
Local application correlates the retained provider request/result and original literal key;
missing historical request facts require reconciliation, not reconstruction from counters.

Fulfillment's shared unresolved-effect predicate fences every non-Pending/non-Committed state,
Pending with retained result/reference/completion/commit evidence, and incomplete Committed.
Claim and private uninvoked-release CAS require all four outcome markers absent; SQLite takes
a zero-row journal writer fence before transaction-local reads, while PostgreSQL locks the
parent. An explicit operator no-effect resolution may requeue Recon/legacy ERROR as Pending
only without reference/result/commit evidence, retaining its original identity and request.
ERROR never automatically transitions to Executing. Explicit success resolution validates the
retained reference and CASes the observed reference/null result/no commit evidence. The bounded
worklist uses the same predicate, UpdatedAt/Id ordering and excludes Executing; stale executions
still need explicit quarantine. F121 enforces the corrected single-row state/identity guards
for PostgreSQL/SQLite, leaves historical payloads/rows intact, and refuses rollback that would
reopen ambiguous execution. Raw SQLite fixtures install it last after any historical guards.

A lifecycle caller key is first resolved against the tenant/Fulfillment journal, before mutable
shipping-option provider selection. Retained provider identity wins; ambiguous historical keys
across providers refuse rather than selecting an oldest row. New journal admission is serialized
on the parent and claim also refuses a colliding retained key, including Pending/Committed rows.
Terminal lifecycle status or provider metadata is not an exact key/payload replay receipt.
Owner journal/recovery paths validate key bounds/blankness without trimming or aliasing a
retained literal key. Transport canonicalization of a new caller key does not authorize
reinterpretation of already saved identity/request facts.
Reship freezes Delivered under the same native snapshot/claim lock without acquiring stock peers.

Provider result/reference facts are monotonic: no recorded value may be replaced or cleared.
A late validated success after stale-execution quarantine is retained with reference/result CAS
while the operation stays reconciliation-required; it is not silently discarded or re-executed.
Contradictory late or duplicate results refuse without overwriting the earlier outcome. The new
unapplied F121 guard enforces that monotonicity, without changing historical migration payloads.
Lifecycle and create-label callers must honor a quarantined success checkpoint rather than
continuing an automatic local commit. ProviderSucceeded alone is not a successful local
create-label application receipt: reuse that retained result and complete native persistence
before acknowledging the create command. Explicit success resolution may promote an identical
retained result without replacing its result/reference or original completion timestamp.
Stale-execution quarantine likewise retains an already saved completion timestamp; it only
records the current time when the historical value is absent, without repairing other facts.
Committed lifecycle replay rereads the current native projection, not a pre-journal snapshot.

A pending cancellation reopened without shipped item progress or an actual `shipped_at` remains
Pending; the `ActiveValue<Option<_>>` wrapper is not shipment evidence. HTTP owner helpers reject
mismatched Tenant/Auth/Request contexts defensively (the canonical extractor already validates
them), and GraphQL rejects a contradictory supplied RequestContext while preserving its existing
context-absent embedded fallback.

Inventory deducts lock/read time from the admitted native budget before its first proof/stock
write, between batch writes and before returning. Failure rolls back on the caller transaction;
preparation rolls back its own probe transaction even when that budget expires.

Local recovery and native provider-result wrappers deduct scoped journal/receipt-read and
request-validation time from the current original positive budget before stock preparation;
no relative deadline refresh is permission to resume work. Committed replay remains O/I-free.
Owner commands also deduct initial projection/provider lookup and progress-receipt admission;
stock-bearing commits and receipt completion retain the same final pre-commit budget check.
These phase checks are not database statement timeouts.

Order cancellation composes the selected Fulfillment reader and an owner-defined pending-only
cancellation command. The owner freezes `pending` before preparation and rechecks it at locked
claim; a race to shipment must never silently turn into stock restoration. An adapter lacking
this capability returns explicit nonretryable unavailability, not ordinary/full cancellation.
Local provider-result recovery is also an owner command: Fulfillment loads its retained journal
key and applies/acknowledges its own transition. Commerce delegates to the selected command
port; it neither decodes the private request protocol nor falls back to native storage when a
selected owner cannot recover. These capabilities do not claim cross-owner saga atomicity.

Mounted `deliver`/`reopen` commands use the existing kernel command receipt, completed in the
same transaction as progress/native stock. It provides exact command replay, not shipment
allocation provenance. GraphQL lifecycle mutations require a bounded caller `idempotencyKey`,
so separate partial intents do not share one derived per-Fulfillment key. Saved legacy values
remain admissible verbatim; there is no automatic dual-key lookup or key migration.

Inventory preparation runs full native physical/history checks in a short Inventory-owned
ordinary transaction and rolls it back, explicitly on both success and refusal. It is not
lock-free: the same ordered line/item/proof/allocation/location/level locking protocol is
used. Preparation also probes new proof primary keys with an insert that is rolled back;
a global conflicting proof is refused before carrier execution without reading another
tenant's proof. No stock level/reservation update is performed during preparation. PostgreSQL transaction advisory locks serialize the tenant/line even when no SKU item
exists, before checking line-wide nullable Variant/allocation history. SQLite takes its
single-writer fence with a zero-row update of the owned proof table; it changes no rows.
Other backends fail closed rather than silently lacking this fence. These locks are released
before carrier I/O; availability is revalidated, not promised across the external call.

### Why Inventory needs durable provenance

The existing return ledger is keyed by return item and has no shipment/cancellation identity.
Reservation quantity zero, soft deletion, legacy cancelled status and caller metadata prove
neither consumption nor restoration. The canonical kernel operation receipt represents a leased
command/result, not an Inventory allocation lifecycle, and reservation metadata has a 32 KiB
physical bound. None may be reinterpreted as a shipment movement proof.

An additive Inventory-owned `inventory_shipment_stock_proofs` table therefore records one
physical-lifecycle projection per Fulfillment item: immutable tenant/Order/Fulfillment/line/
Variant/allocation/location and planned quantity, cumulative native shipped units, and currently
restored undelivered units. Explicit SKU-less native acknowledgements are retained separately
from an explicitly configured no-stock port. These counters describe Inventory effects, not a
second owner of Fulfillment progress. Stock and proof commit together. A nonempty proof table
cannot be silently dropped during rollback. No peer-table FK or foreign writer is introduced.

A first native ship requires zero prior progress; later ship/cancel/reopen requires exact retained
proof and the original allocation. Legacy shipped/cancelled state without proof requires owner
reconciliation, not backfill from counters, migration of old receipts, metadata repair, automatic
restoration or key rotation. The return ledger and potentially applied migration payloads stay
unchanged. Inventory adds only its proof migration/registration; Fulfillment separately adds the
forward-only provider-admission correction described below, without rewriting historical rows.

### Cancellation/return overlap and rollout

Until returned units can be unambiguously associated with a particular delivery, cancellation
restoration/reconsumption refuses a line with a retained return-restock fact. Conversely, fresh
return restock refuses a line with outstanding native restored stock. Exact return receipt replay
remains read-only and does not acquire this new dependency. Both paths serialize on the same
Inventory item locks. This is conservative reconciliation, not a claim to implement partial-unit
lifecycle attribution, unpaid-return eligibility, inspection top-ups or automatic compensation.

Drain old stock/provider writers and in-flight claims before rollout. Install Inventory's
`m20261101_000025_create_shipment_stock_proofs` and Fulfillment's
`m20261101_000121_enforce_provider_effect_admission` through their canonical registries; F121
follows `m20261003_000120_create_checkout_identity_anchor`. Then enable the matching owner and
selected-capability binaries. Mixed old/new writers cannot provide this contract. Neither
migration has been executed here; existing historical rows are not rewritten. Legacy ambiguous
or malformed work stays fenced and requires explicit owner reconciliation, not automatic repair.
Source/guard/regression definitions do not certify compilation, PostgreSQL/SQLite lock timing,
carrier/restart behavior, stock event/outbox/Index convergence or full Product step-8 readiness.

## Native stock outbox continuation (2026-11-01)

This continuation replaces the original "none added" publication behavior for native shipment,
cancellation restoration/reconsumption and returned-stock writes. It does not change their
allocation/provenance/receipt identities, authorize unpaid returns, or introduce inspection top-ups.

Inventory publishes the already registered `DomainEvent::InventoryUpdated` through
`TransactionalEventBus::publish_root_in_tx`, in the exact transaction that changes stock and
proof/receipt. Shipment uses Fulfillment's caller transaction; returned stock uses Inventory's
native transaction. Failure to validate or persist any event refuses and rolls back all local
stock/proof/receipt/counter effects. Commerce, Fulfillment and the host do not write Inventory
notifications or construct a second publisher. No new event type, payload schema or migration is
introduced; the canonical core outbox must already be installed. Unconditional root/typed
outbox publication requires exactly one inserted row; a suppressed insert is not success.
The separate write-once API still correlates an existing exact durable envelope on conflict.

Each physically changed item/location produces one invalidation, grouped over the whole movement
batch. SKU-less acknowledgments, movement-free lifecycle transitions, rolled-back preparation and
exact return receipt replay produce none. Shipment consumption still publishes when stocked and
reserved decrease together and available quantity does not change. Proof/key replay must not
manufacture a notification for a prior effect.

The existing event's quantities remain descriptive, nonnegative global Variant availability
(`sum(stocked - reserved)`), not a per-location counter, reservation promise or physical history.
Before/after values are source observations around this native batch. Multiple changed locations
of the same Variant share those observations; consumers must refetch owner state and must not
apply event-number deltas or order notifications as a replacement ledger. Existing Search ingestion
already routes `inventory.updated` to tenant-scoped Product refresh. This does not wire or certify
an additional generic Index consumer, reservation-write notifications or remote execution.

The bounded native publisher retains tenant-proved Variant/Product identity, correlates every
item/location, and refuses foreign/missing location ownership or a global quantity that cannot be
represented by the existing event's nonnegative i32 fields. It never clamps, relocates stock or
silently drops an event. Preparation preflights representability without writing the outbox.
Human actor UUIDs come from the current PortContext; service/system contexts do not fabricate a
human actor. Neither provider metadata nor operator reasons/claims enter the event.

No existing stock history is backfilled. Events, native stock and proof/receipt atomicity do not
claim distributed atomicity with carrier, Order, financial or Commerce checkpoints. Compilation,
SQL rollback/contention, actual relay/Search delivery, mounted/remote behavior and the remaining
Product step-8 gates require separate verification; regression sources are written but not run.
