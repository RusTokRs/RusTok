# Commerce GraphQL storefront-customer error safety

Status: `source_closed_unvalidated`

## Scope

This source contract closes the currently identified dynamic customer `PortError` handling gap in the mounted Commerce GraphQL query facade and records the correlation-safe Customer read diagnostics consumed by that facade.

The compatibility resolver source in `crates/rustok-commerce/src/graphql/query.rs` remains unchanged. Its three customer-by-user reads continue to serve:

- `storefrontMe`;
- storefront order ownership validation used by order, return, refund, and order-change reads;
- optional customer identity resolution used by storefront shipping and cart-access paths.

The existing customer owner call, `CustomerUserProjectionRequest`, customer `PortContext`, two-second deadline, authenticated actor, successful `CustomerResponse`, GraphQL fields, DTOs, and downstream ownership checks are unchanged.

## Typed shim

The mounted safe-query source aliases only the customer dependency used by the unchanged compatibility query. The shim wraps the canonical `in_process_customer_read_port` and delegates `read_customer_projection_by_user` to the same `CustomerReadPort`.

The legacy resolver still compares a compatibility code to retain established identity-absence behavior. The shim does not copy or inspect `PortError.code` for that decision. It derives the compatibility sentinel only from `PortErrorKind::NotFound`:

- auth-required reads still map missing customer identity to `unauthenticated`;
- optional identity resolution still maps missing customer identity to `None`;
- all other kinds preserve the complete typed `PortError` for the transport mapper.

Owner code strings are therefore no longer a control-flow boundary in the mounted GraphQL path.

## Public envelopes

Non-identity failures are classified only by `PortErrorKind`.

| Owner kind | Public code | Public message | Retryable |
| --- | --- | --- | --- |
| Validation | `CUSTOMER_REQUEST_INVALID` | `Customer query is invalid` | false |
| NotFound | `CUSTOMER_RESOURCE_NOT_FOUND` | `Customer data was not found` | false |
| Conflict | `CUSTOMER_STATE_CONFLICT` | `Customer state conflicts with this query` | false |
| Forbidden | `CUSTOMER_ACCESS_DENIED` | `Customer query is not permitted` | false |
| Unavailable / Timeout | `CUSTOMER_TEMPORARILY_UNAVAILABLE` | `Customer data is temporarily unavailable` | true |
| InvariantViolation | `CUSTOMER_OPERATION_FAILED` | `Customer query could not be completed safely` | false |

The complete owner `PortError`, owner message content, owner code, and owner retryability are not copied into the GraphQL response.

## Bounded diagnostics

The Commerce transport boundary retains a redacted diagnostic token, closed owner kind, stable owner code, owner retryability, owner-message presence/length, selected public code, and retryability. The complete `PortError` and owner-message content are not logged.

The Customer owner and canonical in-process read layers now retain correlation-ID character length instead of the raw correlation ID across policy admission, list validation, tenant parsing, owner failure, and local delegated-outcome events. Other context and request values remain represented only through bounded presence, length, count, and closed-label facts.

Identity-not-found outcomes preserve their existing auth/optional transport semantics without a duplicate Commerce failure event.

## Preserved contracts

- `CustomerReadPort` and the canonical in-process customer provider are unchanged.
- Customer owner public codes, messages, kinds, retryability, operations, and persistence behavior are unchanged.
- `query.rs` remains unchanged.
- Customer FFA/FBA status is unchanged.
- No runtime, mounted GraphQL, compile, remote-adapter, or parity evidence is claimed.

## Still open

- Execute the focused source verifiers and Cargo checks.
- Retain mounted success, missing identity, validation, unavailable, timeout, and invariant GraphQL evidence.
- Continue tax, promotion, inventory, remaining adapter, write-side Customer, and non-`PortError` ecommerce envelope cleanup.

## Intended checks

```bash
node scripts/verify/verify-commerce-graphql-query-customer-error-safety.mjs
node scripts/verify/verify-customer-owner-error-diagnostic-safety.mjs
node scripts/verify/verify-customer-read-policy-context.mjs
node scripts/verify/verify-customer-read-local-context.mjs
cargo check -p rustok-commerce --lib
cargo check -p rustok-customer --lib
```

No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, or CI were executed.
