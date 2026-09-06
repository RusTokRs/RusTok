# Shipping-option read port

Status: source-cutover-ready, unvalidated.

## Scope

`ShippingOptionReadPort` and `ShippingOptionAdminReadPort` are fulfillment-owned
read boundaries for complete shipping-option projections consumed by mounted
Commerce transports.

They remain separate from `ShippingSelectionPort`:

- `ShippingSelectionPort` owns seller/cart selection workflow;
- `ShippingOptionReadPort` owns storefront active-list and single-option reads;
- `ShippingOptionAdminReadPort` owns the complete administrative list-all read;
- the selection provider registry is unchanged;
- no new FBA provider contract or status promotion is claimed.

## Operations

The ports publish three read-only operations:

- `list_shipping_option_projections` returns the active storefront list;
- `list_all_shipping_option_projections` returns the complete administrative list,
  including inactive options;
- `read_shipping_option_projection` returns one complete owner projection.

All operations return the existing fulfillment-owned `ShippingOptionResponse`,
including metadata, allowed shipping-profile slugs, active state, localization
facts, provider identity, and price/currency data.

## Requests and context

`ListShippingOptionProjectionsRequest` and
`ListAllShippingOptionProjectionsRequest` carry requested locale and tenant
default locale. `ReadShippingOptionProjectionRequest` additionally carries the
shipping-option UUID.

`PortContext` independently carries tenant, actor, channel, locale, correlation,
causation, trace, idempotency, and deadline facts. Every read operation requires
read policy and therefore a non-zero deadline.

## In-process adapters

`InProcessShippingOptionReadPort` and
`InProcessShippingOptionAdminReadPort` own concrete `FulfillmentService`
construction and are exported through the canonical root factories:

```rust
in_process_shipping_option_read_port(db)
in_process_shipping_option_admin_read_port(db)
```

The adapters:

1. require read policy;
2. parse tenant identity from `PortContext`;
3. delegate active list, administrative list-all, or lookup to the owner service;
4. preserve requested/default locale arguments;
5. map every current `FulfillmentError` variant to stable `PortError` values;
6. return the owner projection unchanged on success.

The active-list and admin list-all contracts remain separate traits. This keeps
storefront active-only semantics distinct from the admin requirement to inspect
inactive options.

## Application-host composition

The mounted server composes one `CommerceShippingOptionReadRuntime` from the two
root factories, caches it in `ServerRuntimeContext`, and attaches it to
`HostRuntimeContext`. Existing host-installed implementations are preserved.

Mounted GraphQL requires this runtime through manifest data and bridges it into a
resolver-scoped Tokio task-local. The private GraphQL compatibility facade clones
the two owner ports and does not construct providers.

Mounted Commerce HTTP requires the same runtime while building
`CommerceHttpRuntime`. HTTP router construction fails closed when the typed
runtime is missing. Storefront and admin REST handlers clone the ports from HTTP
state and do not construct a concrete fulfillment service for projection reads.

Directly embedded GraphQL schemas retain the explicit
`CommerceShippingOptionReadRuntime::in_process` fallback outside the private
facade. That compatibility path is not mounted runtime parity evidence.

## Mounted Commerce source cutover

GraphQL shipping-option paths use the owner ports for validation, enrichment,
storefront listing, single lookup, and administrative list-all.

Commerce REST now uses the same host-composed runtime for:

- storefront `GET /store/shipping-options` through
  `list_shipping_option_projections`;
- admin `GET /admin/shipping-options` through
  `list_all_shipping_option_projections`;
- admin `GET /admin/shipping-options/{id}` through
  `read_shipping_option_projection`.

Admin create, update, deactivate, and reactivate remain lifecycle mutations over
`FulfillmentService`; this read cutover does not change them.

### REST context

Storefront list uses:

- authenticated user actor when available, otherwise service actor
  `rustok-commerce.storefront-shipping-options`;
- request locale;
- effective public channel, including the cart-derived channel when a cart is
  supplied;
- cart or tenant scoped correlation id;
- two-second deadline.

Admin list and lookup use:

- authenticated user actor;
- request locale;
- request channel when available;
- tenant or shipping-option scoped correlation id;
- two-second deadline.

Requested and tenant-default locales remain explicit owner request fields.

### Preserved success semantics

Storefront owner projections remain active-only. Commerce then preserves its
existing currency, public-channel metadata visibility, and shipping-profile
compatibility filters.

Administrative list-all receives inactive and active options before Commerce
applies active, currency, provider, search, and pagination filters. Single lookup
continues returning the existing successful REST DTO.

### Preserved HTTP policy

REST maps `PortErrorKind` directly to fixed Commerce-owned HTTP envelopes. It does
not parse, match, or expose `PortError.message`.

Storefront preserves `commerce_store_shipping_invalid`,
`commerce_store_not_found`, `commerce_store_shipping_state_conflict`,
`commerce_store_denied`, `commerce_store_shipping_unavailable`, and
`commerce_store_shipping_failed` policies.

Admin preserves `commerce_admin_fulfillment_invalid`,
`commerce_admin_not_found`, `commerce_admin_fulfillment_state_conflict`,
`commerce_permission_denied`,
`commerce_admin_fulfillment_storage_unavailable`, and the fail-closed
`commerce_admin_fulfillment_failed` policy.

## Stable owner errors

| Owner outcome | Port kind | Code | Retryable |
| --- | --- | --- | --- |
| Invalid request | Validation | `fulfillment.validation` | false |
| Shipping option absent | NotFound | `fulfillment.shipping_option_not_found` | false |
| Fulfillment absent | NotFound | `fulfillment.fulfillment_not_found` | false |
| Lifecycle conflict | Conflict | `fulfillment.invalid_transition` | false |
| Storage failure | Unavailable | `fulfillment.database_unavailable` | true |
| Invalid tenant context | Validation | `fulfillment.context_invalid` | false |

Owner messages are stable safe summaries. Validation details and database text are
not copied into the returned public error.

## Diagnostics

The owner boundary records operation, correlation id, tenant, actor, channel
length, locale length, deadline, optional shipping-option id, requested/default
locale lengths, error kind, code, and retryability. Only the technical database
event retains the typed database cause.

Commerce REST additionally records the transport operation, resource identity,
actor, effective channel, locale, deadline, owner code, kind, retryability,
public code, and HTTP status. The owner message is not used as a protocol.

## Source evidence

`contracts/evidence/shipping-option-read-transport-parity-source.json` has status
`source_cutover_ready_unvalidated`. It records that GraphQL and REST share the
application-host runtime, concrete read construction is absent from mounted
Commerce handlers, native projection reads remain absent by design, and runtime
parity has not been proven.

The fulfillment storefront FFA continues to expose seller/cart shipping selection
over native-server and GraphQL paths. It is not a complete projection-read
transport and must not be expanded merely for numerical transport symmetry.

## Verification

Focused intended checks:

```bash
node scripts/verify/verify-fulfillment-shipping-option-read-port.mjs
node scripts/verify/verify-commerce-graphql-query-fulfillment-context.mjs
node scripts/verify/verify-commerce-shipping-option-transport-parity-inventory.mjs
node scripts/verify/verify-commerce-admin-shipping-option-error-context.mjs
node scripts/verify/verify-commerce-admin-shipping-http-error-safety.mjs
node scripts/verify/verify-commerce-storefront-auxiliary-http-error-safety.mjs
cargo check -p rustok-fulfillment --lib
cargo check -p rustok-commerce --lib
cargo check -p rustok-server --features mod-commerce
```

No command was executed locally in this source wave.

## Remaining work

This source slice does not:

- prove mounted GraphQL/REST active-list, list-all, or lookup parity;
- execute deadline, locale, channel, optional-not-found, or failure fixtures;
- add a native complete-projection read surface without a consumer contract;
- propagate public channel into every GraphQL query read context;
- publish owner query ports for fulfillment lifecycle and order-to-fulfillment
  reads;
- retire `FulfillmentService` from fulfillment-owned adapters or admin mutations;
- modify `ShippingSelectionPort` or an FBA registry contract;
- provide compile, remote-profile, restart, or contention evidence;
- promote fulfillment or ecommerce FFA/FBA status.
