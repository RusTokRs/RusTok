# Commerce REST storefront payment-collection owner-port cutover

Status: `source_complete_unvalidated`

## Scope

The mounted Commerce REST `POST /store/payment-collections` handler no longer constructs
`rustok_payment::PaymentService` and no longer consumes concrete `PaymentError` values.

The route now uses two Payment-owned capabilities supplied through `CommerceHttpRuntime`:

- `PaymentCartReadPort::find_reusable_collection_by_cart` for the pre-create reusable lookup;
- `PaymentCollectionPort::create_or_reuse_collection` for owner-local collection creation and
  concurrent create-race adoption.

`CommerceHttpRuntime` now requires the corresponding `PaymentCartReadRuntime` and
`PaymentCollectionRuntime` from `HostRuntimeContext` and exposes only their trait ports to mounted
handlers. The application server host-composes both runtimes host-first, then server-shared, and only
then falls back to the Payment-owned in-process adapters.

No new Payment persistence or lifecycle policy is implemented in Commerce.

## REST parity

The existing route sequence is preserved:

1. storefront channel admission;
2. current customer resolution;
3. cart read and customer access check;
4. completed-cart rejection;
5. cart repricing;
6. StoreContext resolution;
7. reusable Payment collection lookup;
8. collection creation only after a reusable miss.

A collection that already exists at the pre-create owner read still returns `200 OK`.

After a reusable miss, the handler invokes `PaymentCollectionPort::create_or_reuse_collection` and
returns `201 Created`. This preserves the old route behavior when a concurrent creator wins after the
initial read: the Payment owner may adopt the reusable collection internally, while the REST handler
still returns the post-miss `201 Created` response just as the previous `PaymentService::create_collection`
race-recovery path did.

The owner request preserves the prior projection: cart id, no order id, cart customer id, cart
currency, repriced cart total, and the same merged caller/cart/StoreContext metadata.

## Owner call context

Both Payment owner calls carry trusted transport facts:

- admitted tenant id;
- authenticated user actor when present, otherwise the stable
  `rustok-commerce.storefront-payment-collection` service actor;
- request locale with `und` only for a blank locale;
- request channel slug when present;
- cart/operation-bound correlation id;
- two-second deadline.

The create/reuse call additionally carries the cart-bound write admission identity
`storefront-payment-collection:{cart_id}`. It exists to satisfy the canonical write-port admission
contract. This slice does **not** claim that the identity is a durable Payment receipt, an exactly-once
transport record, or a new replay journal. Reuse and create-race adoption remain Payment-owned
behavior.

No public `Idempotency-Key` requirement is added to `POST /store/payment-collections` in this slice.

## Error compatibility and diagnostics

The REST boundary now maps bounded `PortError` facts while retaining the existing public families:

- validation -> `payment_request_invalid` / 400;
- missing resource -> `payment_resource_not_found` / 404;
- lifecycle conflict -> `payment_state_conflict` / 409;
- provider rejection -> `payment_provider_rejected` / 409;
- provider outcome unknown or invalid response -> `payment_reconciliation_required` / 409;
- provider/configuration/timeout availability failures -> `payment_temporarily_unavailable` / 503;
- collection storage failure and the reusable-read `payment.cart_read_unavailable` owner code ->
  `payment_storage_unavailable` / 503.

Unexpected forbidden or invariant owner failures fail closed as `payment_operation_failed` / 500.

Commerce diagnostics retain request-shape context, correlation id, bounded owner error kind, owner
code length, retryability, selected public code, and status. Raw owner/backend error messages are not
logged or returned by this mapper.

## Deliberately still open

This slice removes one mounted REST Payment concrete-service construction only. It does not claim
that every mounted Commerce Product, Order, Payment, or Fulfillment construction has been removed.

The canonical ecommerce topology item remains open.

A fresh mounted-path audit is required before that broad P0 can be closed.

## Validation status

No tests, Cargo commands, Node verifiers, formatter, REST scenarios, workflows, CI reruns, database
scenarios, provider calls, restart scenarios, or remote-adapter scenarios were executed for this
slice, per maintainer instruction. The updated verifier is source-only evidence for maintainer
execution.