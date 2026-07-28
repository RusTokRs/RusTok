# Fulfillment lifecycle read port

Status: source cutover, unvalidated.

## Scope

`FulfillmentReadPort` is the fulfillment-owned boundary for complete lifecycle
projection reads used by Commerce query consumers. It is separate from:

- `ShippingOptionReadPort` and `ShippingOptionAdminReadPort`, which own
  shipping-option projections;
- `CheckoutFulfillmentExecutionPort`, which owns checkout create/adopt/recovery;
- `ShippingSelectionPort`, which owns seller/cart shipping selection;
- fulfillment lifecycle mutation methods, which remain on `FulfillmentService`.

The owner read boundary, host-selected runtime, GraphQL compatibility facade, and
admin REST list/detail consumers now use one typed owner port. No mounted Commerce
lifecycle read constructs the concrete owner service.

## Operations

The owner publishes three read-only operations:

- `read_fulfillment_projection` returns one `FulfillmentResponse` by fulfillment
  id and reports absence as owner `NotFound`;
- `list_fulfillment_projections` preserves page, per-page, status, order, and
  customer filters and returns `FulfillmentProjectionPage`;
- `find_latest_fulfillment_by_order_projection` preserves the current
  latest-created fulfillment lookup and returns `Option<FulfillmentResponse>`.

The existing owner DTO is returned unchanged. Commerce does not define a partial
copy of fulfillment lifecycle state.

## In-process adapter

`InProcessFulfillmentReadPort` owns concrete `FulfillmentService` construction.
The root factory is:

```rust
in_process_fulfillment_read_port(db)
```

Every operation:

1. requires `PortCallPolicy::read()`;
2. parses tenant identity from `PortContext`;
3. delegates to the existing fulfillment owner service;
4. maps every current `FulfillmentError` variant to a stable `PortError`;
5. preserves the owner projection and pagination total on success.

The adapter does not parse, match, or expose owner error messages as control
flow. Database failures are retryable `Unavailable`; validation, not-found, and
conflict outcomes retain stable owner codes.

## Commerce runtime composition

`CommerceFulfillmentLifecycleReadRuntime` holds an
`Arc<dyn FulfillmentReadPort>` and exposes:

- a public constructor for a host-selected adapter;
- an explicit `in_process` constructor;
- a clone getter for consumer injection.

It remains separate from `CommerceShippingOptionReadRuntime`, allowing different
adapters for lifecycle and shipping-option projections.

The application server:

1. reuses a lifecycle runtime already installed in `ServerRuntimeContext`;
2. otherwise constructs the deterministic in-process runtime once;
3. caches that runtime in `ServerRuntimeContext`;
4. attaches the same typed value to `HostRuntimeContext`.

This preserves external adapter selection and gives GraphQL and HTTP composition
the same owner-port instance.

Commerce GraphQL runtime-data construction consumes the host-provided runtime and
retains an explicit in-process fallback for directly embedded compatibility
schemas.

## GraphQL cutover

The existing mounted `CommerceShippingOptionReadScope` now scopes both read
runtimes for each resolver task:

- `CommerceShippingOptionReadRuntime`;
- `CommerceFulfillmentLifecycleReadRuntime`.

The private compatibility facade resolves both runtimes from the same task-local
bridge. Its lifecycle field is now `Arc<dyn FulfillmentReadPort>`; the previous
concrete `FulfillmentService` field and constructor are removed.

The unchanged `query.rs` source keeps its public compatibility behavior:

- fulfillment lookup calls `read_fulfillment_projection` and still converts owner
  not-found to `None`;
- fulfillment list calls `list_fulfillment_projections`, preserving filters,
  page/per-page values, owner total, and `GqlFulfillmentList` metadata;
- admin order detail calls `find_latest_fulfillment_by_order_projection`,
  preserving its optional latest fulfillment.

The facade constructs a two-second read `PortContext` with tenant identity, a
stable Commerce GraphQL service actor, and a resource-scoped correlation id.
Typed `PortErrorKind` values are adapted back to the established compatibility
error classes without parsing or exposing owner messages.

## Admin REST cutover

`GET /admin/fulfillments` calls `list_fulfillment_projections` and preserves:

- page and per-page values;
- status, order, and customer filters;
- the owner pagination total;
- the existing `PaginatedResponse<FulfillmentResponse>` envelope.

`GET /admin/fulfillments/{id}` calls `read_fulfillment_projection` and keeps the
existing detail envelope and not-found policy.

Both handlers construct `PortContext` with tenant identity, authenticated user
actor, request locale, optional request channel, a resource-scoped correlation
id, and a two-second deadline. `PortErrorKind` maps to the existing public
validation, not-found, conflict, permission, unavailable, and safe-failure HTTP
policies without copying owner messages.

Admin lifecycle create/ship/deliver/reopen/reship/cancel paths remain on their
existing concrete or orchestration services. This read cutover does not change
mutation ownership or behavior.

## Diagnostics

The owner boundary records operation, correlation id, tenant, actor, channel
length, locale length, causation/trace presence, deadline, relevant fulfillment,
order, customer, and status facts, stable owner code, error kind, and
retryability. Only technical database events retain the typed internal cause.

The GraphQL and admin REST boundaries additionally retain transport operation,
resource identity, public policy code, retryability, and deadline context.

## Evidence

Source evidence is retained at:

`crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-read-port-source.json`

Its status is `source_cutover_unvalidated`. It records completed default server
composition, GraphQL lifecycle lookup/list/latest-by-order cutover, admin REST
list/detail cutover, and concrete GraphQL delegate removal. Runtime parity remains
explicitly unproven.

## Intended checks

```bash
node scripts/verify/verify-fulfillment-lifecycle-read-port.mjs
node scripts/verify/verify-commerce-graphql-query-fulfillment-context.mjs
cargo check -p rustok-fulfillment --lib
cargo check -p rustok-commerce --lib
cargo check -p rustok-server --features mod-commerce
```

Tests, Cargo commands, formatting, verifiers, workflows, and CI were not run by
the implementation agent.
