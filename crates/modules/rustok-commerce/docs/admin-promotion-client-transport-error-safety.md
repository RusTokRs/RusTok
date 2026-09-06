# Commerce Admin promotion client transport error safety

Status: **source-ready / unvalidated**

## Scope

This source slice covers the final Commerce Admin promotion facade in
`crates/rustok-commerce/admin/src/transport/promotion.rs` and its client error
policy in
`crates/rustok-commerce/admin/src/transport/promotion_client_error_safety.rs`.

Covered operations:

- `preview_cart_promotion`
- `apply_cart_promotion`

## Confirmed gap

The mounted SSR promotion adapter owns context and cart-promotion `PortError`
mapping. The public facade then converts a final returned `ApiError` to one static
Admin message. Before this follow-up, that final mapping also wrote the complete
`ApiError` into structured diagnostics.

Both `ApiError::Graphql(String)` and `ApiError::ServerFn(String)` can contain
framework, serialization, transport, GraphQL, or unexpected server-function text.
That text is not required to correlate or classify the facade failure.

## Source policy

Each promotion facade operation creates a `PromotionClientErrorContext` before the
unchanged native adapter call and maps only a final returned `ApiError`.

The client diagnostic now records only:

- the Commerce Admin promotion consumer and exact operation;
- a per-call correlation id;
- the client transport boundary and stable error code;
- cart-id presence and character length;
- promotion-payload presence;
- the typed native error variant (`graphql` or `server_fn`);
- native error-message presence and character length.

The complete native error and its message are not logged. Cart-id and promotion
draft values, including scope, kind, source id, line item, discount, amount,
metadata, and any other payload fields, are also not logged by this boundary.

The existing public error contract remains `ApiError`, and preview/apply continue
to return the same static final message:

`Commerce admin promotion request could not be completed`

## Preserved behavior

This slice does not change:

- the default or SSR native adapter files;
- the two mounted promotion server-function endpoints;
- owner `PortError` mapping and server-side diagnostics;
- authentication, tenant, permission, channel, locale, or idempotency policy;
- cart-promotion request parsing or normalization;
- preview/apply owner calls, invocation order, or result DTOs;
- order-change transport functions that share the native adapter;
- the `ApiError` result type used by existing callers and tests.

## Evidence boundary

The retained JSON evidence is source-only. Tests, focused verifiers, Cargo,
formatting, workflows, CI, hydrate compilation, SSR compilation, browser behavior,
and mounted preview/apply failure behavior were not executed by this implementation
agent.

The SSR promotion consumer still records raw framework extraction and owner
`PortError` values together with full tenant, user, cart, channel, and locale
identities. That server-side diagnostic cleanup is explicitly outside this
client-only slice and remains open.

The broad ecommerce mapper-cleanup item remains open for the SSR promotion
consumer, payment, fulfillment, order, inventory, customer, remaining tax and
promotion paths, and other non-`PortError` public envelopes.
