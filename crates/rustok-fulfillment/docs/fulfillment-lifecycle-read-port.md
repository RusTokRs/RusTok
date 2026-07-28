# Fulfillment lifecycle read port

Status: source-ready, unvalidated.

## Scope

`FulfillmentReadPort` is the fulfillment-owned boundary for complete lifecycle
projection reads used by Commerce query consumers. It is separate from:

- `ShippingOptionReadPort` and `ShippingOptionAdminReadPort`, which own
  shipping-option projections;
- `CheckoutFulfillmentExecutionPort`, which owns checkout create/adopt/recovery;
- `ShippingSelectionPort`, which owns seller/cart shipping selection;
- fulfillment lifecycle mutation methods, which remain on `FulfillmentService`.

This slice publishes the owner read boundary and a host-selectable Commerce
runtime container. It does not yet compose that runtime in the default server or
cut GraphQL/admin REST consumers over to the port.

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

## Commerce runtime publication

`CommerceFulfillmentLifecycleReadRuntime` holds an
`Arc<dyn FulfillmentReadPort>` and exposes:

- a public constructor for a host-selected adapter;
- an explicit `in_process` constructor;
- a clone getter for consumer injection.

It remains separate from `CommerceShippingOptionReadRuntime`, allowing different
adapters for lifecycle and shipping-option projections.

Commerce GraphQL runtime-data construction consumes a host-provided lifecycle
runtime when one is already present in `HostRuntimeContext`. Until the default
server and mounted consumers are cut over, it retains an explicit in-process
fallback from `GraphqlRuntimeInputs::db_clone()`.

This fallback is compatibility source, not evidence that the default server has
cached or attached the runtime. `ServerRuntimeContext` composition and
`CommerceHttpRuntime` injection remain open.

## Consumer boundary

The current private Commerce GraphQL fulfillment facade still constructs one
concrete `FulfillmentService` for:

- fulfillment lookup;
- fulfillment list;
- latest fulfillment by order.

Admin REST also still constructs `FulfillmentService` for:

- `GET /admin/fulfillments`;
- `GET /admin/fulfillments/{id}`.

Lifecycle mutations remain intentionally outside this read boundary.

The next cutover slice must compose/cache the runtime in the default application
host, inject it into GraphQL and `CommerceHttpRuntime`, route the GraphQL and REST
read consumers through `FulfillmentReadPort`, preserve each transport's
optional-not-found and public error policy, and remove only the private GraphQL
concrete delegate. No mounted consumer is claimed in this slice.

## Diagnostics

The owner boundary records operation, correlation id, tenant, actor, channel
length, locale length, causation/trace presence, deadline, relevant fulfillment,
order, customer, and status facts, stable owner code, error kind, and
retryability. Only technical database events retain the typed internal cause.

## Evidence

Source evidence is retained at:

`crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-read-port-source.json`

Its status is `source_ready_unvalidated`. It explicitly records that default
server composition, GraphQL/admin REST consumer cutover, and private concrete
delegate removal remain incomplete.

## Intended checks

```bash
node scripts/verify/verify-fulfillment-lifecycle-read-port.mjs
cargo check -p rustok-fulfillment --lib
cargo check -p rustok-commerce --lib
```

Tests, Cargo commands, formatting, verifiers, workflows, and CI were not run by
the implementation agent.
