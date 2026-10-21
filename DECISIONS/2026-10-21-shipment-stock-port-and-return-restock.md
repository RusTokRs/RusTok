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
