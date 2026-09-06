# Commerce GraphQL payment query owner-port and error safety

Status: `source_closed_unvalidated`

## Scope

This source wave keeps the mounted Commerce GraphQL payment query contract unchanged while removing concrete `rustok_payment::PaymentService` construction from its private compatibility facade.

The resolver source in `crates/rustok-commerce/src/graphql/query.rs` remains unchanged. Its seven `PaymentService::new(db.clone())` expressions still call the same six logical reads and return the same successful payment collection/refund DTOs. Existing `PaymentCollectionNotFound` and `RefundNotFound` branches continue to return `None` where the GraphQL contract already requires that behavior.

## Owner capabilities

The private Payment compatibility facade now delegates to typed Payment owner capabilities instead of constructing `rustok_payment::PaymentService`:

- `PaymentCartReadPort::find_reusable_collection_by_cart` for the storefront cart-associated reusable collection lookup;
- `PaymentOrderReadPort::find_latest_collection_by_order` for the order-associated lookup;
- `PaymentAdminReadPort::{read,list}_payment_collection_projection` for collection detail/list reads;
- `PaymentAdminReadPort::{read,list}_refund_projection` for refund detail/list reads.

`PaymentCartReadPort` / `PaymentCartReadRuntime` are the only new Payment owner API in this slice. The in-process adapter owns the concrete `PaymentService`; Commerce never imports Payment ORM entities or raw SQL for these reads.

## GraphQL runtime composition

`CommercePaymentReadRuntime` composes the three narrow Payment owner runtimes. The mounted GraphQL extension carries that composite through a task-local resolver scope together with request-owned actor, channel, and locale facts.

Schema composition prefers a host-shared `CommercePaymentReadRuntime`. If one is not supplied, it composes the runtime from any individually shared `PaymentAdminReadRuntime`, `PaymentOrderReadRuntime`, and `PaymentCartReadRuntime`, falling back to in-process owner adapters only for capabilities the host did not provide. Directly embedded compatibility schemas therefore retain an explicit owner-runtime fallback without returning to concrete Payment construction in the resolver facade.

Each shim read builds a bounded two-second `PortContext` with the current tenant, validated user actor or stable service actor, resolved locale, resolved channel when present, and a stable correlation token. These are read calls; no write idempotency or durable receipt is claimed.

## Public envelopes

The facade retains the existing GraphQL compatibility variants while classifying owner `PortError` structurally:

| Owner result | Public code | Public message | Retryable |
| --- | --- | --- | --- |
| Validation | `PAYMENT_REQUEST_INVALID` | `Payment query is invalid` | false |
| Not found | `PAYMENT_RESOURCE_NOT_FOUND` | `Payment resource was not found` | false |
| Conflict | `PAYMENT_STATE_CONFLICT` | `Payment state conflicts with this query` | false |
| Unavailable/timeout | `PAYMENT_TEMPORARILY_UNAVAILABLE` | `Payment data is temporarily unavailable` | true |
| Invariant/reconciliation | `PAYMENT_RECONCILIATION_REQUIRED` | `Payment state requires reconciliation` | false |
| Configuration-coded owner failure | `PAYMENT_CONFIGURATION_ERROR` | `Payment provider configuration is invalid` | false |

Configuration compatibility is recognized only from the three closed built-in owner codes `payment.admin_read_configuration`, `payment.order_read_configuration`, and `payment.cart_read_configuration`; arbitrary external adapter codes are not logged or pattern-matched as configuration.

The compatibility `PaymentCollectionNotFound` and `RefundNotFound` enum variants are reconstructed only from owner `NotFound` plus the requested resource kind, preserving the unchanged branches in `query.rs`.

Validation text, provider identifiers and operation text, lifecycle values, database causes, complete owner errors, and arbitrary owner codes are not copied into GraphQL responses.

## Bounded diagnostics

The Commerce facade records only a redacted diagnostic token, owner operation, stable correlation token, tenant/resource shape facts, structural `PortError` kind, owner-code character length, selected public code, and retryability. The owner code itself is not logged.

The Payment admin read owner adapter was also tightened in this slice: its technical failure log no longer emits `error = ?error`. Order and cart read owner adapters already log only bounded error variants/codes and shape facts.

## Preserved contracts

- `rustok-payment` remains the owner of payment collection/refund persistence and read behavior.
- The underlying Payment service and six canonical read methods remain intact inside owner adapters.
- `query.rs`, GraphQL fields, authorization checks, filters, pagination, result ordering, and DTO conversions are unchanged.
- No Payment provider call is introduced by these read ports.
- Commerce and Payment FFA/FBA status is unchanged.
- The broad Commerce topology P0 remains open for remaining GraphQL mutations/provider operations, post-order/change/return, checkout/reconciliation, and other concrete-owner paths.
- No compile, runtime, mounted GraphQL, remote-adapter, or parity evidence is claimed.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-query-payment-error-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-query-payment-error-safety.mjs`

## Still open

- Execute the focused source verifier and Cargo checks under maintainer control.
- Retain mounted success/not-found/validation/database/reconciliation GraphQL evidence.
- Retain remote-adapter selection evidence proving host-selected Payment read runtimes are used by mounted GraphQL.
- Continue the remaining broad Commerce topology cutover.

## Intended checks

```bash
node scripts/verify/verify-commerce-graphql-query-payment-error-safety.mjs
cargo check -p rustok-commerce --lib
cargo check -p rustok-payment --lib
```

Source/GitHub inspection only. No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, CI, runtime calls, or remote-adapter execution were performed.
