# Fulfillment lifecycle read failure contract

Status: source-ready / unexecuted.

## Scope

This contract covers deterministic deadline and typed-failure behavior for
Commerce consumers of the fulfillment-owned `FulfillmentReadPort`. It is
separate from the mounted projection-parity capture, process-restart evidence,
external-adapter identity evidence, and remote-adapter transport evidence.

The source harness is:

`crates/rustok-commerce/tests/fulfillment_read_port_failure_contract.rs`

The locked machine contract is:

`crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-read-failure-execution-contract.json`

## Typed GraphQL boundary

The private `safe_query.rs` compatibility shim now retains a typed query error
instead of reducing every owner `PortError` to the smaller concrete
`FulfillmentError` enum and then to a dynamic string.

The shim has three outcomes:

- `ShippingOptionNotFound` preserves the existing optional shipping-option
  lookup;
- `FulfillmentNotFound` preserves the existing optional fulfillment lookup;
- `Public(BoundaryError)` carries the exact safe message, code, and retryable
  flag selected from `PortErrorKind`.

The included `query.rs` source remains unchanged. Its existing `err.to_string()`
call resolves to the shim error's inherent `to_string() -> BoundaryError`, so
typed extensions survive without parsing an owner message or introducing a
transport-specific fork.

The locked GraphQL matrix is:

| Owner kind | Public code | Retryable |
|---|---|---:|
| `Validation` | `FULFILLMENT_REQUEST_INVALID` | false |
| `Conflict` | `FULFILLMENT_STATE_CONFLICT` | false |
| `Forbidden` | `FULFILLMENT_ACCESS_DENIED` | false |
| `Unavailable` | `FULFILLMENT_TEMPORARILY_UNAVAILABLE` | true |
| `Timeout` | `FULFILLMENT_TEMPORARILY_UNAVAILABLE` | true |
| `InvariantViolation` | `FULFILLMENT_OPERATION_FAILED` | false |

Owner `NotFound` remains GraphQL `null` with no error for fulfillment lookup.

## Admin REST boundary

The same scripted owner port is mounted through public
`CommerceFulfillmentLifecycleReadRuntime` host composition and exercises
`GET /admin/fulfillments/{id}`.

The locked REST matrix is:

| Owner kind | HTTP status | Public code |
|---|---:|---|
| `Validation` | 400 | `commerce_admin_fulfillment_invalid` |
| `NotFound` | 404 | `commerce_admin_not_found` |
| `Conflict` | 409 | `commerce_admin_fulfillment_state_conflict` |
| `Forbidden` | 401 | `commerce_permission_denied` |
| `Unavailable` | 503 | `commerce_admin_fulfillment_storage_unavailable` |
| `Timeout` | 503 | `commerce_admin_fulfillment_storage_unavailable` |
| `InvariantViolation` | 500 | `commerce_admin_fulfillment_failed` |

## Context and redaction assertions

The harness records every `PortContext` received by the scripted owner port and
requires:

- a two-second deadline for GraphQL lookup, filtered list,
  latest-by-order, and admin REST detail;
- exact tenant identity;
- the stable GraphQL service actor
  `rustok-commerce.graphql-query-fulfillments`;
- the authenticated REST user actor;
- resource-scoped GraphQL and REST correlation identifiers;
- normalized `ru-RU` locale on the REST request;
- no retained owner sentinel message in GraphQL or REST response payloads.

The GraphQL lifecycle context currently uses its established compatibility
locale `en`; locale/channel propagation beyond that compatibility contract
remains part of the wider unexecuted tenant/context runtime evidence.

## Execution

Maintainer command:

```text
cargo test -p rustok-commerce --test fulfillment_read_port_failure_contract -- --nocapture
```

Source verifier:

```text
node scripts/verify/verify-fulfillment-lifecycle-read-failure-contract.mjs
```

Neither command was executed by the implementation agent. The published source
harness does not promote `deadline_failure_proven`, `runtime_parity_proven`, or
any restart/remote-adapter evidence flag.
