# GraphQL query fulfillment owner context

Status: `source_cutover_ready_unvalidated`

## Shipping-option owner reads

Mounted Commerce GraphQL and REST projection reads now consume the same
application-host-composed fulfillment boundaries:

- storefront active list uses
  `ShippingOptionReadPort::list_shipping_option_projections`;
- single shipping-option lookup uses
  `ShippingOptionReadPort::read_shipping_option_projection`;
- administrative list-all uses
  `ShippingOptionAdminReadPort::list_all_shipping_option_projections`.

The ports return fulfillment-owned `ShippingOptionResponse` projections. Commerce
continues to apply transport-specific filtering and public error policy after the
owner projection returns.

This remains a partial fulfillment-query topology result. The private GraphQL
compatibility facade still retains one concrete `FulfillmentService` delegate for
fulfillment lifecycle list/lookup and order-to-fulfillment reads that do not yet
have published owner query ports.

## Application-host composition

The server bootstrap constructs one `CommerceShippingOptionReadRuntime` from the
storefront and administrative root factories, caches it in
`ServerRuntimeContext`, and attaches it to `HostRuntimeContext`.

GraphQL consumes that runtime through manifest data and
`CommerceShippingOptionReadScope`. The private query facade clones the scoped
owner ports and does not construct read providers.

Commerce HTTP router construction consumes the same typed runtime from
`HostRuntimeContext` and stores it in `CommerceHttpRuntime`. HTTP startup fails
closed when the runtime is absent. The REST handlers clone the owner ports from
that state; they do not construct `FulfillmentService` or root in-process read
providers for shipping-option projection reads.

Directly embedded GraphQL schemas that do not mount the server extension retain
an explicit `CommerceShippingOptionReadRuntime::in_process` compatibility
fallback in the public GraphQL runtime module. This fallback is outside the
private facade and is not mounted transport parity evidence.

## REST source cutover

The source inventory at
`crates/rustok-fulfillment/contracts/evidence/shipping-option-read-transport-parity-source.json`
has status `source_cutover_ready_unvalidated`.

It records the following source topology:

- mounted GraphQL and Commerce REST obtain both read ports from the same
  `CommerceShippingOptionReadRuntime`;
- storefront REST delegates active-list loading to
  `list_shipping_option_projections`, then retains its currency,
  public-channel-visibility, and shipping-profile compatibility filters;
- admin REST delegates complete list loading to
  `list_all_shipping_option_projections`, preserving inactive options before
  active, currency, provider, search, and pagination filters;
- admin REST single lookup delegates to `read_shipping_option_projection`;
- admin shipping-option create, update, deactivate, and reactivate remain
  lifecycle mutations over the concrete owner service and are outside this read
  cutover;
- the native fulfillment storefront surface remains seller/cart selection through
  `ShippingSelectionPort`; it does not publish complete projection list/lookup
  operations without a consumer contract.

The inventory is source evidence only. It does not claim compiled handlers,
mounted request execution, GraphQL/REST result parity, or any FFA/FBA promotion.

## Retained read context

GraphQL shipping-option reads construct `PortContext` with:

- tenant identity;
- service actor `rustok-commerce.graphql-query-shipping-options`;
- requested locale with tenant/default fallback data retained separately;
- query-field and resource-scoped correlation id;
- two-second deadline.

The current GraphQL query signature does not carry public channel into every
shipping-option read context, so that propagation remains open.

REST shipping-option reads construct `PortContext` with:

- tenant identity;
- authenticated user actor for admin reads;
- authenticated user actor or storefront service actor for public reads;
- request locale;
- effective public channel when available, including cart-derived channel for
  storefront list requests;
- resource-scoped correlation id;
- two-second deadline.

Requested and tenant-default locale values remain explicit owner request fields.
Single lookup retains the shipping-option UUID.

## Typed public mapping

GraphQL retains its existing typed `FULFILLMENT_*` boundary policy and optional
single-lookup not-found behavior. No `PortError.message` is parsed or matched as
control flow.

REST maps `PortErrorKind` directly to the existing static Commerce HTTP policy:

| Port kind | Storefront policy | Admin policy |
| --- | --- | --- |
| Validation | `400 commerce_store_shipping_invalid` | `400 commerce_admin_fulfillment_invalid` |
| NotFound | `404 commerce_store_not_found` | `404 commerce_admin_not_found` |
| Conflict | `409 commerce_store_shipping_state_conflict` | `409 commerce_admin_fulfillment_state_conflict` |
| Forbidden | `401 commerce_store_denied` | `401 commerce_permission_denied` |
| Unavailable / Timeout | `503 commerce_store_shipping_unavailable` | `503 commerce_admin_fulfillment_storage_unavailable` |
| InvariantViolation | `500 commerce_store_shipping_failed` | `500 commerce_admin_fulfillment_failed` |

These mappings use fixed transport-owned messages. Owner error code, kind,
retryability, correlation, actor, locale, channel, resource identity, and deadline
are retained in diagnostics without exposing the owner message.

## Preserved contracts

- GraphQL `query.rs` remains facade-routed and successful GraphQL DTOs are
  unchanged.
- Single GraphQL shipping-option not-found still returns `None`.
- Storefront owner list remains active-only.
- Storefront REST currency, channel visibility, and shipping-profile filters are
  unchanged after owner projections return.
- Administrative list-all still includes inactive options before local active,
  currency, provider, search, and pagination filtering.
- REST successful DTOs and route shapes are unchanged.
- Admin shipping-option mutations retain their existing concrete lifecycle
  service and typed `FulfillmentError` HTTP mapper.
- Fulfillment lifecycle GraphQL reads remain on the isolated concrete delegate.
- Native seller/cart selection remains a separate contract.
- Fulfillment FFA/FBA status is unchanged.

## Still open

- Execute mounted GraphQL/REST active-list, list-all, and lookup parity fixtures.
- Prove locale, effective channel, deadline, optional not-found, and typed failure
  behavior through mounted requests.
- Add public-channel propagation to every GraphQL shipping-option query context.
- Publish owner ports for fulfillment lifecycle query reads and remove the
  remaining concrete GraphQL facade delegate.
- Continue converting remaining Commerce query boundaries that still use dynamic
  strings.
- Add compile and remote-profile evidence before promoting any FFA/FBA status.

## Intended checks

```bash
node scripts/verify/verify-commerce-graphql-query-fulfillment-context.mjs
node scripts/verify/verify-commerce-shipping-option-transport-parity-inventory.mjs
node scripts/verify/verify-commerce-admin-shipping-option-error-context.mjs
node scripts/verify/verify-commerce-admin-shipping-http-error-safety.mjs
node scripts/verify/verify-commerce-storefront-auxiliary-http-error-safety.mjs
node scripts/verify/verify-fulfillment-shipping-option-read-port.mjs
cargo check -p rustok-commerce --lib
cargo check -p rustok-server --features mod-commerce
```

Tests, Cargo commands, formatting commands, verifiers, workflow checks, and CI
were not run locally for this source wave; validation remains maintainer-owned.
