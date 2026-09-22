# Commerce REST storefront order-return command owner-port cutover

Status: `source_complete_unvalidated`

Date: 2026-08-10

## Scope

This bounded slice moves mounted `POST /store/orders/{id}/returns` behind the existing
Order-owned `OrderPostOrderCommandPort::create_return` capability.

The route remains mounted in the same controller and keeps the pre-existing storefront
channel admission and customer-ownership read before any return mutation is attempted.
The old concrete `OrderService` construction and concrete `OrderError` mapper are removed
from the mounted storefront order controller.

## Preserved transport behavior

The mounted route still:

1. requires the Commerce module to be enabled for the request channel;
2. requires an authenticated customer account;
3. reads the order through the host-selected `OrderReadPort` and rejects orders owned by
   another customer before mutation;
4. accepts the unchanged `CreateOrderReturnInput` request body;
5. returns `201 Created` with the existing `OrderReturnResponse` projection;
6. preserves the prior public Order error families:
   - validation -> `400 commerce_store_order_invalid`;
   - not found -> `404 commerce_store_order_not_found`;
   - state conflict -> `409 commerce_store_order_state_conflict`;
   - unavailable/timeout -> `503 commerce_store_order_unavailable`;
   - invariant failures -> `500 commerce_store_order_failed`.

A defensive `PortErrorKind::Forbidden` mapping remains `401 commerce_store_order_access_denied`,
consistent with the storefront ownership boundary.

## Owner context

After the ownership check, Commerce creates a write `PortContext` with:

- the admitted tenant id;
- the authenticated user id as `PortActor::user`;
- the request locale;
- the request channel when present;
- correlation identity `commerce-storefront-order:create-return:{order_id}`;
- a two-second deadline;
- a fresh UUID idempotency identity required by the current owner write-admission policy.

The fresh UUID is **write-admission metadata only**. `OrderPostOrderCommandPort` does not retain
a durable command receipt keyed by that value, so this slice does not claim exactly-once or
durable replay semantics for storefront return creation.

## Error safety

The new command boundary maps only bounded `PortError` kind/code-length/retryability and
request identity-presence facts. It does not log or return raw owner/backend messages from
the command call.

Existing storefront Order read/customer diagnostics are outside this slice and remain unchanged.

## Subsequent topology update

The Payment refund-list gap that was intentionally left after this command slice was subsequently
moved behind `PaymentOrderReadPort::list_refunds_by_order` in the bounded storefront refund-read
cutover dated 2026-08-10. This record no longer asserts direct mounted `PaymentService`
construction for `GET /store/orders/{id}/refunds`.

The canonical broad ecommerce topology P0 remains open for other remaining topology and
validation work.

## Validation status

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, REST scenarios,
workflows, CI reruns, database scenarios, provider calls, restart scenarios, or remote-adapter
scenarios were executed. Source guards added or updated by this slice are source-reviewed only
and were not run.
