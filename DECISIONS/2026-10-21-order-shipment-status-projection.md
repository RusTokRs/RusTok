# Order shipped/delivered status follows fulfillment truth; stock consumption stays trigger-owned for now

- Date: 2026-10-21
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-commerce (projection); rustok-fulfillment (shipment truth); rustok-order (status); rustok-inventory (stock)
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

ECOM-SHIP-01: order `ship`/`deliver` are manual admin commands independent of fulfillment
`ship`/`deliver`, so the two truths can disagree. Delivery is already guarded in the database
(`m20260713_000008` refuses `shipped → delivered` without delivered fulfillments); shipping is not.

ECOM-INV-01 as first written claimed that a reship after full shipment hits
`active inventory reservation is missing`. Re-reading the code shows that `reopen_fulfillment` and
`reship_fulfillment` change only `delivered_quantity`; `shipped_quantity` never decreases, so the
`fulfillment_inventory_consume_on_ship` trigger (`m20260713_000007`, fires on growth of
`shipped_quantity`) does not fire on reship. Reservations are one row per order line
(`reservation_items.id = line_item_id`), so `LIMIT 1` is not a defect either. What remains is
architectural debt (the trigger reads and writes order and inventory tables, AGENTS §5/§7) and one
behavioral gap: cancelling a fulfillment that already shipped does not return stock.

## Decision

1. **Projection.** `OrderShipmentProjector` derives a `ShipmentTruth` (`NoFulfillments | Open | Shipped
   | Delivered`) from live (non-cancelled) fulfillments: every physical order line fully shipped /
   delivered by the sum of item quantities, fulfillments without items by status. After a successful
   fulfillment `ship` or `deliver` admin command, the projector advances a `paid` order to `shipped`
   (carrier and tracking taken from the shipped fulfillment) and a `shipped` order to `delivered` when
   the truth is `Delivered`. The projection is installed as a decorator of
   `FulfillmentAdminCommandPort`; its failure is logged and never fails the committed fulfillment
   command.
2. **Guard.** The manual order `ship` command is refused while live fulfillments are not fully shipped
   (`order_shipment.fulfillment_not_shipped`); `deliver` is refused unless they are fully delivered.
   Orders without fulfillment records keep the manual behavior for `ship`. The status never moves
   backwards.
3. **Inventory.** *(Superseded for the stock move by
   [the shipment stock port ADR](./2026-10-21-shipment-stock-port-and-return-restock.md).)* The shipping stock-consumption trigger stays the owner in this change. Moving it
   into an inventory port called in the fulfillment transaction needs a migration that drops the
   trigger and a port contract that can be exercised against PostgreSQL; neither can be validated in
   this change, and a wrong cutover would double-consume or skip stock. The target contract is
   recorded here and remains open work.

Target contract for the inventory move: `InventoryShipmentConsumptionPort::consume_shipped(context,
{order_line_item_id, quantity})`, executed by the fulfillment owner inside the transaction that
increases `shipped_quantity`, with `restore_unshipped(...)` for cancellation of shipped quantity.

## Sources of truth and ownership

Shipment truth: `rustok-fulfillment`. Order status: `rustok-order`. The projection is derived and
recomputable from fulfillments on every call.

## Invariants

### Allowed states

Order `shipped` ⇒ live fulfillments (if any) fully shipped; order `delivered` ⇒ fulfillments fully
delivered.

### Forbidden states

An order `shipped` through the manual command while its fulfillments are open.

## Non-goals

Reverting an order status when a fulfillment is reopened; projection from provider reconciliation
paths that bypass the admin command port (the manual command repairs them); moving the trigger.

## Data, transaction, and concurrency boundary

The projection runs after the fulfillment commit in its own order transaction and is idempotent:
it recomputes truth and applies only the missing forward transitions.

## Context dimensions

Tenant and actor from `PortContext`; no channel or locale dependence.

## Events and projections

Order events `OrderShipped`/`OrderDelivered` are emitted by the order owner as before.

## Failure semantics

Projection failure: log a warning, leave the order status, repair through the next projection or the
manual command. Guard read failure: reject the manual command (fail closed).

## Migration and cutover

No schema change. GraphQL wraps the fulfillment lifecycle runtime only when the request scope carries
a `TransactionalEventBus`; otherwise the projection is skipped and the manual command remains.

## Alternatives considered

Order status derived at read time (breaks stored status and existing triggers); event subscriber
(asynchronous, no guard); replacing the trigger now (unverifiable).

## Verification

Unit tests in `order_shipment_projection.rs` for truth derivation and guard verdicts. **Not compiled
or run when written.**

## Consequences

Operators ship orders by shipping fulfillments. Stock consumption and shipped-fulfillment cancellation
stock return stay open under ECOM-INV-01.
