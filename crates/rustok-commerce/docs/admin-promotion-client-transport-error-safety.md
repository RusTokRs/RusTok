# Commerce Admin promotion client transport error safety

Status: **source-ready / unvalidated**

## Scope

This source slice covers the final Commerce Admin promotion facade in
`crates/rustok-commerce/admin/src/transport/promotion.rs`.

Covered operations:

- `preview_cart_promotion`
- `apply_cart_promotion`

## Confirmed gap

The mounted SSR promotion adapter already owns correlation-aware context and cart
promotion `PortError` mapping. The public facade nevertheless returned the shared
native `ApiError` directly. That type stores `ServerFn(String)`, so framework,
serialization, transport, or unexpected server-function text could still become
the displayed Admin error after leaving the native adapter.

## Source policy

Each promotion facade operation now creates a `PromotionClientErrorContext` before
the unchanged native adapter call and maps only a final returned `ApiError`.

The original typed native error is retained only in structured diagnostics with:

- the Commerce Admin promotion consumer and exact operation;
- a per-call correlation id;
- the client transport boundary and a stable error code;
- cart-id presence and character length;
- promotion-payload presence.

The cart id and promotion draft values, including scope, kind, source id, line item,
discount, amount, metadata, and any other payload fields, are not logged by this
client boundary.

The existing public error contract remains `ApiError`, but preview and apply now
return the same static final message:

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

The broad ecommerce mapper-cleanup item remains open for tax, payment,
fulfillment, order, remaining promotion paths, and other non-`PortError` public
envelopes.
