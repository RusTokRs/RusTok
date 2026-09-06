# Commerce GraphQL storefront payment-collection owner-port cutover

Status: `source_complete_unvalidated`

## Scope

The mounted Commerce GraphQL `createStorefrontPaymentCollection` mutation no longer constructs
`rustok_payment::PaymentService`.

The resolver now composes two existing Payment-owned capabilities:

- `PaymentCartReadPort::find_reusable_collection_by_cart` for the pre-create reusable lookup;
- `PaymentCollectionPort::create_or_reuse_collection` for owner-local creation and concurrent-race
  adoption.

`rustok-payment` now publishes the lightweight `PaymentCollectionRuntime` composition wrapper so a
host can select an external `PaymentCollectionPort`. Its built-in in-process adapter is the only new
place in this slice that constructs concrete `PaymentService` for this capability.

`CommercePaymentCommandRuntime` carries the host-selected `PaymentCollectionRuntime` alongside the
already existing Payment collection lifecycle and refund command runtimes. Mounted GraphQL receives
that runtime from `GraphqlRuntimeInputs`; directly embedded compatibility schemas retain the explicit
Payment-owned in-process fallback.

The application server also publishes `PaymentCollectionRuntime` into `HostRuntimeContext`,
preserving a host- or server-selected implementation before using the Payment-owned in-process
adapter.

## Resolver parity

The old resolver performed its reusable lookup before parsing caller-provided payment metadata. This
ordering is preserved deliberately:

1. Commerce completes its existing storefront module/channel, cart access, repricing, and store
   context resolution;
2. the host-selected `PaymentCartReadPort` checks for a reusable collection;
3. an existing collection is returned immediately, without parsing the new request metadata;
4. only on a reusable miss does Commerce parse/merge metadata and call
   `PaymentCollectionPort::create_or_reuse_collection`.

The owner create/reuse operation performs its own reusable check and rechecks after a create failure,
so a concurrent creator can be adopted inside the Payment owner instead of requiring Commerce to
reimplement Payment persistence-race policy.

The request projection remains unchanged: cart id, no order id, cart customer id, cart currency,
repriced cart total, and the same merged cart/store-context metadata are sent to Payment.

## Owner call context

Both Payment owner calls receive trusted `PortContext` values derived from the admitted tenant and
request data:

- authenticated storefront requests use the validated user actor;
- guest storefront requests use the stable
  `rustok-commerce.graphql-storefront-payment-collection` service actor;
- locale comes from `RequestContext`, falling back to `und` only when blank;
- request channel is propagated when present;
- read and write calls carry cart-bound correlation identities;
- both calls use a two-second deadline.

The create/reuse call also carries the stable cart-bound write identity
`graphql-storefront-payment-collection:{cart_id}` to satisfy canonical write-port admission. This
slice does **not** claim that this transport identity is a durable Payment receipt or exactly-once
command record. Reuse and create-race adoption are provided by Payment's existing owner behavior,
not by a new Commerce replay journal.

## Public error compatibility

Concrete `PaymentError` no longer crosses this mounted resolver boundary. The GraphQL mapper consumes
bounded `PortError` facts and preserves the existing public envelope families where the owner code
retains the distinction:

- validation -> `payment_request_invalid`;
- missing resource -> `payment_resource_not_found`;
- lifecycle conflict -> `payment_state_conflict`;
- provider rejection -> `payment_provider_rejected`;
- provider outcome unknown / invalid response -> `payment_reconciliation_required`;
- provider configuration/unavailable -> `payment_temporarily_unavailable`;
- Payment storage failure -> `payment_storage_unavailable`.

Unexpected forbidden/invariant failures fail closed as `payment_operation_failed`. Diagnostics expose
only bounded owner kind/code length and request-context facts; raw owner/backend messages are not
logged or returned by the Commerce mapper.

## Deliberately still open

This slice only removes the mounted GraphQL storefront payment-collection concrete Payment service
construction. It does not claim that every remaining Commerce Product, Order, Payment, or Fulfillment
transport/service construction has been removed.

The canonical ecommerce topology item
`Move remaining mounted Commerce REST/GraphQL construction of Product, Order, Payment, and
Fulfillment concrete services behind host-composed owner ports` therefore remains open pending a
fresh mounted-path audit.

## Validation status

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, mounted GraphQL
scenarios, workflows, CI reruns, database scenarios, provider calls, restart scenarios, or
remote-adapter scenarios were executed for this slice. The accompanying verifier changes are
source-only evidence for maintainer execution.
