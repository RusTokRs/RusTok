# Order read port

Status: owner port published, unvalidated; Commerce consumer cutover remains open.

## Scope

`OrderReadPort` is the order-owned boundary for complete order projection reads
needed by mounted Commerce REST and GraphQL query consumers. It is separate from:

- `CheckoutCompletionPort`, which owns checkout create/place/replay;
- `CheckoutOrderIdentityPort`, which owns checkout/order identity;
- `CheckoutOrderCompensationPort`, which owns checkout cancellation recovery;
- `CheckoutOrderPaymentSettlementPort`, which owns captured-payment settlement;
- order lifecycle, return, and order-change mutations, which remain on existing
  order-owner commands and services.

This source wave publishes the owner boundary only. It does not claim that
Commerce consumers have already stopped constructing `OrderService`.

## Operations

The owner publishes two read-only operations:

- `read_order_projection` returns one complete `OrderResponse` by order id;
- `list_order_projections` preserves page, per-page, status, customer filtering,
  descending creation ordering, and the owner pagination total through
  `OrderProjectionPage`.

Both operations use `PortContext.locale` as the requested locale and accept an
optional tenant-default fallback locale in the typed request. The existing owner
DTO is returned unchanged; Commerce does not define a partial order projection.

## In-process adapter

`InProcessOrderReadPort` owns concrete `OrderService` construction. The root
factory is:

```rust
in_process_order_read_port(db, event_bus)
```

Every operation:

1. requires `PortCallPolicy::read()`;
2. parses tenant identity from `PortContext`;
3. delegates to the existing locale-aware owner service operation;
4. maps every current `OrderError` variant to stable `PortError` policy;
5. preserves complete owner projections and list totals.

The adapter does not inspect owner error messages for control flow. Validation
maps to `Validation`; missing order/return/change maps to `NotFound`; invalid
transitions map to `Conflict`; database failures map to retryable `Unavailable`;
and owner-core failures fail closed as non-retryable `InvariantViolation`.

## Context and diagnostics

The owner boundary retains operation, correlation id, tenant, actor, channel
length, requested/fallback locale lengths, causation/trace presence, deadline,
order/customer/filter facts, stable owner code, error kind, and retryability.
Only database and core failures retain the typed internal cause in technical
logs. Public `PortError.message` values are stable and do not contain raw storage
or owner-invariant details.

## Current consumer inventory

The following mounted Commerce reads still construct `OrderService` and are not
changed in this wave:

- admin REST order list and detail;
- storefront order detail and ownership checks;
- GraphQL order detail and list;
- return/order-change mutation and query paths that require a wider owner contract.

Admin order detail also aggregates payment and fulfillment projections. Its
order-source cutover should remain separate from payment/fulfillment aggregation
cutover so public envelope compatibility can be reviewed independently.

## Next source slice

Publish `CommerceOrderReadRuntime`, allow a host-selected `Arc<dyn OrderReadPort>`,
compose/cache it in the default application host, require it in
`CommerceHttpRuntime`, and cut admin REST order list/detail over first. GraphQL and
storefront reads should follow in separate atomic changes.

## Evidence

Source evidence is retained at:

`crates/rustok-order/contracts/evidence/order-read-port-source.json`

Its status is `owner_port_published_unvalidated`. Runtime composition, Commerce
consumer cutover, compile evidence, mounted parity, deadline/failure execution,
restart, and remote-adapter evidence remain false or open.

## Intended checks

```bash
node scripts/verify/verify-order-read-port.mjs
cargo check -p rustok-order --lib
```

Tests, Cargo commands, formatting, verifiers, workflow checks, and CI were not run
by the implementation agent.
