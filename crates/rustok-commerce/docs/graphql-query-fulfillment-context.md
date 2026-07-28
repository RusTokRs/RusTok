# GraphQL query fulfillment owner context

Status: `source_ready_unvalidated`

## Shipping-option read cutover

The safe Commerce GraphQL query facade routes all mounted shipping-option reads
through fulfillment-owned read ports:

- `storefront_shipping_options` delegates
  `ShippingOptionReadPort::list_shipping_option_projections`;
- single shipping-option lookup delegates
  `ShippingOptionReadPort::read_shipping_option_projection`;
- administrative `shipping_options` delegates
  `ShippingOptionAdminReadPort::list_all_shipping_option_projections`.

The unchanged `query.rs` source still imports and constructs the private
`rustok_fulfillment::FulfillmentService` facade. Inside that facade, storefront
and administrative shipping-option methods use separate owner read-port fields.
The facade keeps one concrete `FulfillmentService` delegate only for fulfillment
lifecycle reads not covered by this slice:

- fulfillment lookup/list;
- order-to-fulfillment lookup.

This is a partial topology result. It closes concrete service and provider
construction for mounted shipping-option GraphQL reads, but it does not claim
that every fulfillment query method is owner-port composed.

## Application-host composition

The server's commerce runtime bootstrap constructs one
`CommerceShippingOptionReadRuntime` from the storefront and administrative root
factories, caches it in `ServerRuntimeContext`, and attaches it to
`HostRuntimeContext`. The manifest GraphQL data factory requires that typed
runtime and stores it in `CommerceGraphqlRuntimeData`.

`CommerceShippingOptionReadScope` is mounted as an async-graphql extension. For
each resolver it copies the typed runtime into a Tokio task-local scope. The
private fulfillment facade resolves that scoped runtime and clones both owner
ports; it no longer imports or calls either root in-process factory.

Directly embedded schemas that do not mount the server extension retain an
explicit `CommerceShippingOptionReadRuntime::in_process` compatibility fallback
in the public Commerce GraphQL runtime module. The fallback preserves existing
standalone schema tests and embeddings without turning the private facade back
into a provider constructor.

## Retained read context

Each port-backed shipping-option query constructs `PortContext` with:

- tenant identity;
- service actor `rustok-commerce.graphql-query-shipping-options`;
- requested locale, falling back to tenant default locale and then `en`;
- query-field and resource-scoped correlation id;
- two-second deadline.

Requested and tenant-default locale values are also retained as explicit owner
request fields. Single lookup retains the shipping-option UUID. The current
query source does not pass public channel into the facade method, so channel
propagation remains open for a later source/query signature change.

## Typed owner mapping

Storefront and administrative list facades map `PortErrorKind` without using
owner message text:

| Port kind | Public message | Public code | Retryable |
| --- | --- | --- | --- |
| Validation | `Fulfillment query is invalid` | `FULFILLMENT_REQUEST_INVALID` | false |
| NotFound | `Fulfillment resource was not found` | `FULFILLMENT_RESOURCE_NOT_FOUND` | false |
| Conflict | `Fulfillment state conflicts with this query` | `FULFILLMENT_STATE_CONFLICT` | false |
| Unavailable / Timeout | `Fulfillment data is temporarily unavailable` | `FULFILLMENT_TEMPORARILY_UNAVAILABLE` | true |
| Forbidden | `Fulfillment query is not permitted` | `FULFILLMENT_ACCESS_DENIED` | false |
| InvariantViolation | `Fulfillment query could not be completed safely` | `FULFILLMENT_OPERATION_FAILED` | false |

The validation, not-found, conflict, and unavailable envelopes preserve the
existing fulfillment public policy. Forbidden and invariant outcomes are
fail-closed coverage for the complete port kind set and are not status
promotions.

The administrative resolver source still contains its existing
`async_graphql::Error::new(err.to_string())` call. The private facade returns
`ShippingOptionAdminQueryError`, whose inherent `to_string` method consumes the
adapter and returns the already-typed `BoundaryError`. Rust resolves that
inherent method before the standard `ToString` trait, so no string is created,
parsed, matched, or used as a control-flow protocol. The same public
`FULFILLMENT_*` message, code, and retryability values reach GraphQL extensions.

Single lookup preserves the existing optional-result source contract rather than
changing `query.rs`:

- owner `NotFound` becomes the stable compatibility variant
  `ShippingOptionNotFound`, so the resolver still returns `Ok(None)`;
- validation, conflict, unavailable, timeout, forbidden, and invariant outcomes
  become stable compatibility `FulfillmentError` values;
- the unchanged resolver converts those stable values through its existing
  generic `COMMERCE_QUERY_OPERATION_FAILED` redaction path.

No `PortError.message` or original fulfillment message is copied into the
compatibility values or administrative GraphQL boundary.

## Diagnostics

The port-backed query boundary records:

- truthful owner `rustok_fulfillment`;
- correlation id;
- tenant and service actor;
- context locale length;
- deadline;
- GraphQL query field;
- exact owner port operation;
- optional shipping-option id;
- requested/default locale lengths;
- stable port error kind name;
- owner code, kind, and retryability;
- public or optional-result policy;
- boundary `commerce_graphql_query_fulfillment_facade`.

Unavailable, timeout, and invariant outcomes use error severity and retain the
stable typed `PortError`. Ordinary outcomes use warning severity without an
owner message field. The fulfillment owner ports independently retain their
owner-local diagnostics and technical database cause.

## Preserved contracts

- `query.rs` is unchanged and remains facade-routed.
- Existing source-level `FulfillmentService::new(db.clone())` calls still resolve
  through the private safe facade.
- Single shipping-option not-found still returns `None` rather than a GraphQL
  error.
- Storefront active-only semantics, currency, channel-visibility, and
  shipping-profile filtering remain unchanged after owner projections return.
- Administrative list-all still includes inactive options before local active,
  currency, provider, search, and pagination filtering.
- Shipping-option GraphQL DTOs and successful results are unchanged.
- Fulfillment lifecycle and order lookup remain on the isolated concrete
  delegate.
- Directly embedded schemas keep an explicit compatibility fallback without
  changing the mounted server composition path.
- Admin order lookup and non-not-found single shipping-option failures keep their
  existing generic `COMMERCE_QUERY_OPERATION_FAILED` fail-closed envelope after
  stable compatibility conversion.
- The fulfillment FFA/FBA status and `ShippingSelectionPort` contract are
  unchanged.

## Still open

- Add public-channel propagation to the shipping-option query `PortContext`.
- Publish owner ports for fulfillment lifecycle query reads, then remove the
  remaining concrete `FulfillmentService` field.
- Migrate REST/native shipping reads and retain parity evidence.
- Continue reviewing order, payment, customer, inventory, region, channel,
  catalog, and remaining Commerce query conversions that still pass through
  dynamic strings.
- Add compile, mounted transport, deadline, REST/native parity, and remote
  evidence before promoting any FBA/FFA status.

## Intended checks

```bash
node scripts/verify/verify-commerce-graphql-query-fulfillment-context.mjs
node scripts/verify/verify-fulfillment-shipping-option-read-port.mjs
node scripts/verify/verify-commerce-graphql-query-error-boundary.mjs
cargo check -p rustok-commerce --lib
cargo check -p rustok-server --features mod-commerce
```

Tests, Cargo commands, formatting commands, verifiers, workflow checks, and CI
were not run locally for this source wave; validation remains maintainer-owned.
