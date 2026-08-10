# Commerce REST storefront order-refund owner-read cutover

Status: `source_complete_unvalidated`

Date: 2026-08-10

## Scope

This bounded slice moves mounted `GET /store/orders/{id}/refunds` behind the host-selected
Payment order-read runtime. Commerce no longer constructs `PaymentService` on this route.

Payment now publishes `PaymentOrderReadPort::list_refunds_by_order` with a typed
`ListRefundsByOrderRequest` / `PaymentOrderRefundPage` contract. The in-process adapter owns the
existing `PaymentService::list_refunds` call. The trait method has a default fail-closed
implementation so existing external adapters remain source-compatible but must explicitly
implement the new capability before the route can succeed.

## Preserved transport behavior

The mounted route still:

1. requires the Commerce module to be enabled for the request channel;
2. requires an authenticated customer account;
3. reads the order through the host-selected Order read port and verifies ownership before
   any Payment read;
4. forwards page, per-page, optional status, and the current order id exactly to Payment;
5. leaves `payment_collection_id` unconstrained inside the owner adapter;
6. returns the existing `PaginatedResponse<RefundResponse>` with the same pagination metadata.

The owner adapter delegates to the same `PaymentService::list_refunds` implementation, preserving
its ordering, filtering, count and DTO semantics.

## Context and host composition

Commerce reuses its bounded storefront order read context for the Payment call:

- admitted tenant id;
- authenticated user as `PortActor::user`;
- request locale;
- request channel when present;
- order-bound correlation identity;
- two-second deadline.

`CommerceHttpRuntime::payment_order_read_port()` already exposes the host-selected
`PaymentOrderReadRuntime`; no embedded fallback is introduced in the controller.

## External adapter compatibility

`PaymentOrderReadPort::list_refunds_by_order` has a default implementation that first enforces
read-port deadline semantics and then returns a stable unavailable `PortError`. Existing external
implementations therefore continue to compile but fail closed for this newly published capability
until they opt in explicitly.

## Public error parity and diagnostics

The Payment owner translates concrete errors to bounded port codes. Commerce preserves the
existing storefront public families, including provider-unavailable, provider-invalid-response,
reconciliation-required, provider-not-configured, generic validation/not-found/conflict and
storage-unavailable envelopes.

The mounted Payment boundary logs correlation, identity-presence facts, owner error kind,
owner-code length, retryability and the selected public envelope. It does not log or return the
raw owner/backend message or concrete Payment error.

## Topology status

This removes the mounted direct `PaymentService` construction from storefront order refund reads.
The canonical broad ecommerce topology P0 remains open because other mounted Product/Order/
Payment/Fulfillment topology and validation work still remains.

## Validation status

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, REST scenarios,
workflows, CI reruns, database scenarios, provider calls, restart scenarios, or remote-adapter
scenarios were executed. Source guards added or updated by this slice are source-reviewed only
and were not run.
