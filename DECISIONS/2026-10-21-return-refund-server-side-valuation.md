# Return refund amounts are valued on the server from the order line snapshot

- Date: 2026-10-21
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-commerce (return decision orchestration); rustok-order (line snapshot); rustok-payment (refund execution)
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

Finding ECOM-POST-03: a return refund amount came from caller-supplied `refund_amount` item
metadata or a free `amount`; only the captured total capped it. Nothing tied the refund to the
price, discount and tax of the returned lines or to what was actually shipped.

## Decision

`return_refund_value.rs` in `rustok-commerce` values a return from the order read through the Order
owner read port:

`value(line) = ((line.total_price - line discounts - allocated order-level discount) + line tax) × returned_qty / ordered_qty`

- the order-level discount is allocated proportionally to `line.total_price`;
- line tax is added only when prices are tax-exclusive (`tax_included = false`);
- shipping charges are not part of a return's value;
- the sum is rounded once to the currency's minor unit.

The refund amount is that value by default. An explicit `amount` is accepted only when it does not
exceed it (goodwill partial refund); a larger amount is a validation error. Item `metadata.refund_amount`
is no longer read. A return without items cannot be valued; only an explicit positive amount is
accepted and the payment-level cap still applies.

Quantity truth: when the order has fulfillment records, a physical line cannot be returned beyond
its shipped quantity; digital lines and orders without fulfillment records are exempt.

Both the mounted owner-port orchestration (`ReturnDecisionOwnerOrchestrationService`, which now needs
`with_order_read_port` and fails closed without it) and the legacy `PostOrderOrchestrationService`
use the same functions.

## Sources of truth and ownership

Order lines, adjustments and tax lines: `rustok-order`. Shipped quantity: `rustok-fulfillment`.
Refund execution and captured cap: `rustok-payment`. The valuation is a pure derived computation in
commerce.

## Invariants

### Allowed states

A return refund ≤ value of the returned lines, and ≤ the collection's refundable remainder.

### Forbidden states

A refund driven by caller metadata; a refund above the returned lines' value; returning unshipped
physical goods when shipment truth exists.

## Non-goals

Restocking inventory on `complete_return` (needs an inventory restock port with an idempotency ledger,
and a per-item resellable decision: not implemented); requiring `paid`-or-later status for creating
return records (a refund already requires a captured collection, and ~15 existing order and commerce
tests create returns on unpaid orders); cumulative shipped-quantity checks across several returns;
shipping refunds.

## Data, transaction, and concurrency boundary

No new persisted state. Refund creation keeps the existing keyed creation. The order is read once per
decision; a concurrent order change can only make the value stale by cents, and the payment cap
remains the hard bound.

## Context dimensions

Tenant and currency from the order; locale is irrelevant.

## Events and projections

None added.

## Failure semantics

Invalid or over-valued requests fail with a validation error after the return record was created (as
before for refund failures). A missing order read port fails closed.

## Migration and cutover

Callers that sent `metadata.refund_amount` must send priced return items or an explicit amount within
the value. No schema change.

## Alternatives considered

Order-owned valuation inside `create_return` (needs payments/fulfillment knowledge the order owner does
not have); trusting operators (current behavior, rejected by the finding).

## Verification

Unit tests in `return_refund_value.rs`. Existing bridge test uses an item-less return with an explicit
amount and stays valid. **Not compiled or run when written.**

## Consequences

Refunds above the goods' value (e.g. shipping) must use the plain refund path. Rounding across several
partial returns of one line can differ by a minor unit; the payment cap bounds it.
