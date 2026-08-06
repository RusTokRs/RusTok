# Commerce GraphQL payment query error safety

Status: `source_closed_unvalidated`

## Scope

This source wave closes the identified typed-error loss in mounted Commerce GraphQL payment collection and refund reads.

The compatibility resolver source in `crates/rustok-commerce/src/graphql/query.rs` remains unchanged. Its existing `PaymentService` calls still perform the same reads and return the same successful payment collection and refund DTOs. Existing `PaymentCollectionNotFound` and `RefundNotFound` branches continue to return `None` where the GraphQL contract already requires that behavior.

## Typed facade

The mounted safe-query source aliases only the Payment dependency used by the unchanged compatibility query. The facade wraps the canonical `rustok_payment::PaymentService` and delegates these owner methods without changing arguments or successful projections:

- `find_reusable_collection_by_cart`;
- `find_latest_collection_by_order`;
- `get_collection`;
- `list_collections`;
- `get_refund`;
- `list_refunds`.

The original resolver converts several failures through `err.to_string()`. The facade preserves those expressions through an inherent typed conversion that returns the safe GraphQL boundary value. It does not format the Payment owner error into a public string.

## Public envelopes

Transport responses are classified structurally by `rustok_payment::error::PaymentError` variant.

| Owner variant | Public code | Public message | Retryable |
| --- | --- | --- | --- |
| Validation | `PAYMENT_REQUEST_INVALID` | `Payment query is invalid` | false |
| Collection/payment/refund not found | `PAYMENT_RESOURCE_NOT_FOUND` | `Payment resource was not found` | false |
| Invalid transition/provider rejection | `PAYMENT_STATE_CONFLICT` | `Payment state conflicts with this query` | false |
| Provider unavailable/database | `PAYMENT_TEMPORARILY_UNAVAILABLE` | `Payment data is temporarily unavailable` | true |
| Invalid/unknown provider outcome | `PAYMENT_RECONCILIATION_REQUIRED` | `Payment state requires reconciliation` | false |
| Provider configuration | `PAYMENT_CONFIGURATION_ERROR` | `Payment provider configuration is invalid` | false |

Validation text, provider identifiers and operation text, lifecycle values, database causes, and complete owner errors are not copied into GraphQL responses.

## Bounded diagnostics

The facade records only:

- a diagnostic token whose `Debug` output is always `redacted`;
- canonical owner and owner operation;
- a stable Commerce GraphQL correlation token derived from the operation and owner resource identity;
- tenant identity, resource kind, and closed UUID shape;
- structural error kind;
- closed owner-detail shape and aggregate character length;
- selected public code and retryability;
- error severity for technical, configuration, and reconciliation failures;
- warning severity for validation, not-found, and conflict rejections.

The complete `PaymentError`, provider identifiers, provider operation text, validation text, lifecycle values, and database error are not logged.

## Preserved contracts

- `rustok-payment` remains the owner of payment collection and refund persistence and read behavior.
- The canonical owner service and six read methods are preserved.
- The query source, GraphQL fields, authorization checks, filters, pagination, result ordering, and DTO conversions are unchanged.
- Commerce and Payment FFA/FBA status is unchanged.
- The broad ecommerce mapper and public-envelope cleanup remains open.
- No compile, runtime, mounted GraphQL, remote-adapter, or parity evidence is claimed.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-query-payment-error-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-query-payment-error-safety.mjs`

## Still open

- Execute the focused source verifier and Cargo checks.
- Retain mounted payment collection/refund success, not-found, validation, database, and reconciliation GraphQL evidence.
- Continue order execution/compensation, inventory, tax, promotion, remaining adapter, write-side, and non-`PortError` ecommerce envelope cleanup.

## Intended checks

```bash
node scripts/verify/verify-commerce-graphql-query-payment-error-safety.mjs
cargo check -p rustok-commerce --lib
cargo check -p rustok-payment --lib
```

No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, or CI were executed.
