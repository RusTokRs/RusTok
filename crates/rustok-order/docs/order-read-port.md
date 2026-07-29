# Order read port

Status: owner port and host runtime published; admin REST, mounted GraphQL, and storefront HTTP detail/ownership cut over, unvalidated.

## Scope

`OrderReadPort` is the order-owned boundary for complete order projection reads
needed by mounted Commerce REST and GraphQL query consumers. It is separate from:

- `CheckoutCompletionPort`, which owns checkout create/place/replay;
- `CheckoutOrderIdentityPort`, which owns checkout/order identity;
- `CheckoutOrderCompensationPort`, which owns checkout cancellation recovery;
- `CheckoutOrderPaymentSettlementPort`, which owns captured-payment settlement;
- order lifecycle, return, and order-change mutations, which remain on existing
  order-owner commands and services.

The owner boundary and host-selected runtime are now published. Mounted admin REST,
GraphQL order list/detail, and storefront HTTP order detail/ownership reads use the
port. Current complete detail/list/ownership projection consumers are cut over in
source; runtime evidence remains unvalidated.

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

## Admin REST cutover

`GET /admin/orders` calls `list_order_projections` and preserves:

- `orders:list` authorization;
- page and per-page values;
- status and customer filters;
- requested locale and tenant-default fallback;
- descending owner ordering and owner pagination total;
- the existing `PaginatedResponse<OrderResponse>` envelope.

`GET /admin/orders/{id}` calls `read_order_projection` and preserves:

- `orders:read` authorization;
- requested locale and tenant-default fallback;
- the complete `OrderResponse` projection;
- the existing `AdminOrderDetailResponse` envelope;
- the existing payment-collection and fulfillment aggregation lookups.

Both handlers create a two-second read `PortContext` with tenant identity,
authenticated user actor, request locale, optional request channel, and a
resource-scoped correlation id.

`PortErrorKind` is mapped back to the established admin HTTP policy:

- validation -> `400 commerce_admin_order_invalid`;
- not found -> `404 commerce_admin_not_found`;
- conflict -> `409 commerce_admin_order_state_conflict`;
- forbidden -> `401 commerce_permission_denied`;
- unavailable/timeout -> `503 commerce_admin_order_storage_unavailable`;
- invariant violation -> `500 commerce_admin_order_failed`.

The mapper logs stable internal code, retryability, owner operation, correlation,
actor/channel/locale/deadline context, and route identities without copying owner
messages into public envelopes.

## Storefront HTTP cutover

`GET /store/orders/{id}` and the shared customer-ownership check now call
`read_order_projection` through `CommerceHttpRuntime.order_read_port()`.

The cutover preserves:

- authenticated customer resolution before order access;
- complete `OrderResponse` detail and tenant-default locale fallback;
- exact customer-id ownership comparison;
- the existing customer-required and access-denied public envelopes;
- the existing order validation, not-found, conflict, unavailable, and fail-closed
  public codes;
- a two-second read deadline, authenticated user actor, request locale, optional
  host-resolved channel, and resource-scoped correlation id.

The ownership helper protects storefront return creation/listing, refund listing,
and order-change listing without constructing `OrderService` for the ownership
projection. The subsequent return mutation/list, refund list, and order-change list
remain on their existing concrete owner services until wider typed contracts are
published.

## Unchanged behavior

This source wave deliberately leaves these paths unchanged:

- admin order mark-paid, ship, deliver, and cancel mutations still construct
  `OrderService`;
- admin order detail payment lookup still constructs `PaymentService`;
- admin order detail fulfillment lookup still constructs `FulfillmentService`;
- storefront return creation/listing and order-change listing still construct
  `OrderService` after typed ownership validation;
- storefront refund listing still constructs `PaymentService` after typed ownership
  validation;
- return and order-change paths still require wider owner contracts.

Keeping payment/fulfillment aggregation and mutations out of this cutover avoids
claiming ownership changes beyond complete order projection reads.

## Context and diagnostics

The owner boundary retains operation, correlation id, tenant, actor, channel
length, requested/fallback locale lengths, causation/trace presence, deadline,
order/customer/filter facts, stable owner code, error kind, and retryability.
Only database and core failures retain the typed internal cause in technical
logs. Public `PortError.message` values are stable and do not contain raw storage
or owner-invariant details.

## Remaining source work

1. publish wider owner contracts before moving return/order-change reads or order
   mutations;
2. retain compile, mounted parity, deadline/failure, restart, and remote-adapter
   evidence before any status promotion.

## Evidence

Source evidence is retained at:

`crates/rustok-order/contracts/evidence/order-read-port-source.json`

Its status is `storefront_http_cutover_unvalidated`. Host composition, admin REST,
mounted GraphQL with actor/channel context, and storefront HTTP detail/ownership
source cutover are recorded as complete. Compile evidence, mounted parity,
deadline/failure execution, restart, and remote-adapter evidence remain false or
open.

## Intended checks

```bash
node scripts/verify/verify-order-read-port.mjs
node scripts/verify/verify-commerce-graphql-order-read-shim.mjs
node scripts/verify/verify-commerce-storefront-order-read-cutover.mjs
node scripts/verify/verify-commerce-admin-order-route-error-context.mjs
cargo check -p rustok-order --lib
cargo check -p rustok-commerce --lib
cargo check -p rustok-server --features mod-commerce
```

Tests, Cargo commands, formatting, verifiers, workflow checks, and CI were not run
by the implementation agent.
