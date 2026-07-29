# Order read port

Status: owner port and host runtime published; complete order projections plus storefront return/change lists cut over, unvalidated.

## Scope

`OrderReadPort` is the order-owned boundary for complete order, return, and
order-change projection reads needed by mounted Commerce REST and GraphQL query
consumers. It is separate from:

- `CheckoutCompletionPort`, which owns checkout create/place/replay;
- `CheckoutOrderIdentityPort`, which owns checkout/order identity;
- `CheckoutOrderCompensationPort`, which owns checkout cancellation recovery;
- `CheckoutOrderPaymentSettlementPort`, which owns captured-payment settlement;
- order lifecycle, return, and order-change mutations, which remain on existing
  order-owner commands and services.

The owner boundary and host-selected runtime are published. Mounted admin REST and
GraphQL complete order list/detail, storefront HTTP order detail/ownership, and
storefront return/order-change lists use the port. GraphQL and admin post-order
reads remain open; runtime evidence remains unvalidated.

## Operations

The owner publishes six read-only operations:

- `read_order_projection` returns one complete `OrderResponse` by order id;
- `list_order_projections` preserves page, per-page, status, customer filtering,
  descending creation ordering, and owner pagination total;
- `read_order_return_projection` returns one `OrderReturnResponse` by return id;
- `list_order_return_projections` preserves page, per-page, order-id/status filters,
  descending creation ordering, return items, and owner pagination total;
- `read_order_change_projection` returns one `OrderChangeResponse` by change id;
- `list_order_change_projections` preserves page, per-page, order-id/status/type
  filters, descending creation ordering, and owner pagination total.

Complete order operations use `PortContext.locale` as requested locale and accept
an optional tenant-default fallback locale. Return and order-change DTOs are not
localized in the current owner model, so their typed requests do not invent locale
fields. All operations preserve existing owner DTOs rather than defining partial
Commerce projections.

## In-process adapter

`InProcessOrderReadPort` owns concrete `OrderService` construction. The root
factory is:

```rust
in_process_order_read_port(db, event_bus)
```

Every operation:

1. requires `PortCallPolicy::read()`;
2. parses tenant identity from `PortContext`;
3. delegates to the existing owner service operation;
4. maps every current `OrderError` variant to stable `PortError` policy;
5. preserves complete owner projections, filters, ordering, and list totals.

The adapter does not inspect owner error messages for control flow. Validation
maps to `Validation`; missing order/return/change maps to `NotFound`; invalid
transitions map to `Conflict`; database failures map to retryable `Unavailable`;
and owner-core failures fail closed as non-retryable `InvariantViolation`.

## Commerce runtime composition

`CommerceOrderReadRuntime` contains one host-selected `Arc<dyn OrderReadPort>` and
exposes:

- `new` for an externally selected adapter;
- `in_process` for the deterministic local adapter;
- `order_read_port` for consumer injection.

The default application host:

1. reuses an existing `CommerceOrderReadRuntime` from `ServerRuntimeContext`;
2. otherwise obtains the shared `TransactionalEventBus` and builds the in-process
   runtime once;
3. caches the runtime in `ServerRuntimeContext`;
4. attaches the same value to `HostRuntimeContext`.

`CommerceHttpRuntime` requires this value. Commerce GraphQL schema-data composition
also requires it. The mounted resolver extension scopes the same runtime into the
safe-query compatibility facade.

The resolver extension separately scopes request-owned order context:

- authenticated actors come only from validated `AuthContext` data;
- unauthenticated reads use the stable `rustok-commerce.graphql-order-query`
  service actor;
- the channel is the host-resolved `RequestContext.channel_slug`;
- directly embedded schemas without the mounted extension use the service actor
  and no channel rather than inventing attribution.

## Complete order consumer cutover

Admin REST order list/detail, mounted GraphQL order list/detail, storefront order
detail, and the shared storefront customer-ownership check use the host-selected
port. Existing authorization, locale fallback, filters, pagination, detail
aggregation, public envelopes, authenticated actor, resolved channel, correlation,
and two-second deadline behavior are preserved.

## Storefront post-order read cutover

`GET /store/orders/{id}/returns` calls `list_order_return_projections` after the
shared typed ownership check. It preserves:

- customer resolution and exact order ownership validation;
- page/per-page values and owner total;
- the order-id and optional status filters;
- descending owner ordering and complete return items;
- the existing `PaginatedResponse<OrderReturnResponse>` envelope.

`GET /store/orders/{id}/changes` calls `list_order_change_projections` after the
same ownership check. It preserves:

- page/per-page values and owner total;
- order-id, optional status, and optional change-type filters;
- descending owner ordering;
- the existing `PaginatedResponse<OrderChangeResponse>` envelope.

Both handlers use the same validated user actor, resolved channel, request locale,
resource-scoped correlation id, two-second deadline, and stable storefront public
error policy as complete order reads.

## Unchanged behavior

This source wave deliberately leaves these paths unchanged:

- storefront return creation still constructs `OrderService` after typed ownership
  validation;
- storefront refund listing still constructs `PaymentService` after typed ownership
  validation;
- GraphQL return/order-change detail and list reads still use the compatibility
  facade's concrete `OrderService` path;
- admin return/order-change reads and all order/return/change mutations remain on
  their current owner services;
- admin order detail payment and fulfillment aggregation remain unchanged.

No mutation, payment, or fulfillment policy moved into the read port.

## Context and diagnostics

The owner boundary retains operation, correlation id, tenant, actor, channel
length, requested/fallback locale lengths, causation/trace presence, deadline,
order/return/change/customer/filter facts, stable owner code, error kind, and
retryability. Only database and core failures retain the typed internal cause in
technical logs. Public `PortError.message` values are stable and do not contain raw
storage or owner-invariant details.

## Remaining source work

1. cut GraphQL return/order-change detail and list reads to the host-selected port;
2. audit and cut admin post-order reads separately without moving mutations;
3. retain compile, mounted parity, deadline/failure, restart, and remote-adapter
   evidence before status promotion.

## Evidence

Source evidence is retained at:

`crates/rustok-order/contracts/evidence/order-read-port-source.json`

Its status is `storefront_post_order_reads_cutover_unvalidated`. Six owner read
operations, host composition, complete order consumers, and storefront post-order
list consumers are recorded as source complete. GraphQL/admin post-order consumer
cutover, compile evidence, mounted parity, deadline/failure execution, restart, and
remote-adapter evidence remain false or open.

## Intended checks

```bash
node scripts/verify/verify-order-read-port.mjs
node scripts/verify/verify-commerce-graphql-order-read-shim.mjs
node scripts/verify/verify-commerce-storefront-order-read-cutover.mjs
node scripts/verify/verify-commerce-storefront-post-order-read-cutover.mjs
node scripts/verify/verify-commerce-admin-order-route-error-context.mjs
cargo check -p rustok-order --lib
cargo check -p rustok-commerce --lib
cargo check -p rustok-server --features mod-commerce
```

Tests, Cargo commands, formatting, verifiers, workflow checks, and CI were not run
by the implementation agent.
