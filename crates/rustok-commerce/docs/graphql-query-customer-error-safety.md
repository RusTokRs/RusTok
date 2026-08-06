# Commerce GraphQL storefront-customer error safety

Status: `source_closed_unvalidated`

## Scope

This source wave closes the currently identified dynamic customer `PortError` handling gap in the mounted Commerce GraphQL query facade.

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

The transport boundary retains:

- a diagnostic token whose `Debug` output is always `redacted`;
- closed owner kind;
- stable owner code;
- owner retryability;
- owner-message presence and character length;
- selected public code and retryability;
- error severity for unavailable, timeout, and invariant failures;
- warning severity for validation, conflict, forbidden, and other ordinary rejections.

The complete `PortError` and owner-message content are not logged.

Identity-not-found outcomes are already recorded by the Customer owner boundary and preserve their existing auth/optional transport semantics without a duplicate Commerce failure event.

## Preserved contracts

- `CustomerReadPort` and the canonical in-process customer provider are unchanged.
- Customer owner public codes, messages, kinds, retryability, operations, and persistence behavior are unchanged.
- `query.rs` remains unchanged.
- Customer FFA/FBA status is unchanged.
- No runtime, mounted GraphQL, compile, remote-adapter, or parity evidence is claimed.

## Still open

- Execute the focused source verifier and Cargo checks.
- Retain mounted success, missing identity, validation, unavailable, timeout, and invariant GraphQL evidence.
- Remove raw correlation-id payloads from remaining Customer owner diagnostics.
- Continue tax, promotion, inventory, remaining adapter, and non-`PortError` ecommerce envelope cleanup.

## Intended checks

```bash
node scripts/verify/verify-commerce-graphql-query-customer-error-safety.mjs
cargo check -p rustok-commerce --lib
cargo check -p rustok-customer --lib
```

No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, or CI were executed.
