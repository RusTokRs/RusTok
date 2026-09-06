# Commerce GraphQL cart read error safety

Status: `source_closed_unvalidated`

## Scope

This source wave closes the identified typed-error loss in mounted Commerce GraphQL storefront cart reads.

The compatibility resolver source in `crates/rustok-commerce/src/graphql/query.rs` remains unchanged. Its three existing calls to `read_storefront_cart` continue to use the same `PortContext`, `CartStorefrontReadRequest`, cart identity, successful `CartResponse`, access policy, shipping enrichment, storefront cart projection, and reusable payment-collection lookup.

Two existing resolver branches still compare the owner code `cart.cart_not_found` and return `None`. No not-found behavior or GraphQL nullability contract is changed.

## Typed facade

The mounted safe-query source aliases only the Cart dependency used by the unchanged compatibility query. The facade wraps the canonical owner-managed `rustok_cart::CartStorefrontPort` returned by `in_process_cart_storefront_port` and delegates the same `read_storefront_cart` operation.

The original resolver reads `error.code` and converts `error.message`. The facade preserves that source contract:

- `code` remains the exact owner code needed by the two compatibility not-found guards;
- `message` is a typed `CartGraphqlMessage`, not the owner message string;
- the complete `PortError` remains typed until the transport-owned GraphQL mapper.

## Public envelopes

Transport responses are classified structurally by `PortErrorKind`.

| Owner kind | Public code | Public message | Retryable |
| --- | --- | --- | --- |
| Validation | `CART_REQUEST_INVALID` | `Cart query is invalid` | false |
| Not found | `CART_RESOURCE_NOT_FOUND` | `Cart was not found` | false |
| Conflict | `CART_STATE_CONFLICT` | `Cart state conflicts with this query` | false |
| Forbidden | `CART_ACCESS_DENIED` | `Cart query is not permitted` | false |
| Unavailable/timeout | `CART_TEMPORARILY_UNAVAILABLE` | `Cart data is temporarily unavailable` | true |
| Invariant violation | `CART_OPERATION_FAILED` | `Cart query could not be completed safely` | false |

The owner message, internal invariant details, database causes, context identities, and complete owner error are not copied into the GraphQL response.

## Bounded diagnostics

The facade records only:

- a diagnostic token whose `Debug` output is always `redacted`;
- canonical owner and operation;
- the trusted correlation id already carried by `PortContext`;
- structural shapes for tenant, actor, channel, locale, causation, traceparent, and cart identity;
- claim and role counts plus deadline;
- structural owner error kind, stable owner code, retryability, and owner-message presence/length;
- selected public code and retryability;
- error severity for unavailable, timeout, and invariant failures;
- warning severity for validation, not-found, conflict, and forbidden rejections.

The complete `PortError`, owner message content, actor id, tenant id, channel, locale, traceparent, causation id, and cart UUID are not logged by this boundary.

## Preserved contracts

- `rustok-cart` remains the owner of cart persistence, lifecycle, and storefront read behavior.
- The canonical owner port, operation, contexts, requests, and success DTO are unchanged.
- The query source, GraphQL fields, access checks, shipping enrichment, payment lookup, and not-found `None` behavior are unchanged.
- Commerce and Cart FFA/FBA status is unchanged.
- The broad ecommerce mapper and public-envelope cleanup remains open.
- No compile, runtime, mounted GraphQL, remote-adapter, or parity evidence is claimed.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-query-cart-read-error-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-query-cart-read-error-safety.mjs`

## Still open

- Execute the focused source verifier and Cargo checks.
- Retain mounted success, not-found, validation, unavailable, and invariant GraphQL evidence.
- Continue inventory, tax, promotion, payment execution/compensation, remaining adapter, write-side, and non-`PortError` ecommerce envelope cleanup.

## Intended checks

```bash
node scripts/verify/verify-commerce-graphql-query-cart-read-error-safety.mjs
cargo check -p rustok-commerce --lib
cargo check -p rustok-cart --lib
```

No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, or CI were executed.
