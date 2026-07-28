# Shipping-option read port

Status: source-ready, unvalidated.

## Scope

`ShippingOptionReadPort` and `ShippingOptionAdminReadPort` are fulfillment-owned
read boundaries for complete shipping-option projections needed by mounted
ecommerce transports.

They are separate from `ShippingSelectionPort`:

- `ShippingSelectionPort` owns seller/cart selection workflow;
- `ShippingOptionReadPort` owns storefront active-list and single-option reads;
- `ShippingOptionAdminReadPort` owns the complete administrative list-all read;
- the selection contract and its provider registry are unchanged;
- no new FBA provider contract or status promotion is claimed.

## Operations

The two ports publish three read-only operations:

- `list_shipping_option_projections` for the storefront-visible active list;
- `list_all_shipping_option_projections` for the complete administrative list,
  including inactive options;
- `read_shipping_option_projection` for one complete owner projection.

All operations return the existing fulfillment-owned `ShippingOptionResponse`.
This keeps metadata, allowed shipping-profile slugs, active state, localization
facts, and provider identity inside the owner projection without defining a
second partial commerce copy.

## Requests

`ListShippingOptionProjectionsRequest` and
`ListAllShippingOptionProjectionsRequest` carry:

- requested locale;
- tenant default locale.

`ReadShippingOptionProjectionRequest` additionally carries:

- shipping-option id.

Locale values remain request data rather than being inferred from public error
text. The delegated `PortContext` independently carries tenant, actor, channel,
locale, correlation, causation, trace, and deadline facts.

## In-process adapters

`InProcessShippingOptionReadPort` and
`InProcessShippingOptionAdminReadPort` own `FulfillmentService` construction and
are exported through separate root factories:

```rust
in_process_shipping_option_read_port(db)
in_process_shipping_option_admin_read_port(db)
```

The adapters:

1. require read policy;
2. parse tenant identity from `PortContext`;
3. delegate active list, administrative list-all, or lookup to
   `FulfillmentService`;
4. preserve requested/default locale arguments;
5. map every `FulfillmentError` variant to a stable `PortError`;
6. return the owner projection unchanged on success.

The two list contracts remain separate traits rather than one overloaded request
or one growing storefront interface. That keeps storefront active-only semantics
separate from the admin requirement to inspect inactive shipping options and
avoids breaking existing `ShippingOptionReadPort` implementors.

## Application-host composition

The mounted server composes one `CommerceShippingOptionReadRuntime` from the two
root factories and stores it in `HostRuntimeContext`. Manifest-generated Commerce
GraphQL runtime data requires that typed runtime during schema construction.
`CommerceShippingOptionReadScope` then copies the runtime into a Tokio task-local
scope for each resolver execution.

The private Commerce fulfillment facade resolves the scoped runtime and clones
its two owner ports. It no longer imports or calls the root in-process factories.
This preserves the unchanged `query.rs` compatibility facade while keeping
provider construction in the application host.

Directly embedded schemas that do not install the mounted server extension use
an explicit `CommerceShippingOptionReadRuntime::in_process` compatibility
fallback in the Commerce GraphQL runtime module. The fallback is outside the
private facade and is not evidence of mounted transport composition or parity.

## Transport parity source inventory

`contracts/evidence/shipping-option-read-transport-parity-source.json` records a
source-only inventory at revision
`aa1f12d7660bda14daa5d8ade11d3074418a573f`. Its status is
`source_audit_only_unvalidated`; it does not claim compiled or mounted behavior.

The inventory establishes three distinct transport states:

1. Mounted GraphQL active list, admin list-all, and lookup consume the
   application-host runtime and fulfillment owner ports.
2. Commerce REST preserves the same successful projection filtering, but its
   storefront active list and admin list-all/lookup still construct concrete
   `FulfillmentService` instances.
3. Fulfillment storefront FFA exposes seller/cart shipping selection over native
   server and GraphQL paths. It does not expose complete shipping-option
   projection list or lookup operations.

The third state is not a missing parity implementation: `ShippingSelectionPort`
and the complete projection read ports have different consumers and semantics.
A projection API must not be added to the native selection surface merely to make
transport counts look symmetrical.

The retained decision is therefore:

- migrate the existing REST projection reads to the host-composed owner runtime;
- preserve storefront currency/channel/profile filters;
- preserve admin inactive-before-filter behavior and active/currency/provider/
  search/pagination filters;
- preserve current HTTP public status/code/message policy through typed
  `PortErrorKind` mapping;
- leave the native selection contract unchanged unless a complete projection
  consumer is designed.

`scripts/verify/verify-commerce-shipping-option-transport-parity-inventory.mjs`
locks this inventory and decision until the REST cutover deliberately updates
both source and evidence.

## Stable owner errors

| Owner outcome | Port kind | Code | Retryable |
| --- | --- | --- | --- |
| Invalid request | Validation | `fulfillment.validation` | false |
| Shipping option absent | NotFound | `fulfillment.shipping_option_not_found` | false |
| Fulfillment absent | NotFound | `fulfillment.fulfillment_not_found` | false |
| Lifecycle conflict | Conflict | `fulfillment.invalid_transition` | false |
| Storage failure | Unavailable | `fulfillment.database_unavailable` | true |
| Invalid tenant context | Validation | `fulfillment.context_invalid` | false |

Messages are stable owner-safe summaries. Validation text and database text are
not copied into the returned `PortError.message`.

## Diagnostics

The owner boundary records:

- owner and operation;
- correlation id;
- tenant id;
- actor;
- channel length;
- locale length;
- causation-id presence;
- traceparent presence;
- deadline;
- optional shipping-option id;
- requested-locale length;
- default-locale length;
- internal error kind, code, and retryability.

Only the technical database event retains the typed owner cause. Ordinary
validation, not-found, and conflict events do not add raw owner message fields.

## Mounted commerce cutover

Mounted Commerce GraphQL shipping-option paths use the host-composed read ports:

- shipping-option validation and single query lookup call
  `read_shipping_option_projection`;
- cart shipping enrichment and storefront query listing call
  `list_shipping_option_projections`;
- administrative `shipping_options` calls
  `list_all_shipping_option_projections`.

Commerce builds a read `PortContext` with a service actor, resource-scoped
correlation id, request locale, optional public channel where applicable, and a
two-second deadline. Existing optional-not-found behavior and public GraphQL
`FULFILLMENT_*` message, code, and retryability envelopes remain unchanged.

The administrative resolver source retains its existing `.to_string()` call for
source compatibility, but the private Commerce facade returns a typed adapter
whose inherent method yields the already-classified GraphQL boundary. No owner
message is serialized, parsed, or matched. Administrative list-all still returns
inactive options before the existing local active, currency, provider, search,
and pagination filters are applied.

Delivery-group projection is a pure Commerce function receiving owner
projections. Existing selection adapters continue to delegate to the separate
`ShippingSelectionPort`; they are not projection-read transports.

## Verification

Focused source guards:

```bash
node scripts/verify/verify-fulfillment-shipping-option-read-port.mjs
node scripts/verify/verify-commerce-graphql-query-fulfillment-context.mjs
node scripts/verify/verify-commerce-shipping-option-transport-parity-inventory.mjs
node scripts/verify/verify-commerce-graphql-shipping-option-typed-error.mjs
node scripts/verify/verify-commerce-graphql-shipping-enrichment-typed-error.mjs
cargo check -p rustok-fulfillment --lib
cargo check -p rustok-commerce --lib
cargo check -p rustok-server --features mod-commerce
```

No command was executed locally in this source wave.

## Remaining work

This slice does not:

- migrate Commerce REST storefront active-list or admin list-all/lookup to the
  host-composed read ports;
- execute GraphQL/REST parity, deadline, locale, channel, or failure fixtures;
- add a native complete-projection read surface without a consumer contract;
- propagate public channel into every GraphQL query read context;
- publish owner read ports for fulfillment lifecycle and order-to-fulfillment
  query paths;
- retire `FulfillmentService` from fulfillment-owned compatibility adapters;
- modify `ShippingSelectionPort`;
- add or change an FBA registry contract;
- provide compile, mounted runtime, remote-profile, restart, or contention evidence;
- promote fulfillment or ecommerce FFA/FBA status.
