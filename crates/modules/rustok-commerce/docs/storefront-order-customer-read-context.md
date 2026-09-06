# Storefront order customer read context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the retained-context gap for customer projection reads in
`crates/rustok-commerce/src/controllers/store/orders.rs`:

- customer lookup used by storefront order-access and ownership helpers;
- the `/store/customers/me` route.

Both paths already delegated to the typed `CustomerReadPort` and mapped failures
through `port_error_to_http_error`. Before this slice, they constructed
`PortContext` inline and moved it into the customer owner call. The HTTP mapper
therefore retained only the returned `PortError`, a consumer operation label, and
an independently passed tenant UUID.

The shared customer lookup in `controllers/store/mod.rs`, GraphQL customer helpers,
checkout runtime consumers, and other adapters are deliberately outside this
slice.

## Delivered source contract

Each order-surface customer read now:

1. constructs one `customer_context` with the existing
   `storefront_customer_port_context` helper;
2. clones that context into
   `CustomerReadPort::read_customer_projection_by_user`;
3. retains the original context for failure diagnostics;
4. maps the same `PortError` through the unchanged shared HTTP policy.

The mapper attributes failures to:

- truthful owner `rustok_customer`;
- exact owner operation `read_customer_projection_by_user`;
- the existing storefront consumer operation such as `get_me`, `get_order`, or an
  order-access operation;
- boundary `commerce_storefront_order_http`.

Diagnostics retain:

- correlation id and tenant id;
- authenticated user id and typed actor;
- channel and locale from the delegated context;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- original owner code, internal message, typed kind, and retryability;
- mapped public code and HTTP status.

Unavailable, timeout, and invariant failures use error severity. Ordinary owner
rejections use warning severity.

## Preserved behavior

This slice does not change:

- `CustomerReadPort` requests or response DTOs;
- the existing customer context constructor, correlation format, actor, locale,
  or two-second deadline;
- `customer.customer_by_user_not_found -> Ok(None)` behavior in order-access
  lookup;
- `/store/customers/me` success response;
- channel-enabled admission before the current-customer route;
- customer-required and order-access-denied HTTP envelopes;
- order ownership lookup and comparison;
- verified customer ID reuse by the refund route;
- order, return, refund, and order-change service calls;
- pagination and filter forwarding;
- payment refund diagnostics added by the preceding storefront refund slice;
- `port_error_to_http_error` status, code, message, and retryability policy;
- FBA, FFA, or ecommerce audit status.

Raw owner evidence remains internal to structured diagnostics. The public HTTP
response is still produced by `port_error_to_http_error` before being returned.

## Static evidence

`scripts/verify/verify-commerce-storefront-order-customer-context.mjs` guards:

- stable customer owner, exact owner operation, and order HTTP boundary;
- mapper inputs for the original `PortError`, retained `PortContext`, user ID, and
  consumer operation;
- technical versus ordinary rejection severity;
- complete available context and original owner error fields;
- mapped public code/status and mapper-before-diagnostics-before-return ordering;
- exactly two retained customer contexts and two delegation clones;
- operation-aware mapper use in order-access lookup and `/customers/me`;
- unchanged customer-not-found fallback, channel guard, order ownership, and safe
  public HTTP policy;
- absence of the two old inline/context-dropping call forms.

The existing broad storefront order HTTP and refund-context verifiers remain
unchanged because their route, mapper-count, ownership, payment, and public-envelope
contracts are preserved.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- the shared storefront customer lookup in `controllers/store/mod.rs`;
- remaining payment execution and compensation consumers;
- remaining order, fulfillment, inventory, customer, tax, and promotion adapters;
- non-`PortError` public envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-storefront-order-customer-context.mjs
node scripts/verify/verify-commerce-storefront-order-http-error-safety.mjs
node scripts/verify/verify-commerce-storefront-order-refund-error-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```
