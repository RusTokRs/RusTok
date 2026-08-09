# Commerce REST storefront Product detail owner-read cutover

Status: `source_complete_unvalidated`

Date: 2026-08-09

## Scope

This bounded slice removes concrete Product service construction from the mounted
`GET /store/products/{id}` handler.

The storefront Product list remains outside this slice. Its legacy REST search
semantics are still implemented directly in Commerce and require a separate parity
proof before owner-port cutover.

## Mounted path

`crates/rustok-commerce/src/controllers/store/mod.rs` mounts
`products::show_product` at `/store/products/{id}`.

`crates/rustok-commerce/src/controllers/store/products.rs` now obtains the
host-selected Product capability through `CommerceHttpRuntime::product_catalog_read_port()`
and calls:

- `ProductCatalogReadPort::read_storefront_product_projection`
- `StorefrontProductProjectionSubject::ProductId`

No `CatalogService::new(...)` remains in the mounted detail handler.

## Preserved semantics

The owner capability already centralizes the behavior previously split across the
Commerce handler and Product service:

- admitted tenant identity;
- requested locale plus tenant default fallback locale;
- published + active Product visibility;
- public-channel metadata visibility;
- public-channel inventory projection;
- hidden or missing Product -> the existing `commerce_store_not_found` envelope.

The request still passes the storefront channel admission guard before the owner read.
The owner `PortContext` uses a stable Commerce service actor, request locale, normalized
public channel when available, a Product-bound correlation id, and a two-second deadline.

## Error boundary

Product owner `PortError` values are mapped to the existing public REST families:

- validation -> `400 commerce_store_product_invalid`;
- not found -> `404 commerce_store_not_found`;
- unavailable/timeout -> `503 commerce_store_product_unavailable`;
- unexpected conflict/forbidden/invariant failures -> fail closed with
  `500 commerce_store_product_failed`.

The new owner-port mapper logs correlation id, owner error kind, retryability, and
owner-code length. It does not log or expose `PortError.message` or another raw
backend/owner message.

## Deliberately unchanged

`GET /store/products` remains on its existing legacy direct Product query/tag path in
this slice. The owner already publishes a legacy storefront list capability, but the
current REST search is locale-scoped while the compatibility owner helper must be
rechecked for exact search parity before replacing the mounted list path.

Region and shipping-option handlers in the same source file are unchanged.

## Topology status

The canonical ecommerce topology item remains open. This slice removes one mounted
Product concrete-service construction, but other mounted Product/Order/Payment/
Fulfillment consumer and orchestration paths still require audit/cutover.

## Validation status

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter,
REST scenarios, workflows, CI reruns, database scenarios, provider calls, restart
scenarios, or remote-adapter scenarios were executed. Source verifiers were updated
or added but were not run.
