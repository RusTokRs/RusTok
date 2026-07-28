# Shipping-option read port

Status: source-ready, unvalidated.

## Scope

`ShippingOptionReadPort` and `ShippingOptionAdminReadPort` are fulfillment-owned
read boundaries for complete shipping-option projections needed by mounted
ecommerce transports.

It is separate from `ShippingSelectionPort`:

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

## In-process adapter

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

The mounted storefront GraphQL helpers use the root read-port factory:

- shipping-option validation calls `read_shipping_option_projection`;
- cart shipping enrichment and storefront query listing call
  `list_shipping_option_projections`.

The separate admin owner port now publishes
`list_all_shipping_option_projections` for the mounted administrative GraphQL
`shipping_options` query. This source slice publishes and verifies that owner
contract; the transport cutover remains a separate bounded change so the
existing public envelope can be preserved and reviewed independently.

Commerce builds a read `PortContext` with a service actor, resource-scoped
correlation id, request locale, optional public channel where applicable, and a
two-second deadline. Existing GraphQL public envelopes remain unchanged.

Delivery-group projection is a pure commerce function receiving owner
projections. The existing compatibility service adapter delegates to the same
pure function, so legacy REST/native behavior is not changed by this slice.

## Verification

Focused source guards:

```bash
node scripts/verify/verify-fulfillment-shipping-option-read-port.mjs
node scripts/verify/verify-commerce-graphql-shipping-option-typed-error.mjs
node scripts/verify/verify-commerce-graphql-shipping-enrichment-typed-error.mjs
cargo check -p rustok-fulfillment --lib
cargo check -p rustok-commerce --lib
```

No command was executed locally in this source wave.

## Remaining work

This slice does not:

- cut the mounted administrative GraphQL `shipping_options` query over to
  `list_all_shipping_option_projections`;
- inject the read ports from the application host rather than root in-process
  factories;
- migrate REST/native storefront shipping reads;
- retire `FulfillmentService` from fulfillment-owned compatibility adapters;
- modify `ShippingSelectionPort`;
- add or change an FBA registry contract;
- provide runtime, remote-profile, restart, or contention evidence;
- promote fulfillment or ecommerce FFA/FBA status.
