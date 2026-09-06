# GraphQL customer diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the customer-owner error mapper used by the Commerce GraphQL storefront cart helpers.

The mapper already selected stable GraphQL messages, codes, retryability, and error/warn severity from `PortErrorKind`. Its diagnostic events still emitted the complete `PortError`, internal message, tenant identity, actor identity, channel, locale, correlation value, causation value, traceparent, and idempotency key.

The constructed storefront customer correlation value includes the user UUID, so logging it verbatim would also disclose the actor identity.

## Bounded diagnostic projection

The mapper now preserves the original `PortError` only until public policy, severity, and safe owner facts are selected. It then shadows the error with a diagnostic type whose `Debug` output is always `redacted`.

The diagnostic events retain:

- truthful customer owner and owner operation;
- consumer operation;
- exact owner code, typed owner kind, and owner retryability;
- owner-message `empty` / `present` shape and byte length;
- tenant and actor identity shape without values;
- actor kind;
- claim and role counts;
- channel, locale, correlation, causation, traceparent, and idempotency presence shapes;
- deadline value;
- public code, public retryability, shared boundary, and static event message.

Identity strings are classified as `empty`, `uuid_nil`, `uuid_non_nil`, or `opaque`. Optional text is classified as `absent`, `empty`, or `present`.

## Preserved GraphQL behavior

This work does not change:

- the customer owner call or request;
- the two-second read deadline;
- anonymous-customer behavior;
- the owner-specific `customer.customer_by_user_not_found` fallback to `None`;
- public messages and codes for validation, not-found, conflict, forbidden, unavailable/timeout, and invariant failures;
- retryability values;
- error severity for unavailable, timeout, and invariant failures;
- warning severity for ordinary owner rejections.

## Remaining helper work

This slice does not close the complete storefront cart helper diagnostic boundary.

Still open:

- `cart_port_error`, which logs the complete Cart/Pricing `PortError`;
- `legacy_graphql_error`, which logs the complete GraphQL error and raw tenant/resource UUIDs;
- typed line-item diagnostics, which still log typed source payloads and raw product/variant/tenant context;
- remaining storefront, tax, promotion, native transport, and owner-adapter cleanup.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-customer-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted GraphQL scenarios, workflows, or CI were run. No compile, runtime, FFA, or FBA status is promoted.
