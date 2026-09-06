# Commerce GraphQL order-change apply owner-port cutover

Status: `source_complete_unvalidated`

Date: 2026-08-10

## Scope

This slice cuts the mounted GraphQL `applyOrderChange` mutation away from the
`OrderChangeOrchestrationService::apply_order_change` compatibility entrypoint that
constructs `OrderService` internally.

The mounted mutation now uses the existing host-composed Order capabilities already
present in `CommerceGraphqlRuntimeData`:

- `CommerceOrderReadRuntime` / `OrderReadPort` for the order-change projection read;
- `OrderPostOrderCommandRuntime` / `OrderPostOrderCommandPort::apply_change` for the
  ordinary/default apply transition.

No new owner capability was required. The Order read and apply capabilities were
already published and the in-process adapters preserve the canonical owner-local
`OrderService::get_order_change` and `OrderService::apply_order_change` semantics.

## Mounted GraphQL composition

`order_change_orchestration_from_context` now selects `from_order_ports` whenever
`CommerceGraphqlRuntimeData` is present and injects:

- `runtime.order_read_runtime().order_read_port()`;
- `runtime.order_post_order_command_runtime().command_port()`;
- the existing host-selected Payment provider registry used by exchange/refund
  orchestration.

The mutation builds an authenticated read `PortContext` with tenant, user actor,
request locale/channel, correlation identity, and a two-second deadline. The write
context reuses the existing GraphQL post-order command context builder.

The generated write idempotency UUID is admission metadata only. The legacy GraphQL
mutation does not expose a caller idempotency key, so this slice does not claim
replay/exactly-once semantics for repeated client requests.

## Dispatch and cross-domain behavior

Commerce still owns order-change type dispatch inside
`OrderChangeOrchestrationService`. The mounted GraphQL transport does not inspect
`change_type`.

- the ordinary/default path reads through `OrderReadPort` and applies through
  `OrderPostOrderCommandPort::apply_change`;
- `exchange` still delegates to the existing exchange post-order orchestration;
- `claim` still delegates to the existing claim post-order orchestration.

This slice intentionally does not decompose the remaining exchange/claim internals.

## Error boundary

Owner read/command `PortError` values are mapped through the existing stable Order
GraphQL public envelope policy. Diagnostics retain only bounded facts such as owner
error kind, owner code length, correlation identity, operation names, public code,
and retryability. The new owner-port mapper does not log raw `PortError.message` or
stringify the owner error.

`OrderChangeOrchestrationError::PostOrder` continues through the existing post-order
GraphQL error mapper so exchange/claim Payment behavior and public envelopes remain
unchanged by this slice.

## Compatibility

`OrderChangeOrchestrationService::new` and the legacy `apply_order_change` method
remain for directly embedded schemas and other compatibility consumers that do not
install `CommerceGraphqlRuntimeData`. Mounted application GraphQL composition already
requires the Order read and post-order command runtimes, so the production resolver
uses host-selected adapters.

This is compatibility source, not a claim that all private legacy orchestration can
be removed yet.

## Plan status

The broad topology P0 remains open. This slice closes only mounted GraphQL
`applyOrderChange` order-owned read/default-apply construction; other mounted
Product, Order, Payment, or Fulfillment construction and deeper Commerce orchestration
must still be audited independently.

## Validation

No tests, Cargo commands, Node verifiers, formatter, GraphQL scenarios, CI/workflows,
database scenarios, provider calls, restart scenarios, or remote-adapter scenarios
were executed in this slice. The source guard was added but intentionally not run.
