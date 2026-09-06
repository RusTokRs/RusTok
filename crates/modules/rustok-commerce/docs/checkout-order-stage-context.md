# Checkout order stage error safety

Status: **source-reviewed / unvalidated**

## Scope

This source wave closes the mounted Commerce checkout order-stage consumer mapper for three Order owner calls:

- `CheckoutOrderRecoveryAdapter::recover_existing_checkout`;
- `CheckoutCompletionPort::complete_checkout`;
- `CheckoutOrderRecoveryAdapter::read_checkout_order`.

The established order-stage implementation is retained unchanged in `checkout_order_stages_legacy.rs` and mounted only through `checkout_order_stages.rs`.

The facade wraps the canonical Order owner boundaries before their failures return to the retained mapper. The complete canonical `PortContext` is still delegated to Order. Only the Commerce consumer-facing error and diagnostic projection changes.

## Public and persisted error policy

The Order owner code, typed kind, and retryability remain available to the Commerce boundary. Owner message text is not copied into `CheckoutOrderStageError::Boundary`, staged pipeline error strings, or checkout-operation error persistence.

| Owner kind | Order-stage message |
| --- | --- |
| `Validation` | `Checkout order request is invalid` |
| `NotFound` | `Checkout order resource was not found` |
| `Conflict` | `Checkout order state conflicts with the requested operation` |
| `Forbidden` | `Checkout order operation is not permitted` |
| `Unavailable` / `Timeout` | `Checkout order service is temporarily unavailable` |
| `InvariantViolation` | `Checkout order operation could not be completed safely` |

The existing `CheckoutOrderStageError::Boundary` shape remains unchanged:

- `stage` remains the existing Commerce stage (`recover_existing`, `complete`, or `read_order`);
- `code` remains the exact Order owner code;
- `message` is now the static kind-based Commerce message;
- `retryable` remains the Order owner value.

This slice does not change staged-checkout failure disposition. Any retry/compensation policy for order-stage failures remains a separate task.

## Diagnostics

The Commerce adapter emits only bounded facts:

- a diagnostic token whose `Debug` output is `redacted`;
- truthful owner `rustok_order` and exact owner operation;
- tenant and actor identity shapes;
- actor kind plus claim and role counts;
- channel, locale, correlation, causation, trace, and idempotency presence/shape facts;
- deadline milliseconds;
- exact owner code, typed kind, and retryability;
- owner-message presence/shape and character length;
- boundary `commerce_checkout_order_stage_adapter`.

Unavailable, timeout, and invariant failures use error severity. Validation, not-found, conflict, forbidden, and other ordinary rejections use warning severity.

The retained compatibility tracing macros are shadowed, so the former raw `PortError`, owner message, tenant, actor, channel, locale, correlation, causation, traceparent, and idempotency values are not emitted a second time.

## Preserved behavior

This source wave does not change:

- `CheckoutCompletionPort` or Order recovery request/response contracts;
- immutable order plan persistence;
- inventory reservation execution;
- legacy snapshot/request hash construction or adoption semantics;
- recovery-before-completion ordering;
- completion request reuse;
- completion idempotency key `checkout:{operation_id}:order:complete`;
- projection read request locale/fallback-locale values;
- completion result/order projection identity validation;
- typed `OrderStatusKind` admission;
- inventory adoption;
- `inventory_reserved -> order_created -> payment_ready` checkpoints;
- successful DTOs;
- public executor methods or custom canonical completion-port injection;
- HTTP, GraphQL, or native contracts;
- Commerce FFA or FBA status.

## Evidence

- `crates/rustok-commerce/contracts/evidence/checkout-order-stage-error-safety-source-review.json`
- `scripts/verify/verify-commerce-checkout-order-stage-context.mjs`
- `scripts/verify/verify-commerce-checkout-completion-cutover.mjs`
- `scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`
- `scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs`

## Remaining work

The broader ecommerce correlation-safe mapper cleanup remains open for remaining Order adapters, inventory, customer, tax, promotion, other ecommerce adapters, and non-`PortError` public envelopes.

Compile, runtime, replay, restart, remote-port, cross-transport, database, workflow, and CI evidence also remain open. The retained legacy order-stage source should be removed only after compile/replay/upgraded-path evidence proves the compatibility source is no longer required.

## Intended maintainer checks

```bash
node scripts/verify/verify-commerce-checkout-order-stage-context.mjs
node scripts/verify/verify-commerce-checkout-completion-cutover.mjs
node --test scripts/verify/verify-commerce-checkout-completion-cutover.test.mjs
node scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs
node scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs
cargo check -p rustok-commerce --lib
```

No tests, Node verifiers, Cargo commands, formatting, Order owner calls, database scenarios, restart scenarios, remote-port scenarios, workflows, or CI were executed.
