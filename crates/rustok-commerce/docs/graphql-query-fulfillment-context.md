# GraphQL query fulfillment owner context

Status: `source_cutover_diagnostics_bounded_unvalidated`

## Owner projection reads

Mounted Commerce GraphQL and REST projection reads consume application-host-
composed fulfillment boundaries.

Shipping-option reads use:

- `ShippingOptionReadPort::list_shipping_option_projections`;
- `ShippingOptionReadPort::read_shipping_option_projection`;
- `ShippingOptionAdminReadPort::list_all_shipping_option_projections`.

Fulfillment lifecycle reads use:

- `FulfillmentReadPort::read_fulfillment_projection`;
- `FulfillmentReadPort::list_fulfillment_projections`;
- `FulfillmentReadPort::find_latest_fulfillment_by_order_projection`.

The ports return fulfillment-owned response projections. Commerce continues to
apply transport-specific filtering, optional-not-found, pagination, and public
error policy after the owner projection returns.

## Application-host composition

The server bootstrap constructs and caches separate host-selectable runtimes:

- `CommerceShippingOptionReadRuntime`;
- `CommerceFulfillmentLifecycleReadRuntime`.

Both typed values are attached to `HostRuntimeContext`. Existing external
adapters are preserved; otherwise the root in-process factories provide the
baseline implementation.

GraphQL consumes both runtimes through manifest data and one mounted
`CommerceShippingOptionReadScope`. The extension nests both task-local values for
each resolver execution. The private query facade clones all three owner ports and
does not construct `FulfillmentService` or root in-process read providers.

Commerce HTTP router construction consumes the same typed runtimes from
`HostRuntimeContext`. HTTP startup fails closed when either required runtime is
absent.

Directly embedded GraphQL schemas that do not mount the server extension retain
explicit in-process compatibility fallbacks in the public GraphQL runtime module.
Those fallbacks are outside the private facade and are not mounted transport parity
evidence.

## GraphQL lifecycle source cutover

The existing `query.rs` source remains facade-routed and unchanged.

- `fulfillment` calls the compatibility facade, which delegates to
  `read_fulfillment_projection`; owner not-found is adapted back to
  `FulfillmentNotFound`, preserving `Ok(None)`.
- `fulfillments` delegates to `list_fulfillment_projections`; page, per-page,
  status, order, customer, owner total, and `GqlFulfillmentList` metadata are
  preserved.
- admin order detail delegates to
  `find_latest_fulfillment_by_order_projection`; its optional latest fulfillment
  remains unchanged.

The private facade now stores `Arc<dyn FulfillmentReadPort>`. The former concrete
lifecycle `FulfillmentService` field and constructor are removed.

## REST source cutover

Shipping-option REST retains its existing owner-port topology and transport
filters.

Admin fulfillment REST now delegates:

- `GET /admin/fulfillments` to `list_fulfillment_projections`;
- `GET /admin/fulfillments/{id}` to `read_fulfillment_projection`.

Page/per-page, status/order/customer filters, owner total, detail not-found,
public status/code/message policy, and successful DTO envelopes remain unchanged.
Admin fulfillment mutations continue to use their existing concrete or
orchestration services and are outside the read cutover.

## Retained read context

GraphQL shipping-option reads construct `PortContext` with:

- tenant identity;
- service actor `rustok-commerce.graphql-query-shipping-options`;
- requested locale with tenant/default fallback data retained separately;
- query-field and resource-scoped correlation id;
- two-second deadline.

GraphQL fulfillment lifecycle reads construct `PortContext` with:

- tenant identity;
- service actor `rustok-commerce.graphql-query-fulfillments`;
- stable compatibility locale;
- query-field, owner-operation, and resource-scoped correlation id;
- two-second deadline.

The current GraphQL query signature does not carry public channel into every read
context, so complete GraphQL channel propagation remains open.

REST reads construct `PortContext` with tenant identity, authenticated or
storefront service actor as appropriate, request locale, effective channel when
available, resource-scoped correlation id, and a two-second deadline.

## Typed public mapping

GraphQL retains its established compatibility behavior. Lifecycle port failures
are classified through `PortErrorKind`, logged with stable owner code and bounded
context, and adapted back to the existing fulfillment error classes used by
unchanged `query.rs`. No `PortError.message` is parsed or matched as control flow.

REST maps `PortErrorKind` directly to the existing static Commerce HTTP policy:

| Port kind | Storefront policy | Admin policy |
| --- | --- | --- |
| Validation | `400 commerce_store_shipping_invalid` | `400 commerce_admin_fulfillment_invalid` |
| NotFound | `404 commerce_store_not_found` | `404 commerce_admin_not_found` |
| Conflict | `409 commerce_store_shipping_state_conflict` | `409 commerce_admin_fulfillment_state_conflict` |
| Forbidden | `401 commerce_store_denied` | `401 commerce_permission_denied` |
| Unavailable / Timeout | `503 commerce_store_shipping_unavailable` | `503 commerce_admin_fulfillment_storage_unavailable` |
| InvariantViolation | `500 commerce_store_shipping_failed` | `500 commerce_admin_fulfillment_failed` |

These mappings use fixed transport-owned messages.

The private GraphQL fulfillment facade now emits only:

- stable owner code, classified owner kind, and owner retryability;
- query field and owner operation;
- error/warn severity selected from `PortErrorKind`;
- tenant, actor, correlation, locale, channel, claims, roles, and deadline as
  bounded lengths, counts, presence flags, or static kinds;
- shipping-option, fulfillment, and order identity as `absent`, `nil`, or
  `non_nil` shapes;
- owner and public message presence and length without message content;
- a zero-sized diagnostic token whose `Debug` output is always `redacted`.

It no longer emits complete `PortError` values, raw correlation ids, raw tenant or
actor values, raw resource UUIDs, owner message content, or public message content.

## Preserved contracts

- GraphQL `query.rs` remains facade-routed and successful GraphQL DTOs are
  unchanged.
- Single GraphQL shipping-option and fulfillment not-found still return `None`.
- GraphQL fulfillment list filters, pagination total, and metadata are unchanged.
- GraphQL admin order detail retains optional latest fulfillment.
- Storefront owner shipping-option list remains active-only.
- REST currency, channel visibility, shipping-profile, admin filtering, and
  pagination behavior remain unchanged.
- Admin shipping-option and fulfillment mutations retain their existing concrete
  or orchestration services and typed error mappers.
- Technical fulfillment query owner failures remain `error`; ordinary owner
  rejections remain `warn`.
- Native seller/cart selection remains a separate contract.
- Fulfillment FFA/FBA status is unchanged.

## Still open

- Execute mounted GraphQL/REST shipping-option and fulfillment lifecycle parity
  fixtures.
- Prove locale, effective channel, deadline, tenant, filter, optional not-found,
  typed failure, restart, and external-adapter behavior through mounted requests.
- Add public-channel propagation to every GraphQL fulfillment read context.
- Continue converting remaining Commerce query boundaries that still use dynamic
  strings.
- Continue the shared storefront HTTP, inventory, customer, tax, promotion,
  native-transport, remaining-adapter, and non-`PortError` public-envelope cleanup.
- Add compile and remote-profile evidence before promoting any FFA/FBA status.

## Intended checks

```bash
node scripts/verify/verify-commerce-graphql-query-fulfillment-context.mjs
node scripts/verify/verify-commerce-graphql-query-fulfillment-diagnostic-safety.mjs
node scripts/verify/verify-fulfillment-lifecycle-read-port.mjs
node scripts/verify/verify-commerce-shipping-option-transport-parity-inventory.mjs
node scripts/verify/verify-commerce-admin-shipping-option-error-context.mjs
node scripts/verify/verify-commerce-admin-shipping-http-error-safety.mjs
node scripts/verify/verify-commerce-storefront-auxiliary-http-error-safety.mjs
node scripts/verify/verify-fulfillment-shipping-option-read-port.mjs
cargo check -p rustok-commerce --lib
cargo check -p rustok-server --features mod-commerce
```

Tests, Cargo commands, formatting commands, verifiers, mounted GraphQL scenarios,
workflow checks, and CI were not run for this source wave; validation remains
maintainer-owned.
